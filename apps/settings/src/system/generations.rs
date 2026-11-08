//! The system generations, from `/nix/var/nix/profiles/system-N-link`: what
//! History lists and Restore goes back to.

use std::path::PathBuf;
use std::thread;

use amane::{Service, watch_file};
use emaki_kit::service::ServiceExt as _;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Generations {
    /// Newest first.
    pub list: Vec<Generation>,
    pub current: Option<PathBuf>,
    pub booted: Option<PathBuf>,
    /// The generation the `system` profile points at.
    pub profile: Option<u32>,
    pub loading: bool,
}

pub use emaki_nixos::generations::Generation;

fn scan() -> Generations {
    let found = emaki_nixos::generations::scan(&super::nixos::config());
    Generations {
        list: found.list,
        current: found.current,
        booted: found.booted,
        profile: found.profile,
        loading: false,
    }
}

fn store(fresh: Generations) {
    Generations::replace(fresh);
}

impl Service for Generations {
    fn new() -> Self {
        Self {
            loading: true,
            ..Self::default()
        }
    }

    // the profile link moves on every switch and roll-back
    fn listen() {
        store(scan());
        let link = super::nixos::config().profiles.join("system");
        let link = link.to_string_lossy();
        for _ in watch_file(&link) {
            store(scan());
        }
    }
}

impl Generations {
    pub fn refresh() {
        thread::spawn(|| store(scan()));
    }
}
