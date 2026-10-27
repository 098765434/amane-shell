//! The repo and the running system, side by side: git's HEAD and dirty
//! files, what `nixos-version` says the running system was built from, the
//! record of the last Apply, and the system the working tree evaluates to.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use amane::Service;
use serde_json::Value;

use super::nixos::{SETTINGS_FILE, git, host, nix, repo};
use super::state_dir;

/// Git, as of the last look: polled every few seconds, since any editor
/// may change it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Repo {
    pub head: Option<String>,
    /// Tracked files with changes, as `git status` names them.
    pub dirty: Vec<String>,
    /// HEAD's subject line.
    pub subject: Option<String>,
    /// How many commits HEAD is past the running system's, when both are known.
    pub past_running: Option<usize>,
    pub error: Option<String>,
    pub looked: bool,
}

fn look() -> Repo {
    let head = git(&["rev-parse", "HEAD"]);
    let error = head.as_ref().err().cloned();
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .map(|out| {
            out.lines()
                .filter(|line| line.len() > 3)
                .map(|line| line[3..].trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    let head = head.ok().map(|out| out.trim().to_string());
    let past_running = match (Running::read().commit(), head.as_deref()) {
        (Some(running), Some(head)) if running != head => Repo::commits_between(running, head),
        _ => None,
    };

    Repo {
        head,
        past_running,
        dirty,
        subject: git(&["log", "-1", "--format=%s"])
            .ok()
            .map(|out| out.trim().to_string()),
        error,
        looked: true,
    }
}

impl Service for Repo {
    fn new() -> Self {
        Self::default()
    }

    fn interval() -> Duration {
        Duration::from_secs(3)
    }

    fn update(&mut self) -> bool {
        let fresh = look();
        let changed = fresh != *self;
        *self = fresh;
        changed
    }
}

impl Repo {
    /// Looks again now, after a write of our own.
    pub fn refresh() {
        thread::spawn(|| {
            let fresh = look();
            let mut now = Self::write();
            if fresh == *now {
                now.quiet();
            } else {
                *now = fresh;
            }
        });
    }

    /// The dirty files other than ours.
    pub fn dirty_elsewhere(&self) -> Vec<&str> {
        self.dirty
            .iter()
            .map(String::as_str)
            .filter(|file| *file != SETTINGS_FILE)
            .collect()
    }

    /// Commits `managed/settings.json` alone (a pathspec commit takes
    /// nothing else from the index) and gives the new HEAD. Never pushes.
    pub fn commit_settings(message: &str) -> Result<String, String> {
        git(&["commit", "-q", "-m", message, "--", SETTINGS_FILE])?;
        git(&["rev-parse", "HEAD"]).map(|out| out.trim().to_string())
    }

    /// How many commits `to` is past `from`, when both are known.
    pub fn commits_between(from: &str, to: &str) -> Option<usize> {
        git(&["rev-list", "--count", &format!("{from}..{to}")])
            .ok()?
            .trim()
            .parse()
            .ok()
    }
}

/// A commit as a label shows it: the first seven characters, keeping
/// `-dirty`.
pub fn short(revision: &str) -> String {
    let (sha, dirty) = match revision.strip_suffix("-dirty") {
        Some(sha) => (sha, "-dirty"),
        None => (revision, ""),
    };
    format!("{}{dirty}", &sha[..sha.len().min(7)])
}

// ───── the running system ────────────────────────────────────────────────────

/// What the running system says about itself; read again when
/// `/run/current-system` moves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Running {
    pub toplevel: Option<PathBuf>,
    pub booted: Option<PathBuf>,
    pub version: Option<String>,
    pub nixpkgs: Option<String>,
    /// The commit it was built from, when it records one (A1 of the plan).
    pub revision: Option<String>,
    pub kernel: Option<String>,
    pub looked: bool,
}

/// `nixos-version --json` for a system: its version, nixpkgs and commit.
pub fn version_of(toplevel: &std::path::Path) -> Option<(String, Option<String>, Option<String>)> {
    let script = toplevel.join("sw/bin/nixos-version");
    let json = super::nixos::run(&script.to_string_lossy(), &["--json"], None).ok()?;
    let value: Value = serde_json::from_str(&json).ok()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);

    Some((
        text("nixosVersion").unwrap_or_default(),
        text("nixpkgsRevision"),
        text("configurationRevision"),
    ))
}

fn look_running() -> Running {
    let toplevel = fs::read_link("/run/current-system").ok();
    let (version, nixpkgs, revision) = toplevel.as_deref().and_then(version_of).unwrap_or_default();

    Running {
        toplevel,
        booted: fs::read_link("/run/booted-system").ok(),
        version: Some(version).filter(|v| !v.is_empty()),
        nixpkgs,
        revision,
        kernel: fs::read_to_string("/proc/sys/kernel/osrelease")
            .ok()
            .map(|k| k.trim().to_string()),
        looked: true,
    }
}

impl Service for Running {
    fn new() -> Self {
        Self::default()
    }

    fn interval() -> Duration {
        Duration::from_secs(5)
    }

    // only a switch moves the link; nixos-version runs again then
    fn update(&mut self) -> bool {
        let toplevel = fs::read_link("/run/current-system").ok();
        if self.looked && toplevel == self.toplevel {
            return false;
        }
        *self = look_running();
        true
    }
}

impl Running {
    pub fn refresh() {
        thread::spawn(|| {
            let fresh = look_running();
            let mut now = Self::write();
            if fresh == *now {
                now.quiet();
            } else {
                *now = fresh;
            }
        });
    }

    /// The commit without `-dirty`.
    pub fn commit(&self) -> Option<&str> {
        self.revision
            .as_deref()
            .map(|rev| rev.strip_suffix("-dirty").unwrap_or(rev))
    }
}

// ───── the last Apply ────────────────────────────────────────────────────────

/// What the last Apply from here made: a system built from a dirty tree
/// records `<sha>-dirty`, and the commit after it moves HEAD, so this is
/// how the two still count as one.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub head: String,
    pub toplevel: PathBuf,
}

fn applied_path() -> PathBuf {
    state_dir().join("applied.json")
}

impl Applied {
    pub fn load() -> Option<Self> {
        let json = fs::read_to_string(applied_path()).ok()?;
        let value: Value = serde_json::from_str(&json).ok()?;
        Some(Self {
            head: value.get("head")?.as_str()?.to_string(),
            toplevel: PathBuf::from(value.get("toplevel")?.as_str()?),
        })
    }

    pub fn save(&self) {
        let path = applied_path();
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        let json = serde_json::json!({
            "head": self.head,
            "toplevel": self.toplevel.to_string_lossy(),
        });
        let _ = fs::write(path, json.to_string());
    }
}

// ───── what the tree builds ──────────────────────────────────────────────────

/// The system the working tree evaluates to, for the exact answer to "is
/// the running system this configuration?". Takes seconds, so it comes
/// after the quicker signs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Toplevel {
    pub path: Option<PathBuf>,
    pub error: Option<String>,
    pub loading: bool,
}

static EVAL: AtomicU64 = AtomicU64::new(0);

impl Service for Toplevel {
    fn new() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }

    fn listen() {
        eval(EVAL.fetch_add(1, Ordering::SeqCst) + 1);
    }
}

impl Toplevel {
    pub fn refresh() {
        let number = EVAL.fetch_add(1, Ordering::SeqCst) + 1;
        Self::write().loading = true;
        thread::spawn(move || eval(number));
    }
}

fn eval(number: u64) {
    let attr = format!(
        "{}#nixosConfigurations.{}.config.system.build.toplevel",
        repo().display(),
        host()
    );
    let result = nix(&["eval", "--raw", &attr]);
    if EVAL.load(Ordering::SeqCst) != number {
        return;
    }

    let mut now = Toplevel::write();
    now.loading = false;
    match result {
        Ok(path) => {
            now.path = Some(PathBuf::from(path.trim()));
            now.error = None;
        }
        Err(error) => now.error = Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_revision_shortens_and_keeps_its_dirt() {
        assert_eq!(short("5d056b8af6262450c485ab8abe3d4e376ba267d1"), "5d056b8");
        assert_eq!(
            short("5d056b8af6262450c485ab8abe3d4e376ba267d1-dirty"),
            "5d056b8-dirty"
        );
        assert_eq!(short("abc"), "abc");
    }
}
