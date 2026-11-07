//! The nixpkgs catalog the Apps page searches: every package `nix search`
//! finds in the configuration's nixpkgs (less nixpkgs' own `tests`), a line each (`attr`, `pname`, `version`,
//! `description`, tab-separated), in a file per locked nixpkgs revision
//! under `$XDG_CACHE_HOME/amane-shell/nixpkgs`. It's built once per nixpkgs
//! update from `nix search --json … ^`, streamed straight into the file, and
//! each search reads the file again (from the page cache, a few hundred
//! thousand lines in tens of milliseconds) rather than the app keeping the
//! packages in memory.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use amane::Service;

use super::apps::{Found, Search};
use super::nixos::repo;

/// How often the page looks at `flake.lock` again for a new nixpkgs.
const RECHECK: Duration = Duration::from_secs(10);

static CHECKED: Mutex<Option<Instant>> = Mutex::new(None);

/// Where the catalog stands: built for `rev` (`ready`, `count` packages),
/// being built since `building`, or failed with `error`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub rev: Option<String>,
    pub ready: bool,
    pub count: usize,
    pub building: Option<Instant>,
    pub error: Option<String>,
}

impl Service for Catalog {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

/// `$XDG_CACHE_HOME/amane-shell/nixpkgs`.
fn cache_dir() -> PathBuf {
    let cache = amane_kit::paths::cache();

    cache.join("amane-shell").join("nixpkgs")
}

fn path_for(rev: &str) -> PathBuf {
    amane_nixos::catalog::file_for(&cache_dir(), rev)
}

use amane_nixos::catalog::write_catalog;
pub use amane_nixos::catalog::{find, instead_of, locked_rev, lookup_in};

impl Catalog {
    /// Makes sure there's a catalog for the nixpkgs `flake.lock` pins,
    /// building it when there isn't; looks at most every [`RECHECK`].
    pub fn ensure() {
        {
            let mut checked = CHECKED.lock().unwrap_or_else(PoisonError::into_inner);
            if checked.is_some_and(|at| at.elapsed() < RECHECK) {
                return;
            }
            *checked = Some(Instant::now());
        }
        thread::spawn(Self::check);
    }

    fn check() {
        if Self::read().building.is_some() {
            return;
        }
        let rev = match fs::read_to_string(repo().join("flake.lock")) {
            Ok(lock) => locked_rev(&lock),
            Err(error) => {
                Self::fail(format!("flake.lock: {error}"));
                return;
            }
        };
        let Some(rev) = rev else {
            Self::fail(String::from("flake.lock pins no nixpkgs input"));
            return;
        };
        {
            let now = Self::read();
            if now.ready && now.rev.as_deref() == Some(rev.as_str()) {
                return;
            }
        }

        let path = path_for(&rev);
        match fs::read(&path) {
            Ok(bytes) => {
                let count = bytes.iter().filter(|&&b| b == b'\n').count();
                *Self::write() = Self {
                    rev: Some(rev),
                    ready: true,
                    count,
                    building: None,
                    error: None,
                };
                Search::resume();
            }
            Err(_) => Self::build(rev, path),
        }
    }

    fn fail(error: String) {
        let mut now = Self::write();
        now.building = None;
        now.ready = false;
        now.error = Some(error);
        drop(now);
        // the page searches with nix instead
        Search::resume();
    }

    fn build(rev: String, path: PathBuf) {
        {
            let mut now = Self::write();
            now.rev = Some(rev.clone());
            now.ready = false;
            now.building = Some(Instant::now());
            now.error = None;
        }
        // redraw each second, for the time it's taken so far
        thread::spawn(|| {
            loop {
                thread::sleep(Duration::from_secs(1));
                if Self::read().building.is_none() {
                    break;
                }
                drop(Self::write());
            }
        });

        match write_catalog(&super::nixos::config(), &path) {
            Ok(count) => {
                // the catalogs of earlier revisions
                if let Ok(entries) = fs::read_dir(cache_dir()) {
                    for entry in entries.flatten() {
                        if entry.path() != path {
                            // an old catalog left behind is only space; the next build tries again
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
                *Self::write() = Self {
                    rev: Some(rev),
                    ready: true,
                    count,
                    building: None,
                    error: None,
                };
                Search::resume();
            }
            Err(error) => Self::fail(error),
        }
    }

    /// The file to search, while there's one for the pinned nixpkgs.
    pub fn file(&self) -> Option<PathBuf> {
        self.rev.as_deref().filter(|_| self.ready).map(path_for)
    }
}

/// What the catalog says of the packages added in Settings, so their rows
/// can show a name, version and icon; and for one it hasn't got (renamed or
/// dropped), the closest package by the last part of its name.
#[derive(Default)]
pub struct Named {
    pub found: HashMap<String, Found>,
    pub instead: HashMap<String, String>,
}

impl Service for Named {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

static NAMING: Mutex<Option<(PathBuf, Vec<String>)>> = Mutex::new(None);

impl Named {
    /// Looks the packages up when the list or the catalog changed.
    pub fn ensure(file: PathBuf, attrs: Vec<String>) {
        let key = (file, attrs);
        {
            let mut naming = NAMING.lock().unwrap_or_else(PoisonError::into_inner);
            if naming.as_ref() == Some(&key) {
                return;
            }
            *naming = Some(key.clone());
        }
        thread::spawn(move || {
            let text = fs::read_to_string(&key.0).unwrap_or_default();
            let found = lookup_in(&text, &key.1);
            let instead = key
                .1
                .iter()
                .filter(|attr| !found.contains_key(*attr))
                .filter_map(|attr| Some((attr.clone(), instead_of(&text, attr)?)))
                .collect();
            let mut now = Self::write();
            now.found = found;
            now.instead = instead;
        });
    }
}
