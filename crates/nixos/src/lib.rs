//! Emaki Settings' NixOS model and jobs, with no UI.
//!
//! - [`settings`]: the settings the configuration has (the flake's
//!   `emakiSettings` output) and `managed/settings.json`, the one file
//!   Settings writes.
//! - [`job`]: build the system as the user, compare it with the running
//!   one, activate it as root through polkit; a fresh `flake.lock` for an
//!   update. Each step reports as it goes and can be stopped.
//! - [`lock`]: a `flake.lock`'s inputs and what a fresh one moves.
//! - [`generations`], [`repo`]: the system profile's generations and what
//!   a system says about itself.
//! - [`catalog`]: the nixpkgs catalog the Apps page searches.
//! - [`xkb`]: the keyboard layouts, for checking what's typed.
//!
//! Nothing here keeps state between calls or knows about a window: Settings
//! holds the state and runs these off its UI thread. Every command goes
//! through [`Config`], so tests run the pipeline against fake programs.

#![deny(missing_docs)]
// no unsafe code: what needs it (syscalls, FFI) lives in emaki-base or amane
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use emaki_process::Cmd;

pub mod catalog;
pub mod generations;
pub mod job;
pub mod lock;
pub mod repo;
pub mod settings;
pub mod xkb;

/// The file Settings writes, in the repo.
pub const SETTINGS_FILE: &str = "managed/settings.json";

/// The module that reads it: a setting only it sets is ours to change.
pub const MANAGED_MODULE: &str = "modules/managed.nix";

/// Where the configuration is, and the programs and system paths the jobs
/// use (the real ones from [`Config::new`]; tests put fakes in their place).
#[derive(Clone, Debug)]
pub struct Config {
    /// The flake repo.
    pub repo: PathBuf,
    /// The `nixosConfigurations` attribute to build.
    pub host: String,
    /// `nix`.
    pub nix: OsString,
    /// `git`.
    pub git: OsString,
    /// `pkexec`, which runs the activation helper as root.
    pub pkexec: OsString,
    /// The activation helper in the running system; polkit's action names
    /// the store path it resolves to.
    pub helper: PathBuf,
    /// `/nix/var/nix/profiles`, where the system's generations are.
    pub profiles: PathBuf,
    /// `/run/current-system`.
    pub current_system: PathBuf,
    /// `/run/booted-system`.
    pub booted_system: PathBuf,
}

impl Config {
    /// The configuration in `repo`, built as `host`, with the real programs.
    pub fn new(repo: impl Into<PathBuf>, host: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            host: host.into(),
            nix: OsString::from("nix"),
            git: OsString::from("git"),
            pkexec: OsString::from("pkexec"),
            helper: PathBuf::from("/run/current-system/sw/bin/emaki-nixos-activate"),
            profiles: PathBuf::from("/nix/var/nix/profiles"),
            current_system: PathBuf::from("/run/current-system"),
            booted_system: PathBuf::from("/run/booted-system"),
        }
    }

    /// git in the repo, syncing what it writes: git leaves loose objects
    /// unsynced by default, and a crash after a commit then leaves it as
    /// empty files, a HEAD git can't read.
    pub fn git(&self, args: &[&str]) -> Result<String, String> {
        Cmd::new(&self.git)
            .args(["-c", "core.fsync=committed"])
            .args(args)
            .dir(&self.repo)
            .stdout()
    }

    /// nix in the repo, without the dirty-tree warning.
    pub fn nix(&self, args: &[&str]) -> Result<String, String> {
        self.nix_cmd().args(args).stdout()
    }

    fn nix_cmd(&self) -> Cmd {
        Cmd::new(&self.nix).arg("--no-warn-dirty").dir(&self.repo)
    }

    /// A file in the repo.
    pub fn in_repo(&self, path: impl AsRef<Path>) -> PathBuf {
        self.repo.join(path)
    }
}

/// Runs a program to its end: what it printed on success, else the last
/// lines of what it said.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = Cmd::new(program).args(args);
    if let Some(cwd) = cwd {
        command = command.dir(cwd);
    }
    command.stdout()
}
