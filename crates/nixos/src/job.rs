//! The pipeline behind Apply: build the system as the user (nix reports
//! progress as JSON lines), compare it with the running one, then activate
//! it as root through polkit. Check for Updates is the same pipeline from a
//! fresh `flake.lock` written beside the repo, which goes into the repo
//! once the update is applied ([`Steps::place_lock`]). Restore is compare
//! and activate on an older generation.
//!
//! [`Steps`] runs one step at a time, on the caller's thread, and is what
//! stops it from another. What a step says comes back as it goes: the log
//! lines, and the build's [`Progress`].

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use amane_process::{Cmd, Failure, Stopper};
use serde_json::Value;

use crate::Config;
use crate::lock::{InputChange, diff_locks};

/// How many of the last lines nix said a failed build's message keeps.
const FAILED_LINES: usize = 15;

/// How a built system is activated: `switch` and `boot` record a
/// generation, `test` doesn't, so a restart undoes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Activate now and boot into it from now on.
    Switch,
    /// Boot into it from the next start, leaving the running system alone.
    Boot,
    /// Activate now without making it the boot default.
    Test,
}

impl Mode {
    /// The helper's argument for it: "switch", "boot" or "test".
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

/// A stage of the pipeline: where a step is, or where it failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Writing a fresh `flake.lock` (Check for Updates).
    Lock,
    /// nix evaluating the configuration, before anything builds.
    Eval,
    /// Building or downloading the system.
    Build,
    /// Comparing the built system with the running one.
    Compare,
    /// Activating it as root.
    Activate,
}

impl Stage {
    /// Its name as the page shows it, such as "Checking for updates".
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
    /// Builds and downloads finished.
    pub done: u64,
    /// Builds and downloads nix expects in all.
    pub expected: u64,
    /// Builds and downloads under way.
    pub running: u64,
    /// Builds and downloads that failed.
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
    /// The package's name.
    pub name: String,
    /// Its version(s) in the running system, "∅" when it's new.
    pub from: String,
    /// Its version(s) in the built system, "∅" when it's gone.
    pub to: String,
    /// How its size changes, such as "+12.3 KiB", when nix says.
    pub size: Option<String>,
}

/// What a [`DiffLine`] does to a package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Only the built system has it.
    Added,
    /// Only the running system has it.
    Removed,
    /// Both have it, at different versions or sizes.
    Changed,
}

impl DiffLine {
    /// Whether the package was added, removed or changed, from the "∅" sides.
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
    /// The built system's store path.
    pub out: PathBuf,
    /// The diff-closures lines that parsed.
    pub diff: Vec<DiffLine>,
    /// What diff-closures said, for the log when the lines didn't parse.
    pub raw_diff: String,
    /// Its kernel, initrd or modules aren't the booted ones.
    pub needs_restart: bool,
}

/// Why a step didn't finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepError {
    /// [`Steps::cancel`] stopped it.
    Cancelled,
    /// It failed at this stage, saying this.
    Failed(Stage, String),
}

impl StepError {
    fn at(stage: Stage) -> impl FnOnce(String) -> Self {
        move |message| Self::Failed(stage, message)
    }
}

/// Runs the pipeline's steps, one at a time, and stops the one running.
/// A `static` holds one, so a Cancel button reaches it.
pub struct Steps {
    child: Mutex<Option<Stopper>>,
    cancelled: AtomicBool,
}

impl Default for Steps {
    fn default() -> Self {
        Self::new()
    }
}

impl Steps {
    /// Nothing running.
    pub const fn new() -> Self {
        Self {
            child: Mutex::new(None),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Ready for a new job: a cancel from the last one no longer counts.
    pub fn start(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    /// Stops the step running, and any after it in this job.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(child) = self
            .child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            child.stop();
        }
    }

    /// Whether the job was cancelled.
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Runs `command` to its end, stoppable by [`Steps::cancel`].
    fn run(
        &self,
        command: &Cmd,
        stage: Stage,
        each_stderr_line: &mut dyn FnMut(&str),
    ) -> Result<amane_process::Output, StepError> {
        if self.cancelled() {
            return Err(StepError::Cancelled);
        }
        let mut running = command.spawn().map_err(|error| {
            StepError::Failed(
                stage,
                format!("{}: {error}", command.program().to_string_lossy()),
            )
        })?;
        *self.child.lock().unwrap_or_else(PoisonError::into_inner) = Some(running.stopper());
        let seen = running.each_stderr_line(each_stderr_line);
        let result = running.finish_with(seen);
        self.child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if self.cancelled() {
            return Err(StepError::Cancelled);
        }
        result.map_err(|failure: Failure| StepError::Failed(stage, failure.message))
    }

    /// `nix flake update` into `lock`, without touching the repo's: what it
    /// moves against the repo's lock. Each line nix says goes to `log`.
    pub fn relock(
        &self,
        config: &Config,
        lock: &Path,
        inputs: &[String],
        log: &mut dyn FnMut(String),
    ) -> Result<Vec<InputChange>, StepError> {
        if let Some(folder) = lock.parent() {
            fs::create_dir_all(folder)
                .map_err(|e| StepError::Failed(Stage::Lock, e.to_string()))?;
        }
        let command = Cmd::new(&config.nix)
            .args(["flake", "update"])
            .args(inputs)
            .arg("--flake")
            .arg(&config.repo)
            .arg("--output-lock-file")
            .arg(lock)
            .arg("--no-warn-dirty")
            .dir(&config.repo);
        self.run(&command, Stage::Lock, &mut |line| {
            let line = strip_ansi(line);
            if !line.trim().is_empty() {
                log(line);
            }
        })?;

        let old = fs::read_to_string(config.repo.join("flake.lock")).unwrap_or_default();
        let new =
            fs::read_to_string(lock).map_err(|e| StepError::Failed(Stage::Lock, e.to_string()))?;
        diff_locks(&old, &new).map_err(StepError::at(Stage::Lock))
    }

    /// Builds the configuration: the built system's path. With
    /// `reference_lock`, that lock is built instead of the repo's. `report`
    /// is called as nix says anything, with whether it's the last call.
    pub fn build(
        &self,
        config: &Config,
        reference_lock: Option<&Path>,
        report: &mut dyn FnMut(&mut NixLog, bool),
    ) -> Result<PathBuf, StepError> {
        let flake = format!(
            "{}#nixosConfigurations.{}.config.system.build.toplevel",
            config.repo.display(),
            config.host
        );
        let mut command = Cmd::new(&config.nix).args([
            "build",
            "--no-link",
            "--print-out-paths",
            "--log-format",
            "internal-json",
            "--no-warn-dirty",
        ]);
        if let Some(lock) = reference_lock {
            command = command.arg("--reference-lock-file").arg(lock);
        }
        let command = command.arg(&flake).dir(&config.repo);

        let mut log = NixLog::default();
        let result = self.run(&command, Stage::Build, &mut |line| {
            log.line(line);
            report(&mut log, false);
        });
        report(&mut log, true);
        match result {
            Ok(output) => {
                let path = output.stdout.lines().last().unwrap_or("").trim();
                if path.starts_with("/nix/store/") {
                    Ok(PathBuf::from(path))
                } else {
                    Err(StepError::Failed(
                        Stage::Build,
                        String::from("nix gave no output path"),
                    ))
                }
            }
            Err(StepError::Cancelled) => Err(StepError::Cancelled),
            Err(StepError::Failed(..)) => {
                let stage = if log.saw_builds {
                    Stage::Build
                } else {
                    Stage::Eval
                };
                let message = if log.recent.is_empty() {
                    String::from("nix failed without saying why")
                } else {
                    Vec::from(log.recent.clone()).join("\n")
                };
                Err(StepError::Failed(stage, message))
            }
        }
    }

    /// The built system against the running one. A comparison that fails
    /// shows its error in place of the changes.
    pub fn compare(&self, config: &Config, out: &Path) -> Built {
        let raw = Cmd::new(&config.nix)
            .args(["store", "diff-closures"])
            .arg(&config.current_system)
            .arg(out)
            .stdout()
            .unwrap_or_else(|message| message);

        let needs_restart = ["kernel", "initrd", "kernel-modules"].iter().any(|part| {
            fs::read_link(out.join(part)).ok()
                != fs::read_link(config.booted_system.join(part)).ok()
        });

        Built {
            out: out.to_path_buf(),
            diff: parse_diff(&raw),
            raw_diff: raw,
            needs_restart,
        }
    }

    /// Activates `out` as root, through polkit and the helper the running
    /// system has.
    pub fn activate(&self, config: &Config, out: &Path, mode: Mode) -> Result<(), String> {
        let helper = fs::canonicalize(&config.helper).map_err(|_| {
            String::from(
                "amane-nixos-activate isn't in the running system yet: switch once by hand \
                 (sudo nixos-rebuild switch) to a configuration with modules/managed.nix",
            )
        })?;
        match Cmd::new(&config.pkexec)
            .arg(helper)
            .arg(mode.arg())
            .arg(out)
            .output()
        {
            Ok(_) => Ok(()),
            Err(Failure {
                code: Some(126), ..
            }) => Err(String::from("Cancelled")),
            Err(Failure {
                code: Some(127), ..
            }) => Err(String::from("Not authorised")),
            Err(failure) => Err(failure.message),
        }
    }

    /// Puts an update's fresh lock into the repo and stages it, for the
    /// commit that records the update.
    pub fn place_lock(&self, config: &Config, lock: &Path) -> Result<(), String> {
        fs::copy(lock, config.repo.join("flake.lock"))
            .map_err(|e| format!("flake.lock not written: {e}"))?;
        config.git(&["add", "--", "flake.lock"]).map(|_| ())
    }
}

/// The generation the system profile points at now.
pub fn system_generation(config: &Config) -> Option<u32> {
    let link = fs::read_link(config.profiles.join("system")).ok()?;
    crate::generations::number(&link.to_string_lossy())
}

/// nix's `@nix {…}` lines read into [`Progress`] and log lines.
#[derive(Clone, Debug, Default)]
pub struct NixLog {
    /// How far the build is.
    pub progress: Progress,
    /// Whether nix got as far as building (else a failure is evaluation's).
    pub saw_builds: bool,
    // nix's id for the `builds` activity, whose results count progress
    builds: Option<u64>,
    lines: Vec<String>,
    recent: VecDeque<String>,
    changed: bool,
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

impl NixLog {
    /// Reads one line nix wrote to stderr.
    pub fn line(&mut self, line: &str) {
        match line.strip_prefix("@nix ") {
            Some(json) => self.event(json),
            None => {
                let line = strip_ansi(line);
                if !line.trim().is_empty() {
                    self.say(line);
                }
            }
        }
    }

    /// Whether anything changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// The log lines read since the last call.
    pub fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    fn say(&mut self, line: String) {
        if self.recent.len() == FAILED_LINES {
            self.recent.pop_front();
        }
        self.recent.push_back(line.clone());
        self.lines.push(line);
        self.changed = true;
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
                    self.say(strip_ansi(text));
                }
            }
            Some("msg") => {
                // errors, warnings and notices; nix's chatter is below
                if number("level").unwrap_or(9) <= 2
                    && let Some(text) = event.get("msg").and_then(Value::as_str)
                {
                    for line in strip_ansi(text).lines() {
                        self.say(line.to_string());
                    }
                }
            }
            _ => {}
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
pub fn strip_ansi(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::time::Duration;

    /// A folder of fake programs and a repo, gone with the test.
    struct Fakes {
        dir: PathBuf,
        config: Config,
    }

    impl Fakes {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("amane-nixos-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("repo")).unwrap();
            fs::create_dir_all(dir.join("profiles")).unwrap();
            let mut config = Config::new(dir.join("repo"), "test-host");
            config.profiles = dir.join("profiles");
            config.current_system = dir.join("current");
            config.booted_system = dir.join("booted");
            config.helper = dir.join("helper");
            fs::write(&config.helper, "").unwrap();
            let fakes = Self { dir, config };
            // every fake logs how it was called, one line a call
            for name in ["nix", "git", "pkexec"] {
                fakes.program(name, "exit 0");
            }
            fakes
        }

        /// A fake `name` that logs its arguments, then runs `body`.
        fn program(&self, name: &str, body: &str) {
            let path = self.dir.join(name);
            let calls = self.dir.join("calls");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\necho \"{name} $*\" >> '{}'\n{body}\n",
                    calls.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn config(&self) -> Config {
            let mut config = self.config.clone();
            config.nix = self.dir.join("nix").into();
            config.git = self.dir.join("git").into();
            config.pkexec = self.dir.join("pkexec").into();
            config
        }

        fn calls(&self) -> Vec<String> {
            fs::read_to_string(self.dir.join("calls"))
                .unwrap_or_default()
                .lines()
                .map(String::from)
                .collect()
        }
    }

    impl Drop for Fakes {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    const OUT: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-nixos-system-test";

    #[test]
    fn a_build_reports_progress_and_gives_its_path() {
        let fakes = Fakes::new("build");
        fakes.program(
            "nix",
            &format!(
                r#"echo '@nix {{"action":"start","id":1,"type":104,"text":""}}' >&2
echo '@nix {{"action":"result","id":1,"type":105,"fields":[1,2,0,0]}}' >&2
echo '{OUT}'"#
            ),
        );
        let steps = Steps::new();
        let mut last = Progress::default();
        let mut finals = 0;
        let out = steps
            .build(&fakes.config(), None, &mut |log, last_call| {
                last = log.progress.clone();
                finals += usize::from(last_call);
            })
            .unwrap();
        assert_eq!(out, PathBuf::from(OUT));
        assert_eq!((last.done, last.expected, last.percent()), (1, 2, 50));
        assert_eq!(finals, 1);
        let call = &fakes.calls()[0];
        assert!(
            call.starts_with("nix build --no-link --print-out-paths --log-format internal-json")
        );
        assert!(call.ends_with("#nixosConfigurations.test-host.config.system.build.toplevel"));
    }

    #[test]
    fn a_failed_build_says_what_nix_said_and_where() {
        let fakes = Fakes::new("failed");
        fakes.program(
            "nix",
            r#"echo '@nix {"action":"msg","level":0,"msg":"error: attribute foo missing"}' >&2
exit 1"#,
        );
        let failed = Steps::new().build(&fakes.config(), None, &mut |_, _| {});
        assert_eq!(
            failed,
            Err(StepError::Failed(
                Stage::Eval,
                String::from("error: attribute foo missing")
            ))
        );
    }

    #[test]
    fn a_cancelled_build_stops_and_says_so() {
        let fakes = Fakes::new("cancel");
        // exec, so the stop reaches the one holding the pipe
        fakes.program("nix", "exec sleep 30");
        let steps = Arc::new(Steps::new());
        let canceller = Arc::clone(&steps);
        let stop = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            canceller.cancel();
        });
        let result = steps.build(&fakes.config(), None, &mut |_, _| {});
        stop.join().unwrap();
        assert_eq!(result, Err(StepError::Cancelled));
        // and nothing after it in the job runs until the next one starts
        assert_eq!(
            steps.relock(&fakes.config(), &fakes.dir.join("l"), &[], &mut |_| {}),
            Err(StepError::Cancelled)
        );
        steps.start();
        assert!(!steps.cancelled());
    }

    #[test]
    fn an_update_writes_a_fresh_lock_and_says_what_moved() {
        let fakes = Fakes::new("update");
        let lock = |rev: &str| {
            format!(
                r#"{{"nodes":{{"nixpkgs":{{"locked":{{"rev":"{rev}"}}}},"root":{{"inputs":{{"nixpkgs":"nixpkgs"}}}}}},"root":"root","version":7}}"#
            )
        };
        fs::write(fakes.config.repo.join("flake.lock"), lock("1111111aaaa")).unwrap();
        // writes the lock it's told to, as nix does
        fakes.program(
            "nix",
            &format!(
                r#"while [ $# -gt 0 ]; do [ "$1" = --output-lock-file ] && echo '{}' > "$2"; shift; done
echo "• Updated input 'nixpkgs'" >&2"#,
                lock("2222222bbbb")
            ),
        );
        let fresh = fakes.dir.join("state/update.lock");
        let mut said = Vec::new();
        let changes = Steps::new()
            .relock(&fakes.config(), &fresh, &["nixpkgs".into()], &mut |line| {
                said.push(line)
            })
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text(), "nixpkgs 1111111 → 2222222");
        assert_eq!(said, ["• Updated input 'nixpkgs'"]);
        assert!(fakes.calls()[0].starts_with("nix flake update nixpkgs --flake "));

        // applied: the lock goes into the repo and is staged
        Steps::new().place_lock(&fakes.config(), &fresh).unwrap();
        assert_eq!(
            fs::read_to_string(fakes.config.repo.join("flake.lock"))
                .unwrap()
                .trim(),
            lock("2222222bbbb")
        );
        assert_eq!(
            fakes.calls().last().unwrap(),
            "git -c core.fsync=committed add -- flake.lock"
        );
    }

    #[test]
    fn activating_goes_through_polkit_and_reads_its_answers() {
        let fakes = Fakes::new("activate");
        let steps = Steps::new();
        let old = Path::new("/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-nixos-system-old");
        // a rollback: compare an older generation, then switch to it
        fakes.program("nix", "echo 'linux: 7.2.12 → 7.2.10, -1.2 MiB'");
        let built = steps.compare(&fakes.config(), old);
        assert_eq!(built.diff.len(), 1);
        assert_eq!(built.diff[0].name, "linux");
        steps.activate(&fakes.config(), old, Mode::Switch).unwrap();
        let helper = fs::canonicalize(&fakes.config.helper).unwrap();
        assert_eq!(
            fakes.calls().last().unwrap(),
            &format!("pkexec {} switch {}", helper.display(), old.display())
        );

        fakes.program("pkexec", "exit 126");
        assert_eq!(
            steps.activate(&fakes.config(), old, Mode::Boot),
            Err(String::from("Cancelled"))
        );
        fakes.program("pkexec", "exit 127");
        assert_eq!(
            steps.activate(&fakes.config(), old, Mode::Test),
            Err(String::from("Not authorised"))
        );
        fakes.program("pkexec", "echo 'activation failed' >&2; exit 4");
        assert_eq!(
            steps.activate(&fakes.config(), old, Mode::Switch),
            Err(String::from("activation failed"))
        );
    }

    #[test]
    fn the_steps_of_an_apply_run_in_order() {
        let fakes = Fakes::new("order");
        fakes.program(
            "nix",
            &format!("[ \"$1\" = build ] && echo '{OUT}'; exit 0"),
        );
        let config = fakes.config();
        let steps = Steps::new();
        let out = steps.build(&config, None, &mut |_, _| {}).unwrap();
        steps.compare(&config, &out);
        steps.activate(&config, &out, Mode::Switch).unwrap();
        let order: Vec<String> = fakes
            .calls()
            .iter()
            .map(|call| {
                call.split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        assert_eq!(order[0], "nix build");
        assert_eq!(order[1], "nix store");
        assert!(order[2].starts_with("pkexec "));
    }

    #[test]
    fn the_profile_names_the_running_generation() {
        let fakes = Fakes::new("profile");
        std::os::unix::fs::symlink("system-65-link", fakes.config.profiles.join("system")).unwrap();
        assert_eq!(system_generation(&fakes.config), Some(65));
    }

    #[test]
    fn nix_events_become_progress_and_log_lines() {
        let mut log = NixLog::default();
        log.event(r#"{"action":"start","id":1,"level":4,"parent":0,"text":"","type":104}"#);
        assert!(log.saw_builds);
        assert!(!log.progress.evaluating);

        log.event(
            r#"{"action":"start","id":2,"level":4,"parent":1,"text":"building '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-amane-shell-0.1.0.drv'","type":105}"#,
        );
        assert_eq!(
            log.progress.current.as_deref(),
            Some("building amane-shell-0.1.0")
        );

        log.event(r#"{"action":"result","id":1,"type":105,"fields":[3,7,1,0]}"#);
        assert_eq!(log.progress.done, 3);
        assert_eq!(log.progress.expected, 7);
        assert_eq!(log.progress.percent(), 42);

        // another activity's progress isn't the builds'
        log.event(r#"{"action":"result","id":2,"type":105,"fields":[9,9,0,0]}"#);
        assert_eq!(log.progress.done, 3);

        log.event(
            r#"{"action":"result","id":2,"type":101,"fields":["   Compiling amane v0.1.0"]}"#,
        );
        log.event(r#"{"action":"msg","level":0,"msg":"\u001b[31;1merror:\u001b[0m attribute 'foo' missing"}"#);
        log.event(r#"{"action":"msg","level":5,"msg":"chatter"}"#);
        assert!(log.take_changed());
        assert_eq!(
            log.take_lines(),
            vec![
                "   Compiling amane v0.1.0".to_string(),
                "error: attribute 'foo' missing".to_string()
            ]
        );
        assert!(log.take_lines().is_empty());
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
