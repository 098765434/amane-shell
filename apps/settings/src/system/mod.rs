//! The System pages: the NixOS configuration in `~/nixos-config`, read
//! through the flake's `amaneSettings` output, changed through
//! `managed/settings.json`, built as the user and activated through polkit
//! ([nixos.md](../../../../docs/integration/nixos.md)).

pub mod about;
pub mod apply_bar;
pub mod apps;
pub mod controls;
pub mod generations;
pub mod history;
pub mod job;
pub mod nixos;
pub mod power;
pub mod repo;
pub mod services;
pub mod storage;
pub mod update;

use std::path::PathBuf;

use amane::Service;

use nixos::Managed;
use repo::{Applied, Repo, Running, Toplevel};

/// `$XDG_STATE_HOME/amane-shell/nixos`, for the record of the last Apply.
pub fn state_dir() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_default();

    state.join("amane-shell").join("nixos")
}

/// Why the running system isn't the configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum Reason {
    /// Settings changed here, waiting for Apply.
    Edits(usize),
    /// Other files with uncommitted changes.
    Dirty(Vec<String>),
    /// HEAD isn't the commit the running system was built from.
    HeadMoved { commits: Option<usize> },
    /// The tree evaluates to a different system.
    Exact,
}

impl Reason {
    pub fn text(&self) -> String {
        match self {
            Self::Edits(1) => String::from("1 setting changed, not applied"),
            Self::Edits(n) => format!("{n} settings changed, not applied"),
            Self::Dirty(files) if files.len() == 1 => {
                format!("{} has uncommitted changes", files[0])
            }
            Self::Dirty(files) => format!("{} files have uncommitted changes", files.len()),
            Self::HeadMoved { commits: Some(1) } => String::from("1 commit not applied"),
            Self::HeadMoved { commits: Some(n) } => format!("{n} commits not applied"),
            Self::HeadMoved { commits: None } => String::from("Built from another commit"),
            Self::Exact => String::from("The configuration differs from the running system"),
        }
    }
}

/// Whether the running system is this configuration.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Checking,
    UpToDate,
    Pending(Vec<Reason>),
    /// Can't say: the running system doesn't record its commit yet.
    Unknown(String),
}

/// From the quick signs (the file, git) to the exact one (what the tree
/// evaluates to), in that order.
pub fn status() -> Status {
    let managed = Managed::read();
    let repo = Repo::read();
    let running = Running::read();
    let toplevel = Toplevel::read();

    if !repo.looked || !running.looked {
        return Status::Checking;
    }

    let mut reasons = Vec::new();
    let edits = managed.changes().len();
    if edits > 0 {
        reasons.push(Reason::Edits(edits));
    }
    let dirty: Vec<String> = repo
        .dirty_elsewhere()
        .into_iter()
        .map(String::from)
        .collect();
    if !dirty.is_empty() {
        reasons.push(Reason::Dirty(dirty));
    }

    let Some(head) = repo.head.as_deref() else {
        return Status::Unknown(
            repo.error
                .clone()
                .unwrap_or_else(|| String::from("No git repo")),
        );
    };

    // the exact answer, when it's in
    let exact = match (&toplevel.path, &running.toplevel) {
        (Some(evaluated), Some(current)) if !toplevel.loading => Some(evaluated == current),
        _ => None,
    };

    match running.commit() {
        Some(commit) if commit == head => {}
        // a build from a dirty tree, then the commit of its edits: still one
        Some(_)
            if Applied::load().is_some_and(|applied| {
                applied.head == head && Some(&applied.toplevel) == running.toplevel.as_ref()
            }) => {}
        Some(commit) => {
            if exact != Some(true) {
                let _ = commit;
                reasons.push(Reason::HeadMoved {
                    commits: repo.past_running,
                });
            }
        }
        None => {
            if reasons.is_empty() {
                return match exact {
                    Some(true) => Status::UpToDate,
                    Some(false) => Status::Pending(vec![Reason::Exact]),
                    None => Status::Unknown(String::from(
                        "The running system doesn't record its commit; apply once to know.",
                    )),
                };
            }
        }
    }

    if reasons.is_empty() && exact == Some(false) {
        reasons.push(Reason::Exact);
    }

    if reasons.is_empty() {
        Status::UpToDate
    } else {
        Status::Pending(reasons)
    }
}
