//! The pipeline behind Apply: build the system as the user (nix reports
//! progress as JSON lines), compare it with the running one, activate it
//! as root through polkit, then commit what changed. Check for Updates is
//! the same pipeline from a fresh `flake.lock` written beside the repo,
//! which goes into the repo once the update is applied. One job at a time,
//! on its own threads; the window reads its [`Phase`].

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Notify, Service};
use serde_json::Value;

use super::generations::Generations;
use super::nixos::{Managed, Snapshot, failure, git, host, repo};
use super::repo::{Applied, Repo, Running, Toplevel};
use super::state_dir;
use super::update::{self, InputChange};

/// Where the helper is in the running system; polkit's action names the
/// store path it resolves to.
const HELPER: &str = "/run/current-system/sw/bin/amane-nixos-activate";

/// How many lines of the build log the window keeps.
const LOG_LINES: usize = 300;

/// How often progress reaches the window at most.
const REDRAW: Duration = Duration::from_millis(100);

/// How a built system is activated: `switch` and `boot` record a
/// generation, `test` doesn't, so a restart undoes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Switch,
    Boot,
    Test,
}

impl Mode {
    pub fn arg(self) -> &'static str {
        match self {
            Self::Switch => "switch",
            Self::Boot => "boot",
            Self::Test => "test",
        }
    }

    /// Whether it makes a generation (and so is worth a commit).
    pub fn records(self) -> bool {
        self != Self::Test
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Lock,
    Eval,
    Build,
    Compare,
    Activate,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Self::Lock => "Checking for updates",
            Self::Eval => "Evaluation",
            Self::Build => "Build",
            Self::Compare => "Comparing",
            Self::Activate => "Applying",
        }
    }
}

/// How far a build is, from nix's `builds` activity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub done: u64,
    pub expected: u64,
    pub running: u64,
    pub failed: u64,
    /// What nix is on: "building amane-shell", "fetching linux-7.2.10".
    pub current: Option<String>,
    /// Before anything builds or downloads: nix is still evaluating.
    pub evaluating: bool,
}

impl Progress {
    /// 0–100.
    pub fn percent(&self) -> u8 {
        if self.expected == 0 {
            return 0;
        }
        ((self.done * 100) / self.expected).min(100) as u8
    }
}

/// One line of `nix store diff-closures`.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine {
    pub name: String,
    pub from: String,
    pub to: String,
    pub size: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Removed,
    Changed,
}

impl DiffLine {
    pub fn kind(&self) -> Kind {
        match (self.from.as_str(), self.to.as_str()) {
            ("∅", _) => Kind::Added,
            (_, "∅") => Kind::Removed,
            _ => Kind::Changed,
        }
    }
}

/// A system that's built and compared, waiting to be activated.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    pub out: PathBuf,
    pub diff: Vec<DiffLine>,
    /// What diff-closures said, for the log when the lines didn't parse.
    pub raw_diff: String,
    /// Its kernel, initrd or modules aren't the booted ones.
    pub needs_restart: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    /// A check found nothing newer than the lock.
    UpToDate,
    Building(Progress),
    Built(Built),
    Activating {
        mode: Mode,
    },
    Done {
        mode: Mode,
        generation: Option<u32>,
        /// The commit made, or why there's none.
        commit: Option<Result<String, String>>,
        /// The packages this Apply added in Settings, now installed.
        installed: Vec<String>,
    },
    Failed {
        stage: Stage,
        message: String,
    },
}

/// What's being built: the working tree, an old generation to go back to,
/// or the working tree with a newer `flake.lock`.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Head,
    Generation(PathBuf),
    Update {
        /// The fresh lock, in the state folder until it's applied.
        lock: PathBuf,
        /// The inputs asked for; all when empty.
        inputs: Vec<String>,
        /// What the fresh lock moves.
        changes: Vec<InputChange>,
    },
}

impl Target {
    /// Whether it's the working tree, with or without a newer lock: what
    /// a commit records.
    pub fn is_ours(&self) -> bool {
        matches!(self, Self::Head | Self::Update { .. })
    }
}

pub struct BuildJob {
    pub phase: Phase,
    pub started: Option<Instant>,
    pub log: VecDeque<String>,
    pub target: Target,
    /// Activate as soon as it's built (Try, Restore).
    then: Option<Mode>,
}

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static CANCELLED: AtomicBool = AtomicBool::new(false);

impl Service for BuildJob {
    fn new() -> Self {
        Self {
            phase: Phase::Idle,
            started: None,
            log: VecDeque::new(),
            target: Target::Head,
            then: None,
        }
    }

    // only its own threads change it
    fn listen() {}
}

impl BuildJob {
    /// Whether a build or activation is under way: nothing else may start,
    /// and the window stays open.
    pub fn running(&self) -> bool {
        matches!(self.phase, Phase::Building(_) | Phase::Activating { .. })
    }

    /// Whether there's anything to show: a job, its result, or its failure.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// "1:12" since the job started.
    pub fn elapsed(&self) -> String {
        let seconds = self.started.map_or(0, |at| at.elapsed().as_secs());
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }

    /// Builds the working tree; `then` activates it as soon as it's built.
    pub fn start_build(then: Option<Mode>) {
        {
            let mut job = Self::write();
            if job.running() {
                job.quiet();
                return;
            }
            job.phase = Phase::Building(Progress {
                evaluating: true,
                ..Progress::default()
            });
            job.started = Some(Instant::now());
            job.log.clear();
            job.target = Target::Head;
            job.then = then;
        }
        CANCELLED.store(false, Ordering::SeqCst);

        thread::spawn(|| match build(None) {
            Ok(out) => compare_then(out),
            Err(Some((stage, message))) => fail(stage, message),
            // cancelled
            Err(None) => Self::write().phase = Phase::Idle,
        });
    }

    /// Writes a fresh lock for `inputs` (all when empty) beside the repo,
    /// and builds the working tree with it when something moved.
    pub fn start_update(inputs: Vec<String>) {
        let lock = state_dir().join("update.lock");
        {
            let mut job = Self::write();
            if job.running() {
                job.quiet();
                return;
            }
            job.phase = Phase::Building(Progress {
                current: Some(String::from("checking for updates")),
                ..Progress::default()
            });
            job.started = Some(Instant::now());
            job.log.clear();
            job.target = Target::Update {
                lock: lock.clone(),
                inputs: inputs.clone(),
                changes: Vec::new(),
            };
            job.then = None;
        }
        CANCELLED.store(false, Ordering::SeqCst);

        thread::spawn(move || {
            let changes = match relock(&lock, &inputs) {
                Ok(changes) => changes,
                Err(Some((stage, message))) => return fail(stage, message),
                Err(None) => return Self::write().phase = Phase::Idle,
            };
            if changes.is_empty() {
                let mut job = Self::write();
                job.note("Every input is at its newest.");
                job.phase = Phase::UpToDate;
                drop(job);
                // the scheduled check's record is stale now
                if inputs.is_empty() {
                    update::Check::clear();
                }
                return;
            }
            {
                let mut job = Self::write();
                for change in &changes {
                    job.note(change.text());
                }
                if let Target::Update { changes: known, .. } = &mut job.target {
                    *known = changes;
                }
                job.phase = Phase::Building(Progress {
                    evaluating: true,
                    ..Progress::default()
                });
            }
            match build(Some(&lock)) {
                Ok(out) => compare_then(out),
                Err(Some((stage, message))) => fail(stage, message),
                Err(None) => Self::write().phase = Phase::Idle,
            }
        });
    }

    /// Goes back to a generation: compares it with the running system,
    /// then activates it the given way.
    pub fn restore(toplevel: PathBuf, mode: Mode) {
        {
            let mut job = Self::write();
            if job.running() {
                job.quiet();
                return;
            }
            job.phase = Phase::Building(Progress {
                current: Some(String::from("comparing with the running system")),
                ..Progress::default()
            });
            job.started = Some(Instant::now());
            job.log.clear();
            job.target = Target::Generation(toplevel.clone());
            job.then = Some(mode);
        }

        thread::spawn(move || compare_then(toplevel));
    }

    /// Stops the build.
    pub fn cancel() {
        CANCELLED.store(true, Ordering::SeqCst);
        if let Some(child) = CHILD
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            let _ = child.kill();
        }
    }

    /// Activates the built system, as root, through polkit.
    pub fn activate(mode: Mode) {
        let out = {
            let mut job = Self::write();
            let Phase::Built(built) = &job.phase else {
                job.quiet();
                return;
            };
            let out = built.out.clone();
            job.phase = Phase::Activating { mode };
            out
        };

        thread::spawn(move || match switch(&out, mode) {
            Ok(()) => finish(out, mode),
            Err(message) => fail(Stage::Activate, message),
        });
    }

    /// Puts a finished or failed job away.
    pub fn dismiss() {
        let mut job = Self::write();
        if job.running() {
            job.quiet();
            return;
        }
        job.phase = Phase::Idle;
    }

    fn note(&mut self, line: impl Into<String>) {
        if self.log.len() == LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(line.into());
    }
}

fn fail(stage: Stage, message: String) {
    let mut job = BuildJob::write();
    job.note(format!("{}: {message}", stage.name()));
    job.phase = Phase::Failed { stage, message };
    job.then = None;
}

// ───── building ──────────────────────────────────────────────────────────────

/// `nix flake update` into `lock`, without touching the repo's: what it
/// moves against the repo's lock. None when cancelled.
fn relock(lock: &Path, inputs: &[String]) -> Result<Vec<InputChange>, Option<(Stage, String)>> {
    let repo = repo();
    if let Some(folder) = lock.parent() {
        fs::create_dir_all(folder).map_err(|e| Some((Stage::Lock, e.to_string())))?;
    }
    let mut command = Command::new("nix");
    command.args(["flake", "update"]).args(inputs).args([
        "--flake",
        &repo.to_string_lossy(),
        "--output-lock-file",
        &lock.to_string_lossy(),
        "--no-warn-dirty",
    ]);
    let child = command
        .current_dir(&repo)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Some((Stage::Lock, format!("nix: {error}"))))?;
    *CHILD.lock().unwrap_or_else(PoisonError::into_inner) = Some(child);
    let child = CHILD.lock().unwrap_or_else(PoisonError::into_inner).take();
    let output = child
        .map(Child::wait_with_output)
        .ok_or(None)?
        .map_err(|error| Some((Stage::Lock, error.to_string())))?;

    {
        let mut job = BuildJob::write();
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            let line = strip_ansi(line);
            if !line.trim().is_empty() {
                job.note(line);
            }
        }
    }
    if CANCELLED.load(Ordering::SeqCst) {
        return Err(None);
    }
    if !output.status.success() {
        return Err(Some((
            Stage::Lock,
            failure(&output.stderr, &output.stdout, output.status.code()),
        )));
    }

    let old = fs::read_to_string(repo.join("flake.lock")).unwrap_or_default();
    let new = fs::read_to_string(lock).map_err(|e| Some((Stage::Lock, e.to_string())))?;
    update::diff_locks(&old, &new).map_err(|e| Some((Stage::Lock, e)))
}

/// The built system's path; none when cancelled. With `reference_lock`,
/// that lock is built instead of the repo's.
fn build(reference_lock: Option<&Path>) -> Result<PathBuf, Option<(Stage, String)>> {
    let flake = format!(
        "{}#nixosConfigurations.{}.config.system.build.toplevel",
        repo().display(),
        host()
    );
    let mut command = Command::new("nix");
    command.args([
        "build",
        "--no-link",
        "--print-out-paths",
        "--log-format",
        "internal-json",
        "--no-warn-dirty",
    ]);
    if let Some(lock) = reference_lock {
        command.arg("--reference-lock-file").arg(lock);
    }
    let mut child = command
        .arg(&flake)
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Some((Stage::Build, format!("nix: {error}"))))?;

    let stderr = child.stderr.take();
    let stdout = child.stdout.take();
    *CHILD.lock().unwrap_or_else(PoisonError::into_inner) = Some(child);

    // the log, until nix is done with it
    let mut follow = Follow::default();
    if let Some(stderr) = stderr {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            follow.line(&line);
        }
    }
    follow.flush(true);

    let mut out = String::new();
    if let Some(mut stdout) = stdout {
        let _ = stdout.read_to_string(&mut out);
    }
    let child = CHILD.lock().unwrap_or_else(PoisonError::into_inner).take();
    let status = child.and_then(|mut child| child.wait().ok());

    if CANCELLED.load(Ordering::SeqCst) {
        return Err(None);
    }
    match status {
        Some(status) if status.success() => {
            let path = out.lines().last().unwrap_or("").trim();
            if path.starts_with("/nix/store/") {
                Ok(PathBuf::from(path))
            } else {
                Err(Some((
                    Stage::Build,
                    String::from("nix gave no output path"),
                )))
            }
        }
        _ => {
            let stage = if follow.saw_builds {
                Stage::Build
            } else {
                Stage::Eval
            };
            let said: Vec<String> = BuildJob::read()
                .log
                .iter()
                .rev()
                .take(15)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let message = if said.is_empty() {
                String::from("nix failed without saying why")
            } else {
                said.join("\n")
            };
            Err(Some((stage, message)))
        }
    }
}

/// Reads nix's `@nix {…}` lines into progress and log lines, reaching the
/// window a few times a second.
#[derive(Default)]
struct Follow {
    progress: Progress,
    /// nix's id for the `builds` activity, whose results count progress.
    builds: Option<u64>,
    saw_builds: bool,
    lines: Vec<String>,
    changed: bool,
    last: Option<Instant>,
}

// nix's activity and result types (src/libutil/logging.hh)
const ACT_COPY_PATH: u64 = 100;
const ACT_FILE_TRANSFER: u64 = 101;
const ACT_BUILDS: u64 = 104;
const ACT_BUILD: u64 = 105;
const ACT_SUBSTITUTE: u64 = 108;
const ACT_FETCH_TREE: u64 = 112;
const RES_BUILD_LOG_LINE: u64 = 101;
const RES_PROGRESS: u64 = 105;

impl Follow {
    fn line(&mut self, line: &str) {
        match line.strip_prefix("@nix ") {
            Some(json) => self.event(json),
            None => {
                let line = strip_ansi(line);
                if !line.trim().is_empty() {
                    self.lines.push(line);
                    self.changed = true;
                }
            }
        }
        self.flush(false);
    }

    fn event(&mut self, json: &str) {
        let Ok(event) = serde_json::from_str::<Value>(json) else {
            return;
        };
        let number = |key: &str| event.get(key).and_then(Value::as_u64);
        let kind = number("type").unwrap_or(0);
        let id = number("id").unwrap_or(0);

        match event.get("action").and_then(Value::as_str) {
            Some("start") => {
                let text = event.get("text").and_then(Value::as_str).unwrap_or("");
                match kind {
                    ACT_BUILDS => {
                        self.builds = Some(id);
                        self.saw_builds = true;
                        self.progress.evaluating = false;
                        self.changed = true;
                    }
                    ACT_BUILD | ACT_COPY_PATH | ACT_SUBSTITUTE | ACT_FILE_TRANSFER
                    | ACT_FETCH_TREE => {
                        let verb = match kind {
                            ACT_BUILD => "building",
                            ACT_FETCH_TREE => "fetching",
                            ACT_FILE_TRANSFER => "downloading",
                            _ => "fetching",
                        };
                        if let Some(name) = store_name(text) {
                            self.progress.current = Some(format!("{verb} {name}"));
                            self.progress.evaluating = false;
                            self.changed = true;
                        } else if kind == ACT_FETCH_TREE && !text.is_empty() {
                            self.progress.current = Some(strip_ansi(text));
                            self.changed = true;
                        }
                    }
                    _ => {}
                }
            }
            Some("result") => {
                let fields = event.get("fields").and_then(Value::as_array);
                if kind == RES_PROGRESS && Some(id) == self.builds {
                    if let Some(fields) = fields {
                        let at = |i: usize| fields.get(i).and_then(Value::as_u64).unwrap_or(0);
                        self.progress.done = at(0);
                        self.progress.expected = at(1);
                        self.progress.running = at(2);
                        self.progress.failed = at(3);
                        self.changed = true;
                    }
                } else if kind == RES_BUILD_LOG_LINE
                    && let Some(text) = fields.and_then(|f| f.first()).and_then(Value::as_str)
                {
                    self.lines.push(strip_ansi(text));
                    self.changed = true;
                }
            }
            Some("msg") => {
                // errors, warnings and notices; nix's chatter is below
                if number("level").unwrap_or(9) <= 2
                    && let Some(text) = event.get("msg").and_then(Value::as_str)
                {
                    for line in strip_ansi(text).lines() {
                        self.lines.push(line.to_string());
                    }
                    self.changed = true;
                }
            }
            _ => {}
        }
    }

    fn flush(&mut self, force: bool) {
        if !self.changed {
            return;
        }
        let due = self.last.is_none_or(|at| at.elapsed() >= REDRAW);
        if !force && !due {
            return;
        }
        self.last = Some(Instant::now());
        self.changed = false;

        let mut job = BuildJob::write();
        for line in self.lines.drain(..) {
            job.note(line);
        }
        let building_ours = job.target.is_ours();
        if let Phase::Building(progress) = &mut job.phase
            && building_ours
        {
            *progress = self.progress.clone();
        }
    }
}

/// "foo-1.2" from "building '/nix/store/<hash>-foo-1.2.drv'".
fn store_name(text: &str) -> Option<String> {
    let start = text.find("/nix/store/")? + "/nix/store/".len();
    let rest = &text[start..];
    let name = rest
        .split(['\'', '"', ' '])
        .next()?
        .get(33..)?
        .trim_end_matches(".drv");
    (!name.is_empty()).then(|| name.to_string())
}

/// The text without terminal colour codes.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

// ───── comparing ─────────────────────────────────────────────────────────────

fn compare_then(out: PathBuf) {
    let then = BuildJob::read().then;
    match compare(&out) {
        Ok(built) => {
            {
                let mut job = BuildJob::write();
                if built.diff.is_empty() {
                    job.note(built.raw_diff.clone());
                }
                job.phase = Phase::Built(built);
            }
            if let Some(mode) = then {
                BuildJob::activate(mode);
            }
        }
        Err(message) => fail(Stage::Compare, message),
    }
}

fn compare(out: &Path) -> Result<Built, String> {
    let raw = Command::new("nix")
        .args(["store", "diff-closures", "/run/current-system"])
        .arg(out)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()
        .map(|output| {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).into_owned()
            } else {
                failure(&output.stderr, &output.stdout, output.status.code())
            }
        })
        .unwrap_or_else(|error| error.to_string());

    let needs_restart = ["kernel", "initrd", "kernel-modules"].iter().any(|part| {
        fs::read_link(out.join(part)).ok()
            != fs::read_link(Path::new("/run/booted-system").join(part)).ok()
    });

    Ok(Built {
        out: out.to_path_buf(),
        diff: parse_diff(&raw),
        raw_diff: raw,
        needs_restart,
    })
}

/// `nix store diff-closures` lines: "name: 1.0 → 1.1, +12.3 KiB",
/// "name: ∅ → 2.0", "name: 1.0 → ∅, -1.2 MiB". Lines that don't read
/// that way are left out; the raw text is kept beside.
pub fn parse_diff(text: &str) -> Vec<DiffLine> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(": ")?;
            let (versions, size) = match rest.rsplit_once(", ") {
                Some((versions, size))
                    if size.starts_with('+') || size.starts_with('-') || size.starts_with('±') =>
                {
                    (versions, Some(size.trim().to_string()))
                }
                _ => (rest, None),
            };
            let (from, to) = versions.split_once(" → ")?;
            Some(DiffLine {
                name: name.trim().to_string(),
                from: from.trim().to_string(),
                to: to.trim().to_string(),
                size,
            })
        })
        .collect()
}

// ───── activating ────────────────────────────────────────────────────────────

fn switch(out: &Path, mode: Mode) -> Result<(), String> {
    let helper = fs::canonicalize(HELPER).map_err(|_| {
        String::from(
            "amane-nixos-activate isn't in the running system yet: switch once by hand \
             (sudo nixos-rebuild switch) to a configuration with modules/managed.nix",
        )
    })?;
    let output = Command::new("pkexec")
        .arg(helper)
        .arg(mode.arg())
        .arg(out)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("pkexec: {error}"))?;

    match output.status.code() {
        Some(0) => Ok(()),
        Some(126) => Err(String::from("Cancelled")),
        Some(127) => Err(String::from("Not authorised")),
        code => Err(failure(&output.stderr, &output.stdout, code)),
    }
}

fn finish(out: PathBuf, mode: Mode) {
    let target = BuildJob::read().target.clone();

    // the edits that went in, as a commit; a restore has none of its own.
    // what it installs, before the commit makes it the last applied
    let installed = if target.is_ours() {
        super::apps::arriving(&Managed::read(), &Snapshot::read())
    } else {
        Vec::new()
    };

    // An update puts its lock into the repo first, and commits that too.
    let commit = if mode.records() && target.is_ours() {
        let edits = {
            let managed = Managed::read();
            (!managed.changes().is_empty()).then(|| managed.commit_message(&Snapshot::read()))
        };
        match &target {
            Target::Update { lock, changes, .. } => {
                let placed = fs::copy(lock, repo().join("flake.lock"))
                    .map_err(|e| format!("flake.lock not written: {e}"))
                    .and_then(|_| git(&["add", "--", "flake.lock"]).map(|_| ()));
                let message = match edits {
                    Some(edits) => format!("{}; {edits}", update::message(changes)),
                    None => update::message(changes),
                };
                Some(placed.and_then(|()| {
                    Repo::commit_paths(&message, &["flake.lock", super::nixos::SETTINGS_FILE])
                        .map(|_| message)
                }))
            }
            _ => edits.map(|message| Repo::commit_settings(&message).map(|_| message)),
        }
    } else {
        None
    };

    if mode.records() && target.is_ours() {
        let head = commit
            .as_ref()
            .and_then(|c| c.as_ref().ok())
            .map(|_| Repo::read().head.clone())
            .unwrap_or(None)
            .or_else(|| {
                git(&["rev-parse", "HEAD"])
                    .ok()
                    .map(|h| h.trim().to_string())
            });
        if let Some(head) = head {
            Applied {
                head,
                toplevel: out.clone(),
            }
            .save();
        }
    }

    Managed::reload();
    Repo::refresh();
    Running::refresh();
    Generations::refresh();
    super::installed::Launchers::refresh();
    if target.is_ours() {
        Snapshot::refresh();
        Toplevel::refresh();
    }
    // an applied update is no longer waiting, whatever the timer found
    if mode.records() && matches!(target, Target::Update { .. }) {
        update::Check::clear();
    }

    let generation = mode
        .records()
        .then(|| fs::read_link("/nix/var/nix/profiles/system").ok())
        .flatten()
        .and_then(|link| {
            link.to_string_lossy()
                .strip_prefix("system-")?
                .strip_suffix("-link")?
                .parse()
                .ok()
        });

    let what = if matches!(target, Target::Update { .. }) {
        "System update applied"
    } else {
        "System settings applied"
    };
    let body = match (mode, generation) {
        (Mode::Test, _) => String::from("Tried without recording it: a restart undoes it."),
        (Mode::Boot, _) => String::from("It takes effect at the next restart."),
        (_, Some(n)) => format!("Generation {n} is running and in History."),
        (_, None) => String::from("The new system is running."),
    };
    Notify::new(what).body(&body).send();

    let mut job = BuildJob::write();
    job.phase = Phase::Done {
        mode,
        generation,
        commit,
        installed,
    };
    job.then = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nix_events_become_progress_and_log_lines() {
        let mut follow = Follow::default();
        follow.event(r#"{"action":"start","id":1,"level":4,"parent":0,"text":"","type":104}"#);
        assert!(follow.saw_builds);
        assert!(!follow.progress.evaluating);

        follow.event(
            r#"{"action":"start","id":2,"level":4,"parent":1,"text":"building '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-amane-shell-0.1.0.drv'","type":105}"#,
        );
        assert_eq!(
            follow.progress.current.as_deref(),
            Some("building amane-shell-0.1.0")
        );

        follow.event(r#"{"action":"result","id":1,"type":105,"fields":[3,7,1,0]}"#);
        assert_eq!(follow.progress.done, 3);
        assert_eq!(follow.progress.expected, 7);
        assert_eq!(follow.progress.percent(), 42);

        // another activity's progress isn't the builds'
        follow.event(r#"{"action":"result","id":2,"type":105,"fields":[9,9,0,0]}"#);
        assert_eq!(follow.progress.done, 3);

        follow.event(
            r#"{"action":"result","id":2,"type":101,"fields":["   Compiling amane v0.1.0"]}"#,
        );
        follow.event(r#"{"action":"msg","level":0,"msg":"\u001b[31;1merror:\u001b[0m attribute 'foo' missing"}"#);
        follow.event(r#"{"action":"msg","level":5,"msg":"chatter"}"#);
        assert_eq!(
            follow.lines,
            vec![
                "   Compiling amane v0.1.0".to_string(),
                "error: attribute 'foo' missing".to_string()
            ]
        );
    }

    #[test]
    fn a_store_path_names_its_package() {
        assert_eq!(
            store_name(
                "copying path '/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-linux-7.2.10' from 'https://cache.nixos.org'"
            ),
            Some(String::from("linux-7.2.10"))
        );
        assert_eq!(store_name("evaluating"), None);
    }

    #[test]
    fn diff_closures_lines_are_read_leniently() {
        let text = "linux: 7.2.10 → 7.2.12, +1.2 MiB\nfoo: ∅ → 2.0\nbar: 1.0 → ∅, -3.4 KiB\n\
                    nixos-system-nixos: 26.05.1 → 26.05.2\nsomething odd\n";
        let diff = parse_diff(text);
        assert_eq!(diff.len(), 4);
        assert_eq!(diff[0].size.as_deref(), Some("+1.2 MiB"));
        assert_eq!(diff[0].kind(), Kind::Changed);
        assert_eq!(diff[1].kind(), Kind::Added);
        assert_eq!(diff[2].kind(), Kind::Removed);
        assert_eq!(diff[3].size, None);
    }

    #[test]
    fn progress_counts_in_percent() {
        let mut progress = Progress::default();
        assert_eq!(progress.percent(), 0);
        progress.expected = 4;
        progress.done = 4;
        assert_eq!(progress.percent(), 100);
    }
}
