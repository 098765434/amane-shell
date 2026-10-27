//! The NixOS configuration as Settings sees it: the [`Snapshot`] the flake's
//! `amaneSettings` output gives (every setting in `managed/registry.nix`
//! with its effective value, how it's set and which files set it), and
//! [`Managed`], `managed/settings.json`, the one file Settings writes.
//! `modules/managed.nix` turns that file into option definitions, so the
//! hand-written modules are never touched.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Service, watch_file};
use amane_kit::settings::nixos;
use serde_json::Value;

/// The file Settings writes, in the repo.
pub const SETTINGS_FILE: &str = "managed/settings.json";

/// The module that reads it: a setting only it sets is ours to change.
pub const MANAGED_MODULE: &str = "modules/managed.nix";

/// The flake repo, from `nixos.json`.
pub fn repo() -> PathBuf {
    nixos().repo_path()
}

/// The `nixosConfigurations` attribute to build, from `nixos.json`.
pub fn host() -> String {
    nixos().host_attr()
}

/// Runs a program to its end (so off the UI thread): what it printed on
/// success, else the last lines of what it said.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).env("NO_COLOR", "1");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| format!("{program}: {error}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(failure(
            &output.stderr,
            &output.stdout,
            output.status.code(),
        ))
    }
}

/// The last lines a failed command said: nix puts the error itself last.
pub fn failure(stderr: &[u8], stdout: &[u8], code: Option<i32>) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    let said: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("warning: Git tree"))
        .collect();

    if said.is_empty() {
        return match code {
            Some(code) => format!("exited with {code}"),
            None => String::from("stopped by a signal"),
        };
    }

    said[said.len().saturating_sub(12)..].join("\n")
}

/// Runs git in the repo.
pub fn git(args: &[&str]) -> Result<String, String> {
    run("git", args, Some(&repo()))
}

/// Runs nix in the repo, without the dirty-tree warning.
pub fn nix(args: &[&str]) -> Result<String, String> {
    let mut all = vec!["--no-warn-dirty"];
    all.extend_from_slice(args);
    run("nix", &all, Some(&repo()))
}

// ───── the snapshot ──────────────────────────────────────────────────────────

/// How a setting is changed, from the registry's `control`.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Switch,
    Int { min: i64, max: i64 },
    TimeZone,
    Text,
}

/// When a change takes effect, from the registry's `tier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Rebuild,
    Restart,
}

impl Tier {
    pub fn note(self) -> &'static str {
        match self {
            Self::Rebuild => "Takes effect after Apply",
            Self::Restart => "Needs a restart",
        }
    }
}

/// One setting from the registry, with what the configuration makes of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub page: String,
    pub label: String,
    pub control: Control,
    pub tier: Tier,
    pub value: Value,
    pub default: Value,
    pub description: String,
    /// The repo files that define the option; ours is `modules/managed.nix`.
    pub set_in: Vec<String>,
}

impl Entry {
    /// Whether changing `settings.json` changes it: nothing but
    /// `managed.nix` sets it. Another file's definition would win over
    /// ours, or clash with it.
    pub fn managed(&self) -> bool {
        self.set_in.iter().all(|file| file == MANAGED_MODULE)
    }

    /// The other files that set it.
    pub fn set_elsewhere(&self) -> Vec<&str> {
        self.set_in
            .iter()
            .filter(|file| *file != MANAGED_MODULE)
            .map(String::as_str)
            .collect()
    }

    /// Whether a value is one the control takes, before it's written.
    pub fn check(&self, value: &Value) -> Result<(), String> {
        match (&self.control, value) {
            (Control::Switch, Value::Bool(_)) => Ok(()),
            (Control::Int { min, max }, Value::Number(n)) => match n.as_i64() {
                Some(n) if (*min..=*max).contains(&n) => Ok(()),
                _ => Err(format!("{} takes {min} to {max}", self.label)),
            },
            (Control::TimeZone, Value::String(zone)) => {
                let name = zone.trim();
                let fine = !name.is_empty()
                    && !name.starts_with('/')
                    && !name.contains("..")
                    && Path::new("/etc/zoneinfo").join(name).is_file();
                if fine {
                    Ok(())
                } else {
                    Err(format!("No time zone named {name}"))
                }
            }
            (Control::Text, Value::String(_)) => Ok(()),
            _ => Err(format!("{} can't be set to {}", self.label, show(value))),
        }
    }
}

/// A value as a label says it.
pub fn show(value: &Value) -> String {
    match value {
        Value::Null => String::from("unset"),
        Value::Bool(true) => String::from("on"),
        Value::Bool(false) => String::from("off"),
        Value::String(text) => text.clone(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

fn entry(value: &Value) -> Option<Entry> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);
    let control = match text("control")?.as_str() {
        "switch" => Control::Switch,
        "int" => Control::Int {
            min: value.get("min").and_then(Value::as_i64).unwrap_or(0),
            max: value.get("max").and_then(Value::as_i64).unwrap_or(i64::MAX),
        },
        "timezone" => Control::TimeZone,
        _ => Control::Text,
    };
    let tier = match text("tier").as_deref() {
        Some("restart") => Tier::Restart,
        _ => Tier::Rebuild,
    };

    Some(Entry {
        id: text("id")?,
        page: text("page").unwrap_or_default(),
        label: text("label").unwrap_or_else(|| text("id").unwrap_or_default()),
        control,
        tier,
        value: value.get("value").cloned().unwrap_or(Value::Null),
        default: value.get("default").cloned().unwrap_or(Value::Null),
        description: text("description").unwrap_or_default(),
        set_in: value
            .get("setIn")
            .and_then(Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// What `nix eval --json .#amaneSettings` gave.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    pub entries: Vec<Entry>,
    pub revision: Option<String>,
    pub host: Option<String>,
}

pub fn parse_snapshot(json: &str) -> Result<Parsed, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| format!("amaneSettings: {error}"))?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("amaneSettings has no entries")?
        .iter()
        .filter_map(entry)
        .collect();

    Ok(Parsed {
        entries,
        revision: value
            .get("revision")
            .and_then(Value::as_str)
            .map(String::from),
        host: value.get("host").and_then(Value::as_str).map(String::from),
    })
}

/// The settings the configuration has, as last evaluated. Taken once at
/// start and again on [`Snapshot::refresh`]; stale while `loading`.
pub struct Snapshot {
    pub entries: Vec<Entry>,
    /// The commit the repo was at, `-dirty` when it had changes.
    pub revision: Option<String>,
    pub host: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
    pub taken: Option<Instant>,
}

// which refresh is the latest, so an older one that ends late is dropped
static TAKE: AtomicU64 = AtomicU64::new(0);

impl Service for Snapshot {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            revision: None,
            host: None,
            error: None,
            loading: true,
            taken: None,
        }
    }

    // the first evaluation, off the UI thread; later ones come from refresh
    fn listen() {
        take(TAKE.fetch_add(1, Ordering::SeqCst) + 1);
    }
}

impl Snapshot {
    /// Evaluates again, in the background.
    pub fn refresh() {
        let take_number = TAKE.fetch_add(1, Ordering::SeqCst) + 1;
        Self::write().loading = true;
        thread::spawn(move || take(take_number));
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn for_page(&self, page: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.page == page)
            .collect()
    }
}

fn take(take_number: u64) {
    let flake = format!("{}#amaneSettings", repo().display());
    let result = nix(&["eval", "--json", &flake]).and_then(|json| parse_snapshot(&json));

    if TAKE.load(Ordering::SeqCst) != take_number {
        return;
    }

    let mut now = Snapshot::write();
    now.loading = false;
    now.taken = Some(Instant::now());
    match result {
        Ok(parsed) => {
            now.entries = parsed.entries;
            now.revision = parsed.revision;
            now.host = parsed.host;
            now.error = None;
        }
        Err(error) => now.error = Some(error),
    }
}

// ───── managed/settings.json ─────────────────────────────────────────────────

/// A setting that differs between the working tree and HEAD: what Apply
/// would make permanent.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub id: String,
    pub from: Option<Value>,
    pub to: Option<Value>,
}

/// `managed/settings.json` as it is on disk (`current`) and as last
/// committed (`committed`): the difference is what's waiting for Apply.
#[derive(Clone, Debug, PartialEq)]
pub struct Managed {
    pub current: BTreeMap<String, Value>,
    pub committed: BTreeMap<String, Value>,
    pub error: Option<String>,
}

pub fn settings_path() -> PathBuf {
    repo().join(SETTINGS_FILE)
}

fn parse_map(json: &str) -> Result<BTreeMap<String, Value>, String> {
    let value: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    match value {
        Value::Object(map) => Ok(map.into_iter().collect()),
        Value::Null => Ok(BTreeMap::new()),
        _ => Err(String::from("not an object")),
    }
}

fn look() -> Managed {
    let path = settings_path();
    let (current, error) = match fs::read_to_string(&path) {
        Ok(json) => match parse_map(&json) {
            Ok(map) => (map, None),
            Err(error) => (BTreeMap::new(), Some(format!("{SETTINGS_FILE}: {error}"))),
        },
        Err(_) => (BTreeMap::new(), None),
    };
    let committed = git(&["show", &format!("HEAD:{SETTINGS_FILE}")])
        .ok()
        .and_then(|json| parse_map(&json).ok())
        .unwrap_or_default();

    Managed {
        current,
        committed,
        error,
    }
}

impl Service for Managed {
    // both reads take a few milliseconds
    fn new() -> Self {
        look()
    }

    // a write from here, by hand, or a checkout: the file's folder is watched
    fn listen() {
        loop {
            let path = settings_path();
            let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
            if !folder.is_dir() {
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            for _ in watch_file(&path.to_string_lossy()) {
                Self::reload();
            }
        }
    }
}

impl Managed {
    /// Reads the file and HEAD's copy again.
    pub fn reload() {
        let fresh = look();
        let mut now = Self::write();
        if fresh == *now {
            now.quiet();
        } else {
            *now = fresh;
        }
    }

    /// The value shown for a setting: the file's, else the configuration's.
    pub fn value<'a>(&'a self, id: &str, configured: &'a Value) -> &'a Value {
        self.current.get(id).unwrap_or(configured)
    }

    /// What differs from HEAD.
    pub fn changes(&self) -> Vec<Change> {
        let mut ids: Vec<&String> = self.current.keys().chain(self.committed.keys()).collect();
        ids.sort();
        ids.dedup();

        ids.into_iter()
            .filter(|id| self.current.get(*id) != self.committed.get(*id))
            .map(|id| Change {
                id: id.clone(),
                from: self.committed.get(id).cloned(),
                to: self.current.get(id).cloned(),
            })
            .collect()
    }

    /// Each change as a few words, with the registry's labels where the
    /// snapshot has them: "Time zone → Europe/Oslo", "Tailscale off".
    pub fn summary(&self, snapshot: &Snapshot) -> Vec<String> {
        self.changes()
            .iter()
            .map(|change| {
                let label = snapshot
                    .entry(&change.id)
                    .map_or(change.id.clone(), |entry| entry.label.clone());
                match &change.to {
                    Some(Value::Bool(on)) => format!("{label} {}", if *on { "on" } else { "off" }),
                    Some(value) => format!("{label} → {}", show(value)),
                    None => format!("{label} unset"),
                }
            })
            .collect()
    }

    /// The commit's subject for the changes.
    pub fn commit_message(&self, snapshot: &Snapshot) -> String {
        format!("settings: {}", self.summary(snapshot).join(", "))
    }

    /// Writes one setting and stages the file, so the flake sees it. The
    /// caller has checked the value against its entry.
    pub fn set(id: &str, value: Value) {
        let mut map = Self::read().current.clone();
        map.insert(String::from(id), value);
        let path = settings_path();

        thread::spawn(move || {
            let result = save(&path, &map).and_then(|()| git(&["add", "--", SETTINGS_FILE]));
            Self::reload();
            if let Err(error) = result {
                Self::write().error = Some(error);
            }
        });
    }

    /// Puts HEAD's copy back.
    pub fn discard() {
        thread::spawn(|| {
            let result = git(&["checkout", "--", SETTINGS_FILE]);
            Self::reload();
            if let Err(error) = result {
                Self::write().error = Some(error);
            }
        });
    }
}

/// Writes the map as the file, whole, through a rename.
fn save(path: &Path, map: &BTreeMap<String, Value>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(map).map_err(|error| error.to_string())? + "\n";
    let new = path.with_extension("json.new");
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    fs::write(&new, json).map_err(|error| error.to_string())?;
    fs::rename(&new, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = r#"{"entries":[
        {"control":"timezone","default":null,"description":"","id":"timeZone","label":"Time zone",
         "page":"about","path":["time","timeZone"],"setIn":["modules/managed.nix"],"tier":"rebuild",
         "type":"null or string","value":"Europe/Oslo"},
        {"control":"switch","default":false,"id":"tailscale","label":"Tailscale","page":"about",
         "path":["services","tailscale","enable"],"setIn":["modules/services.nix"],"tier":"rebuild",
         "type":"boolean","value":true},
        {"control":"int","default":null,"id":"bootEntries","label":"Boot entries kept","max":50,"min":1,
         "page":"storage","path":["boot","loader","systemd-boot","configurationLimit"],
         "setIn":["modules/managed.nix"],"tier":"restart","type":"null or signed integer","value":10}
    ],"host":"nixos-niri","revision":"5d056b8-dirty"}"#;

    fn snapshot() -> Snapshot {
        let parsed = parse_snapshot(SNAPSHOT).unwrap();
        Snapshot {
            entries: parsed.entries,
            revision: parsed.revision,
            host: parsed.host,
            error: None,
            loading: false,
            taken: None,
        }
    }

    #[test]
    fn the_snapshot_says_what_each_setting_is_and_who_sets_it() {
        let snapshot = snapshot();
        assert_eq!(snapshot.entries.len(), 3);
        assert_eq!(snapshot.revision.as_deref(), Some("5d056b8-dirty"));

        let zone = snapshot.entry("timeZone").unwrap();
        assert_eq!(zone.control, Control::TimeZone);
        assert!(zone.managed());

        let tailscale = snapshot.entry("tailscale").unwrap();
        assert!(!tailscale.managed());
        assert_eq!(tailscale.set_elsewhere(), vec!["modules/services.nix"]);

        let boot = snapshot.entry("bootEntries").unwrap();
        assert_eq!(boot.control, Control::Int { min: 1, max: 50 });
        assert_eq!(boot.tier, Tier::Restart);
        assert_eq!(snapshot.for_page("storage").len(), 1);
    }

    #[test]
    fn a_value_is_checked_against_its_control() {
        let snapshot = snapshot();
        let boot = snapshot.entry("bootEntries").unwrap();
        assert!(boot.check(&Value::from(8)).is_ok());
        assert!(boot.check(&Value::from(0)).is_err());
        assert!(boot.check(&Value::from("ten")).is_err());

        let tailscale = snapshot.entry("tailscale").unwrap();
        assert!(tailscale.check(&Value::Bool(false)).is_ok());
        assert!(tailscale.check(&Value::from(1)).is_err());

        let zone = snapshot.entry("timeZone").unwrap();
        assert!(zone.check(&Value::from("../etc/passwd")).is_err());
        assert!(zone.check(&Value::from("")).is_err());
    }

    #[test]
    fn the_changes_are_what_differs_from_head_and_read_as_words() {
        let managed = Managed {
            current: parse_map(
                r#"{"timeZone":"Europe/Berlin","tailscale":false,"bootEntries":10}"#,
            )
            .unwrap(),
            committed: parse_map(r#"{"timeZone":"Europe/Oslo","tailscale":true,"bootEntries":10}"#)
                .unwrap(),
            error: None,
        };
        let changes = managed.changes();
        assert_eq!(changes.len(), 2);
        assert_eq!(
            managed.commit_message(&snapshot()),
            "settings: Tailscale off, Time zone → Europe/Berlin"
        );

        let same = Managed {
            committed: managed.current.clone(),
            ..managed
        };
        assert!(same.changes().is_empty());
    }

    #[test]
    fn a_missing_file_is_empty_and_a_broken_one_says_so() {
        assert!(parse_map("null").unwrap().is_empty());
        assert!(parse_map("[1]").is_err());
        assert!(parse_map("{").is_err());
    }
}
