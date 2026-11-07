//! What the last NixOS update check found: `amane-update-check` (a user
//! timer from the NixOS configuration) relocks the flake beside the repo
//! and writes `nixos/check.json` in the shell's state dir; Shell Settings
//! clears it once an update is applied. The Software Update tile reads it.

use amane::{Service, watch_file};
use amane_kit::service::ServiceExt as _;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Updates {
    /// The inputs with something newer, by name.
    pub changed: Vec<String>,
    /// When the check ran, as seconds since the epoch.
    pub checked: Option<u64>,
    pub error: Option<String>,
}

fn look() -> Updates {
    let check = amane_nixos::lock::read_check();
    Updates {
        changed: check
            .changes
            .into_iter()
            .map(|change| change.name)
            .collect(),
        checked: check
            .checked
            .and_then(|at| u64::try_from(at.timestamp()).ok()),
        error: check.error,
    }
}

impl Service for Updates {
    fn new() -> Self {
        look()
    }

    fn listen() {
        let path = amane_nixos::lock::check_path();
        loop {
            if !path.parent().is_some_and(|folder| folder.is_dir()) {
                std::thread::sleep(std::time::Duration::from_secs(60));
                continue;
            }
            for _ in watch_file(&path.to_string_lossy()) {
                Self::replace(look());
            }
        }
    }
}

impl Updates {
    /// "nixpkgs, amane-shell": what has updates, for a subtitle.
    pub fn names(&self) -> String {
        self.changed.join(", ")
    }
}
