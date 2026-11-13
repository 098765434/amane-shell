//! A Needle fine-tune on another machine, followed from the status bar:
//! an activity with the step, the loss and the time left, an alert when it
//! ends with how it scored, and Stop and Log buttons. `--plain` prints the
//! same to the terminal instead.
//!
//!     emaki-train-watch HOST RUN [--plain]
//!
//! The run is one `apps/assistant/train/remote.sh run` started on HOST: its
//! log is read over Tailscale SSH (`~/needle-ft/runs/RUN/train.log`) from the
//! start, again after a dropped connection. A copy is kept in
//! `$XDG_CACHE_HOME/emaki-train-watch/RUN.log`, which Log opens.

// no unsafe code: what needs it (syscalls, FFI) lives in emaki-base or amane
#![forbid(unsafe_code)]
mod run_log;

use std::fs::File;
use std::io::{BufRead, BufReader, Write};

use emaki_process::Cmd;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use emaki_status::{
    ActivityButton, ActivityContent, ActivityEvent, Dismiss, ItemIcon, LiveActivity, Tint, Trailing,
};

use run_log::{Phase, Run};

/// How long a finished run waits for its dev score (`remote.sh score`).
const SCORE_WAIT: Duration = Duration::from_secs(15 * 60);
/// Quiet this long, it asks whether the run is still going.
const QUIET: Duration = Duration::from_secs(120);

enum Message {
    Line(String),
    /// The SSH connection ended.
    Gone,
    Action(String),
}

fn main() {
    emaki_log::init(emaki_base::app_ids::TRAIN_WATCH);
    emaki_base::migrate::from_amane();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let plain = args.iter().any(|a| a == "--plain");
    let names: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [host, name] = names[..] else {
        eprintln!("usage: emaki-train-watch HOST RUN [--plain]");
        std::process::exit(2);
    };
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        eprintln!("emaki-train-watch: {name:?} isn't a run name");
        std::process::exit(2);
    }
    let watch = Watch::new(host.clone(), name.clone(), plain);
    std::process::exit(watch.run());
}

struct Watch {
    host: String,
    name: String,
    copy: PathBuf,
    activity: Option<LiveActivity>,
    // asked "stop it?" and waiting for the answer
    asking: bool,
    // what the terminal was last told
    printed: String,
    // what the activity was last sent, and when: an update goes out only when it changes, and
    // at most once a SEND_EVERY, so a log printing many lines a second doesn't redraw the bar
    // for each
    sent: Option<(ActivityContent, Instant)>,
    // a change held back until SEND_EVERY has passed
    held: Option<ActivityContent>,
}

/// The least time between two activity updates that aren't alerts.
const SEND_EVERY: Duration = Duration::from_secs(1);

impl Watch {
    fn new(host: String, name: String, plain: bool) -> Self {
        let cache = emaki_base::paths::cache().join(emaki_base::app_ids::TRAIN_WATCH);
        // the log is written below, which says so if the folder couldn't be made
        let _ = std::fs::create_dir_all(&cache);
        let copy = cache.join(format!("{name}.log"));
        let mut watch = Self {
            host,
            name,
            copy,
            activity: None,
            asking: false,
            printed: String::new(),
            sent: None,
            held: None,
        };
        if !plain {
            watch.activity = Some(LiveActivity::start(watch.content(&Run::default())));
        }
        watch
    }

    /// Follows the run to its end; the exit status to leave with.
    fn run(mut self) -> i32 {
        let (tx, rx) = mpsc::channel();
        if let Some(activity) = &self.activity {
            let tx = tx.clone();
            activity.on_event(move |event| match event {
                ActivityEvent::Action(action) => {
                    // the watcher has gone
                    let _ = tx.send(Message::Action(action));
                }
                ActivityEvent::Dismissed => {
                    // the watcher has gone
                    let _ = tx.send(Message::Action(String::from("dismissed")));
                }
            });
        }

        let mut run = Run::default();
        let mut copy = File::create(&self.copy).ok();
        let mut backoff = Duration::from_secs(2);
        let mut heard = Instant::now();
        let mut ended: Option<Instant> = None;
        self.follow(tx.clone());

        loop {
            let wait = match ended {
                Some(at) => SCORE_WAIT.saturating_sub(at.elapsed()),
                None => QUIET,
            };
            // a change held back may be due before the rest
            let held = self.send_held().filter(|held| *held < wait);
            match rx.recv_timeout(held.unwrap_or(wait)) {
                Ok(Message::Line(line)) => {
                    heard = Instant::now();
                    backoff = Duration::from_secs(2);
                    if let Some(copy) = &mut copy {
                        let _ = writeln!(copy, "{line}");
                    }
                    let was = run.phase;
                    let scored = run.dev.is_some();
                    if !run.feed(&line) {
                        continue;
                    }
                    let ending = run.finished() && !matches!(was, Phase::Done | Phase::Failed);
                    let alert = ending || (!scored && run.dev.is_some());
                    self.show(&run, alert);
                    if ending {
                        ended = Some(Instant::now());
                    }
                    if run.phase == Phase::Failed || run.dev.is_some() {
                        return self.end(&run);
                    }
                }
                Ok(Message::Gone) => {
                    if ended.is_some() {
                        return self.end(&run);
                    }
                    // read it all again from the start, once the connection's back
                    std::thread::sleep(backoff);
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                    run = Run::default();
                    copy = File::create(&self.copy).ok();
                    self.follow(tx.clone());
                }
                Ok(Message::Action(action)) => match action.as_str() {
                    "stop" => {
                        self.asking = true;
                        self.show(&run, true);
                    }
                    "keep" => {
                        self.asking = false;
                        self.show(&run, false);
                    }
                    "stop-now" => {
                        self.asking = false;
                        let stopped = self.stop();
                        run.phase = Phase::Failed;
                        run.last = match stopped {
                            Ok(()) => String::from("stopped"),
                            Err(error) => format!("couldn't stop it: {error}"),
                        };
                        self.show(&run, false);
                        return self.end(&run);
                    }
                    "log" => {
                        if let Err(error) = Cmd::new("xdg-open").arg(&self.copy).detach() {
                            log::warn!("couldn't open the log: {error}");
                        }
                    }
                    "dismissed" => {
                        // the activity's gone from the bar: nothing left to show it in
                        self.activity = None;
                        return 0;
                    }
                    _ => {}
                },
                // a held change's time came: it goes out at the top of the loop
                Err(RecvTimeoutError::Timeout) if held.is_some() => {}
                Err(RecvTimeoutError::Timeout) if ended.is_some() => return self.end(&run),
                Err(RecvTimeoutError::Timeout) => {
                    if heard.elapsed() >= QUIET && !self.going() && !run.finished() {
                        run.phase = Phase::Failed;
                        run.last = String::from("the run stopped without finishing");
                        self.show(&run, true);
                        return self.end(&run);
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return self.end(&run),
            }
        }
    }

    /// Reads the log over SSH on a thread of its own, from its first line.
    fn follow(&self, tx: Sender<Message>) {
        let path = format!("needle-ft/runs/{}/train.log", self.name);
        let child = Cmd::new("tailscale")
            .args(["ssh", &self.host, "--", "tail", "-n", "+1", "-F", &path])
            .spawn();
        std::thread::spawn(move || {
            match child {
                Ok(mut running) => {
                    if let Some(out) = running.take_stdout() {
                        for line in BufReader::new(out).lines().map_while(Result::ok) {
                            if tx.send(Message::Line(line)).is_err() {
                                // nobody reads the log any more
                                running.stopper().stop();
                                break;
                            }
                        }
                    }
                    // tail -F only ends when ssh does; how is in the run's state
                    let _ = running.finish();
                }
                Err(error) => log::warn!("couldn't follow the log: {error}"),
            }
            // the watcher may have gone already
            let _ = tx.send(Message::Gone);
        });
    }

    /// Whether the run's unit is still going on the host.
    fn going(&self) -> bool {
        Cmd::new("tailscale")
            .args(["ssh", &self.host, "--", "systemctl", "--user", "is-active"])
            .arg("--quiet")
            .arg(format!("needle-ft-{}", self.name))
            .status()
            .is_ok()
    }

    fn stop(&self) -> Result<(), String> {
        Cmd::new("tailscale")
            .args(["ssh", &self.host, "--", "systemctl", "--user", "stop"])
            .arg(format!("needle-ft-{}", self.name))
            .output()
            .map(|_| ())
            .map_err(|failure| failure.message)
    }

    fn show(&mut self, run: &Run, alert: bool) {
        match &self.activity {
            Some(_) => {
                let content = self.content(run);
                let same = self.sent.as_ref().is_some_and(|(sent, _)| *sent == content);
                let soon = self
                    .sent
                    .as_ref()
                    .is_some_and(|(_, at)| at.elapsed() < SEND_EVERY);
                if alert || (!same && !soon) {
                    self.send(content, alert);
                } else if !same {
                    self.held = Some(content);
                } else {
                    self.held = None;
                }
            }
            None => {
                let line = format!("{}  {}", self.title(run), run.summary());
                if line != self.printed {
                    println!("{line}");
                    self.printed = line;
                }
            }
        }
    }

    fn send(&mut self, content: ActivityContent, alert: bool) {
        if let Some(activity) = &self.activity {
            activity.update(content.clone(), alert);
        }
        self.sent = Some((content, Instant::now()));
        self.held = None;
    }

    /// Sends a held-back change once its time has come; how long until then otherwise.
    fn send_held(&mut self) -> Option<Duration> {
        self.held.as_ref()?;
        let since = self
            .sent
            .as_ref()
            .map_or(SEND_EVERY, |(_, at)| at.elapsed());
        if since >= SEND_EVERY {
            if let Some(content) = self.held.take() {
                self.send(content, false);
            }
            return None;
        }
        Some(SEND_EVERY - since)
    }

    fn end(&mut self, run: &Run) -> i32 {
        // the last state goes out before the end
        if let Some(content) = self.held.take() {
            self.send(content, false);
        }
        if let Some(activity) = self.activity.take() {
            let lasts = if run.phase == Phase::Done { 30 } else { 12 };
            activity.end(Dismiss::After(lasts));
            // the end goes out on amane's worker; give it a moment before leaving
            std::thread::sleep(Duration::from_millis(500));
        }
        if run.phase == Phase::Done { 0 } else { 1 }
    }

    fn title(&self, run: &Run) -> String {
        let name = &self.name;
        match run.phase {
            Phase::Starting | Phase::Training | Phase::Scoring | Phase::Building => {
                format!("Training Needle · {name}")
            }
            Phase::Done => format!("Needle {name} trained"),
            Phase::Failed if run.last == "stopped" => format!("Stopped {name}"),
            Phase::Failed => format!("Training {name} failed"),
        }
    }

    fn content(&self, run: &Run) -> ActivityContent {
        let glyph = match run.phase {
            Phase::Done => "check-circle",
            Phase::Failed => "error",
            _ => "update",
        };
        let mut content = ActivityContent::new(
            "Training",
            ItemIcon::Glyph(String::from(glyph)),
            &self.title(run),
        );
        content.subtitle = run.summary();
        content.relevance = 0.6;
        content.progress = run.progress();
        content.trailing = match run.phase {
            Phase::Training => Trailing::Progress(run.progress().unwrap_or(0.0)),
            Phase::Starting => Trailing::Text(String::from("Starting")),
            Phase::Scoring => Trailing::Text(String::from("Scoring")),
            Phase::Building => Trailing::Text(String::from("Building")),
            Phase::Done => {
                Trailing::Text(run.accuracy.clone().unwrap_or_else(|| String::from("Done")))
            }
            Phase::Failed => Trailing::Text(String::from(if run.last == "stopped" {
                "Stopped"
            } else {
                "Failed"
            })),
        };
        content.tint = match run.phase {
            Phase::Done => Some(Tint::Green),
            Phase::Failed if run.last != "stopped" => Some(Tint::Red),
            _ => None,
        };
        let log = ActivityButton::new("log", "Log");
        content.buttons = if run.finished() {
            vec![log]
        } else if self.asking {
            content.subtitle = String::from("Stop this run? What it learned so far is lost.");
            vec![
                ActivityButton::new("keep", "Keep going"),
                ActivityButton {
                    destructive: true,
                    ..ActivityButton::new("stop-now", "Stop")
                },
            ]
        } else {
            vec![
                log,
                ActivityButton {
                    destructive: true,
                    ..ActivityButton::new("stop", "Stop…")
                },
            ]
        };
        content
    }
}
