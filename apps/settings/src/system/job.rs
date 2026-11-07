//! The pipeline behind Apply: build the system as the user (nix reports
//! progress as JSON lines), compare it with the running one, activate it
//! as root through polkit, then commit what changed. Check for Updates is
//! the same pipeline from a fresh `flake.lock` written beside the repo,
//! which goes into the repo once the update is applied. One job at a time,
//! on its own threads; the window reads its [`Phase`].

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Notify, Service};
use amane_nixos::job::{NixLog, StepError, Steps, system_generation};

pub use amane_nixos::job::{Built, Kind, Mode, Progress, Stage};

use super::generations::Generations;
use super::nixos::{Managed, Snapshot, config, git};
use super::repo::{Applied, Repo, Running, Toplevel};
use super::state_dir;
use super::update::{self, InputChange};

/// How many lines of the build log the window keeps.
const LOG_LINES: usize = 300;

/// How often progress reaches the window at most.
const REDRAW: Duration = Duration::from_millis(100);

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

// runs the job's steps, and stops the one running
static STEPS: Steps = Steps::new();

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
        STEPS.start();

        thread::spawn(|| match build(None) {
            Ok(out) => compare_then(out),
            Err(error) => stopped(error),
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
        STEPS.start();

        thread::spawn(move || {
            let changes = match STEPS.relock(&config(), &lock, &inputs, &mut |line| {
                Self::write().note(line)
            }) {
                Ok(changes) => changes,
                Err(error) => return stopped(error),
            };
            if changes.is_empty() {
                let mut job = Self::write();
                job.note("Every input is at its newest.");
                job.phase = Phase::UpToDate;
                drop(job);
                // the scheduled check's record is stale now
                if inputs.is_empty() {
                    update::Checked::clear();
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
                Err(error) => stopped(error),
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
        STEPS.cancel();
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

        thread::spawn(move || match STEPS.activate(&config(), &out, mode) {
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

/// A job's step that didn't finish: cancelled puts the job away.
fn stopped(error: StepError) {
    match error {
        StepError::Failed(stage, message) => fail(stage, message),
        StepError::Cancelled => BuildJob::write().phase = Phase::Idle,
    }
}

/// Builds the working tree, or with `reference_lock` an update of it; the
/// log and progress reach the window a few times a second.
fn build(reference_lock: Option<&Path>) -> Result<PathBuf, StepError> {
    let mut last: Option<Instant> = None;
    let mut pending = false;
    STEPS.build(
        &config(),
        reference_lock,
        &mut |log: &mut NixLog, last_call| {
            pending |= log.take_changed();
            let due = last.is_none_or(|at| at.elapsed() >= REDRAW);
            if !pending || !(due || last_call) {
                return;
            }
            last = Some(Instant::now());
            pending = false;

            let mut job = BuildJob::write();
            for line in log.take_lines() {
                job.note(line);
            }
            let building_ours = job.target.is_ours();
            if let Phase::Building(progress) = &mut job.phase
                && building_ours
            {
                *progress = log.progress.clone();
            }
        },
    )
}

// ───── comparing ─────────────────────────────────────────────────────────────

fn compare_then(out: PathBuf) {
    let then = BuildJob::read().then;
    let built = STEPS.compare(&config(), &out);
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
                let placed = STEPS.place_lock(&config(), lock);
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
        update::Checked::clear();
    }

    let generation = mode
        .records()
        .then(|| system_generation(&config()))
        .flatten();

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
