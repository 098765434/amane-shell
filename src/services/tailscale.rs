//! Tailscale — the tailnet as `tailscaled` has it, driven through its CLI,
//! the interface upstream documents.
//!
//! Nothing polls: `tailscale debug watch-ipn` prints whenever anything
//! changes, and each line is taken only as "something happened" (it's
//! pretty-printed JSON, and a `debug` command besides), after which
//! `status --json` and `debug prefs` are read again. A burst is collapsed
//! into one read. When the daemon goes away the stream ends with it, and
//! it's started again with a backoff, so a stopped daemon never has the
//! shell respawning a process in a tight loop.
//!
//! Reads work for anyone; writes need root or the operator grant
//! (`sudo tailscale set --operator=$USER`). Without it the state still
//! shows, read-only, and the page says how to grant it; the shell never
//! escalates. `tailscale up` is only ever run without flags: with any, it
//! resets every setting not given. Settings go through `tailscale set`.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use amane::Service;
use serde_json::Value;

use super::{installed, run};

const BINARY: &str = "tailscale";

/// How long a burst of changes is let settle before reading once.
const SETTLE: Duration = Duration::from_millis(300);

/// The change stream's restart pacing.
const BACKOFF_FIRST: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(60);

/// How long a login waits for the control plane's URL.
const LOGIN_WAIT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// No `tailscale` binary: nothing to show.
    #[default]
    Missing,
    /// The binary is there but `tailscaled` doesn't answer.
    DaemonDown,
    NeedsLogin,
    Starting,
    Stopped,
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub name: String,
    pub dns: String,
    /// Its IPv4 tailnet address, else the first it has.
    pub ip: Option<String>,
    pub os: String,
    pub online: bool,
    /// It offers itself as an exit node.
    pub exit_option: bool,
    /// It's the exit node in use.
    pub exit_active: bool,
}

/// What `status --json` says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Status {
    state: State,
    hostname: String,
    ip: Option<String>,
    peers: Vec<Peer>,
    health: Vec<String>,
    auth_url: Option<String>,
}

/// What `debug prefs` says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Prefs {
    accept_routes: bool,
    accept_dns: bool,
    shields_up: bool,
    operator: Option<String>,
}

#[derive(Default, PartialEq)]
pub struct Tailscale {
    status: Status,
    prefs: Option<Prefs>,
    // a write was refused for want of the operator grant
    denied: bool,
    logging_in: bool,
    error: Option<String>,
}

impl Service for Tailscale {
    fn new() -> Self {
        let mut tailscale = Self::default();
        if installed(BINARY) {
            let (status, prefs) = read_now();
            tailscale.status = status;
            tailscale.prefs = prefs;
        }
        tailscale
    }

    fn listen() {
        if !installed(BINARY) {
            return;
        }
        let mut backoff = BACKOFF_FIRST;
        loop {
            if watch() {
                backoff = BACKOFF_FIRST;
            }
            // the daemon may have gone with the stream
            refresh();
            thread::sleep(backoff);
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    }
}

impl Tailscale {
    /// Whether there's a `tailscale` to drive.
    pub fn present(&self) -> bool {
        self.status.state != State::Missing
    }

    pub fn state(&self) -> State {
        self.status.state
    }

    /// Connected or on the way.
    pub fn active(&self) -> bool {
        matches!(self.status.state, State::Running | State::Starting)
    }

    pub fn hostname(&self) -> &str {
        &self.status.hostname
    }

    /// This device's tailnet address.
    pub fn ip(&self) -> Option<&str> {
        self.status.ip.as_deref()
    }

    /// The other devices on the tailnet, online first, then by name.
    pub fn peers(&self) -> &[Peer] {
        &self.status.peers
    }

    pub fn exit_nodes(&self) -> impl Iterator<Item = &Peer> {
        self.status.peers.iter().filter(|peer| peer.exit_option)
    }

    pub fn exit_node(&self) -> Option<&Peer> {
        self.status.peers.iter().find(|peer| peer.exit_active)
    }

    /// What tailscaled thinks is wrong; empty when all's well.
    pub fn health(&self) -> &[String] {
        &self.status.health
    }

    pub fn accept_routes(&self) -> bool {
        self.prefs.as_ref().is_some_and(|p| p.accept_routes)
    }

    pub fn accept_dns(&self) -> bool {
        self.prefs.as_ref().is_some_and(|p| p.accept_dns)
    }

    pub fn shields_up(&self) -> bool {
        self.prefs.as_ref().is_some_and(|p| p.shields_up)
    }

    /// Writes would be refused: no operator grant for this user (judged
    /// from the prefs, without a probe write), or one already was.
    pub fn read_only(&self) -> bool {
        self.denied
            || self
                .prefs
                .as_ref()
                .is_some_and(|p| p.operator.as_deref() != Some(user().as_str()))
    }

    /// A login is waiting for the control plane's URL.
    pub fn logging_in(&self) -> bool {
        self.logging_in
    }

    /// What the last write that failed said.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// One line for what it's doing: "via <exit node>", the address, …
    pub fn describe(&self) -> String {
        match self.status.state {
            State::Running => match (self.exit_node(), self.ip()) {
                (Some(exit), _) => format!("via {}", exit.name),
                (None, Some(ip)) => ip.to_string(),
                (None, None) => String::from("Connected"),
            },
            State::Starting => String::from("Connecting…"),
            State::NeedsLogin if self.logging_in => String::from("Contacting Tailscale…"),
            State::NeedsLogin => String::from("Not logged in"),
            State::Stopped => String::from("Disconnected"),
            State::DaemonDown => String::from("Daemon not running"),
            State::Missing => String::new(),
        }
    }

    /// The one-time grant that makes it writable. Shown, never run.
    pub fn operator_command() -> String {
        format!("sudo {BINARY} set --operator={}", user())
    }

    pub fn up() {
        write(vec!["up".into()]);
    }

    pub fn down() {
        write(vec!["down".into()]);
    }

    /// Routes through `ip`, or straight out with `None`.
    pub fn set_exit_node(ip: Option<&str>) {
        write(vec![
            "set".into(),
            format!("--exit-node={}", ip.unwrap_or("")),
        ]);
    }

    pub fn set_accept_routes(on: bool) {
        write(vec!["set".into(), format!("--accept-routes={on}")]);
    }

    pub fn set_accept_dns(on: bool) {
        write(vec!["set".into(), format!("--accept-dns={on}")]);
    }

    pub fn set_shields_up(on: bool) {
        write(vec!["set".into(), format!("--shields-up={on}")]);
    }

    /// Starts a login and opens its page in the browser once the control
    /// plane hands out the URL (a few seconds). `tailscale login` waits
    /// until the browser half is done, so it's left running.
    pub fn login() {
        {
            let mut tailscale = Self::write();
            if tailscale.logging_in {
                return;
            }
            tailscale.logging_in = true;
            tailscale.error = None;
        }
        thread::spawn(|| {
            let child = Command::new(BINARY)
                .arg("login")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match child {
                // reaped whenever it finishes
                Ok(mut child) => drop(thread::spawn(move || child.wait())),
                Err(e) => Self::write().error = Some(format!("{BINARY} login: {e}")),
            }

            let start = Instant::now();
            while start.elapsed() < LOGIN_WAIT {
                thread::sleep(Duration::from_secs(1));
                let url = capture(&["status", "--json"])
                    .and_then(|json| parse_status(&json))
                    .and_then(|status| status.auth_url);
                if let Some(url) = url {
                    run("xdg-open", vec![url]);
                    break;
                }
            }
            Self::write().logging_in = false;
        });
    }

    /// Puts `text` on the clipboard.
    pub fn copy(text: &str) {
        run("wl-copy", vec![text.to_string()]);
    }
}

// ───── the daemon ────────────────────────────────────────────────────────────

fn user() -> String {
    std::env::var("USER").unwrap_or_default()
}

/// Runs `tailscale args` and gives its stdout, whatever it exits with (the
/// JSON reads report ordinary states that way); `None` when it won't start.
fn capture(args: &[&str]) -> Option<String> {
    let output = Command::new(BINARY)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_now() -> (Status, Option<Prefs>) {
    let status = capture(&["status", "--json"])
        .and_then(|json| parse_status(&json))
        .unwrap_or(Status {
            state: State::DaemonDown,
            ..Status::default()
        });
    let prefs = capture(&["debug", "prefs"]).and_then(|json| parse_prefs(&json));
    (status, prefs)
}

/// Reads the daemon again, redrawing only on a change.
fn refresh() {
    let (status, prefs) = read_now();
    let tailscale = Tailscale::read();
    let same = tailscale.status == status && tailscale.prefs == prefs;
    drop(tailscale);
    if same {
        return;
    }
    let mut tailscale = Tailscale::write();
    // a new grant (or a lost one) is news the prefs bring
    if tailscale.prefs.as_ref().map(|p| &p.operator) != prefs.as_ref().map(|p| &p.operator) {
        tailscale.denied = false;
    }
    tailscale.status = status;
    tailscale.prefs = prefs;
}

/// Follows the change stream until it ends; whether anything came of it.
fn watch() -> bool {
    let child = Command::new(BINARY)
        .args(["debug", "watch-ipn"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    let Some(stdout) = child.stdout.take() else {
        return false;
    };

    let (lines, changes) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.is_err() || lines.send(()).is_err() {
                break;
            }
        }
    });

    let mut heard = false;
    // a line is only "something changed": let the burst settle, then read
    while changes.recv().is_ok() {
        heard = true;
        thread::sleep(SETTLE);
        while changes.try_recv().is_ok() {}
        refresh();
    }
    let _ = child.kill();
    let _ = child.wait();
    heard
}

/// Runs a write off the UI thread; a refusal for want of the operator
/// grant turns the controls read-only, any other failure is shown.
fn write(args: Vec<String>) {
    thread::spawn(move || {
        let output = Command::new(BINARY)
            .args(&args)
            .stdin(Stdio::null())
            .output();
        let failure = match output {
            Ok(out) if out.status.success() => None,
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                Some(
                    first_line(&stderr)
                        .or_else(|| first_line(&stdout))
                        .unwrap_or("failed")
                        .to_string(),
                )
            }
            Err(e) => Some(e.to_string()),
        };
        {
            let mut tailscale = Tailscale::write();
            match failure {
                None => tailscale.error = None,
                Some(message) if refused(&message) => tailscale.denied = true,
                Some(message) => tailscale.error = Some(message),
            }
        }
        refresh();
    });
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

/// Whether a failure is the daemon's permission check.
fn refused(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "access denied",
        "not allowed",
        "permission denied",
        "must be run as root",
        "operator",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

// ───── parsing ───────────────────────────────────────────────────────────────

fn backend_state(state: &str) -> State {
    match state {
        "Running" => State::Running,
        "Starting" | "NoState" => State::Starting,
        // neither is fixable from Quick Settings but by logging in
        "NeedsLogin" | "NeedsMachineAuth" => State::NeedsLogin,
        _ => State::Stopped,
    }
}

fn string(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

/// The IPv4 address, else the first.
fn tailnet_ip(value: &Value) -> Option<String> {
    let ips: Vec<&str> = value["TailscaleIPs"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    ips.iter()
        .find(|ip| ip.contains('.'))
        .or(ips.first())
        .map(|ip| ip.to_string())
}

fn parse_status(json: &str) -> Option<Status> {
    let raw: Value = serde_json::from_str(json).ok()?;
    let me = &raw["Self"];

    let mut peers: Vec<Peer> = raw["Peer"]
        .as_object()
        .map(|peers| {
            peers
                .values()
                .map(|peer| Peer {
                    name: match string(peer, "HostName") {
                        name if name.is_empty() => string(peer, "DNSName"),
                        name => name,
                    },
                    dns: string(peer, "DNSName").trim_end_matches('.').to_string(),
                    ip: tailnet_ip(peer),
                    os: string(peer, "OS"),
                    online: peer["Online"].as_bool().unwrap_or(false),
                    exit_option: peer["ExitNodeOption"].as_bool().unwrap_or(false),
                    exit_active: peer["ExitNode"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Some(Status {
        state: backend_state(raw["BackendState"].as_str().unwrap_or_default()),
        hostname: string(me, "HostName"),
        ip: tailnet_ip(me),
        peers,
        health: raw["Health"]
            .as_array()
            .map(|health| {
                health
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        auth_url: raw["AuthURL"]
            .as_str()
            .filter(|url| !url.is_empty())
            .map(String::from),
    })
}

fn parse_prefs(json: &str) -> Option<Prefs> {
    let raw: Value = serde_json::from_str(json).ok()?;
    Some(Prefs {
        accept_routes: raw["RouteAll"].as_bool().unwrap_or(false),
        accept_dns: raw["CorpDNS"].as_bool().unwrap_or(false),
        shields_up: raw["ShieldsUp"].as_bool().unwrap_or(false),
        // left out of the JSON while unset
        operator: raw["OperatorUser"]
            .as_str()
            .filter(|user| !user.is_empty())
            .map(String::from),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNING: &str = include_str!("../../tests/fixtures/tailscale/status.json");
    const NEEDS_LOGIN: &str = include_str!("../../tests/fixtures/tailscale/needs-login.json");
    const PREFS: &str = include_str!("../../tests/fixtures/tailscale/prefs.json");

    #[test]
    fn a_running_tailnet_lists_its_peers_online_first() {
        let status = parse_status(RUNNING).unwrap();
        assert_eq!(status.state, State::Running);
        assert_eq!(status.hostname, "laptop");
        assert_eq!(status.ip.as_deref(), Some("100.64.0.1"));
        let names: Vec<&str> = status.peers.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Desktop", "server", "phone"]);
        // the IPv4 address even when it isn't listed first
        assert_eq!(status.peers[0].ip.as_deref(), Some("100.64.0.4"));
        assert_eq!(status.peers[1].dns, "server.example.ts.net");
        assert_eq!(status.auth_url, None);
    }

    #[test]
    fn exit_nodes_and_the_one_in_use() {
        let tailscale = Tailscale {
            status: parse_status(RUNNING).unwrap(),
            ..Tailscale::default()
        };
        let offered: Vec<&str> = tailscale.exit_nodes().map(|p| p.name.as_str()).collect();
        assert_eq!(offered, ["Desktop", "server"]);
        assert_eq!(
            tailscale.exit_node().map(|p| p.name.as_str()),
            Some("server")
        );
        assert_eq!(tailscale.describe(), "via server");
    }

    #[test]
    fn logged_out_has_a_url_once_a_login_started() {
        let status = parse_status(NEEDS_LOGIN).unwrap();
        assert_eq!(status.state, State::NeedsLogin);
        assert_eq!(status.ip, None);
        assert!(status.peers.is_empty());
        assert_eq!(status.health, ["You are logged out."]);
        assert_eq!(
            status.auth_url.as_deref(),
            Some("https://login.tailscale.com/a/abc123")
        );
    }

    #[test]
    fn prefs_carry_the_switches_and_the_grant() {
        let prefs = parse_prefs(PREFS).unwrap();
        assert!(prefs.accept_routes && !prefs.accept_dns && prefs.shields_up);
        assert_eq!(prefs.operator.as_deref(), Some("ocbk"));
        assert_eq!(
            parse_prefs(r#"{"RouteAll": false}"#).unwrap().operator,
            None
        );
        assert_eq!(parse_status("not json"), None);
    }

    #[test]
    fn backend_states_map_as_the_extension_does() {
        assert_eq!(backend_state("NoState"), State::Starting);
        assert_eq!(backend_state("NeedsMachineAuth"), State::NeedsLogin);
        assert_eq!(backend_state("Stopped"), State::Stopped);
        assert_eq!(backend_state("Weird"), State::Stopped);
    }

    #[test]
    fn a_refusal_is_told_from_other_failures() {
        assert!(refused(
            "Access denied: prefs write access denied, try 'sudo tailscale set --operator=$USER'"
        ));
        assert!(!refused("invalid value for --exit-node"));
    }
}
