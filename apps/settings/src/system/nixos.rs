//! The NixOS configuration as Settings sees it: the [`Snapshot`] the flake's
//! `emakiSettings` output gives (every setting in `managed/registry.nix`
//! with its effective value, how it's set and which files set it), and
//! [`Managed`], `managed/settings.json`, the one file Settings writes.
//! `modules/managed.nix` turns that file into option definitions, so the
//! hand-written modules are never touched.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Service, watch_file};
use emaki_kit::settings::nixos;

use serde_json::Value;

use emaki_kit::service::ServiceExt as _;
pub use emaki_nixos::{MANAGED_MODULE, SETTINGS_FILE, run};

/// The flake repo, from `nixos.json`.
pub fn repo() -> PathBuf {
    nixos().repo_path()
}

/// The `nixosConfigurations` attribute to build, from `nixos.json`.
pub fn host() -> String {
    nixos().host_attr()
}

/// The configuration as `nixos.json` names it, for the jobs.
pub fn config() -> emaki_nixos::Config {
    emaki_nixos::Config::new(repo(), host())
}

/// Runs git in the repo (see [`emaki_nixos::Config::git`]).
pub fn git(args: &[&str]) -> Result<String, String> {
    config().git(args)
}

/// Runs nix in the repo, without the dirty-tree warning.
pub fn nix(args: &[&str]) -> Result<String, String> {
    config().nix(args)
}

pub use emaki_nixos::settings::{
    Change, Control, Entry, SystemPackage, Tier, parse_map, parse_snapshot, save, show, show_as,
    strings,
};

/// The settings the configuration has, as last evaluated. Taken once at
/// start and again on [`Snapshot::refresh`]; stale while `loading`.
pub struct Snapshot {
    pub entries: Vec<Entry>,
    /// The commit the repo was at, `-dirty` when it had changes.
    pub revision: Option<String>,
    pub host: Option<String>,
    /// What the repo's files install system-wide.
    pub system_packages: Vec<SystemPackage>,
    /// The packages added in Settings this nixpkgs doesn't have: the build
    /// leaves them out, with a warning.
    pub missing_packages: Vec<String>,
    pub error: Option<String>,
    pub loading: bool,
    pub taken: Option<Instant>,
}

// which refresh is the latest, so an older one that ends late is dropped
static TAKE: AtomicU64 = AtomicU64::new(0);

impl Service for Snapshot {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            revision: None,
            host: None,
            system_packages: Vec::new(),
            missing_packages: Vec::new(),
            error: None,
            loading: true,
            taken: None,
        }
    }

    // the first evaluation, off the UI thread; later ones come from refresh
    fn listen() {
        take(TAKE.fetch_add(1, Ordering::SeqCst) + 1);
    }
}

impl Snapshot {
    /// Evaluates again, in the background.
    pub fn refresh() {
        let take_number = TAKE.fetch_add(1, Ordering::SeqCst) + 1;
        Self::write().loading = true;
        thread::spawn(move || take(take_number));
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn for_page(&self, page: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.page == page)
            .collect()
    }
}

fn take(take_number: u64) {
    let flake = format!("{}#emakiSettings", repo().display());
    let result = nix(&["eval", "--json", &flake]).and_then(|json| parse_snapshot(&json));

    if TAKE.load(Ordering::SeqCst) != take_number {
        return;
    }

    let mut now = Snapshot::write();
    now.loading = false;
    now.taken = Some(Instant::now());
    match result {
        Ok(parsed) => {
            now.entries = parsed.entries;
            now.revision = parsed.revision;
            now.host = parsed.host;
            now.system_packages = parsed.system_packages;
            now.missing_packages = parsed.missing_packages;
            now.error = None;
        }
        Err(error) => now.error = Some(error),
    }
}

// ───── managed/settings.json ─────────────────────────────────────────────────

/// `managed/settings.json` as it is on disk (`current`) and as last
/// committed (`committed`): the difference is what's waiting for Apply.
#[derive(Clone, Debug, PartialEq)]
pub struct Managed {
    pub current: BTreeMap<String, Value>,
    pub committed: BTreeMap<String, Value>,
    pub error: Option<String>,
}

pub fn settings_path() -> PathBuf {
    repo().join(SETTINGS_FILE)
}

fn look() -> Managed {
    let path = settings_path();
    let (current, error) = match fs::read_to_string(&path) {
        Ok(json) => match parse_map(&json) {
            Ok(map) => (map, None),
            Err(error) => (BTreeMap::new(), Some(format!("{SETTINGS_FILE}: {error}"))),
        },
        Err(_) => (BTreeMap::new(), None),
    };
    let committed = git(&["show", &format!("HEAD:{SETTINGS_FILE}")])
        .ok()
        .and_then(|json| parse_map(&json).ok())
        .unwrap_or_default();

    Managed {
        current,
        committed,
        error,
    }
}

impl Service for Managed {
    // both reads take a few milliseconds
    fn new() -> Self {
        look()
    }

    // a write from here, by hand, or a checkout: the file's folder is watched
    fn listen() {
        loop {
            let path = settings_path();
            let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
            if !folder.is_dir() {
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            for _ in watch_file(&path.to_string_lossy()) {
                Self::reload();
            }
        }
    }
}

impl Managed {
    /// Reads the file and HEAD's copy again.
    pub fn reload() {
        Self::replace(look());
    }

    /// The value shown for a setting: the file's, else the configuration's.
    pub fn value<'a>(&'a self, id: &str, configured: &'a Value) -> &'a Value {
        self.current.get(id).unwrap_or(configured)
    }

    /// What differs from HEAD.
    pub fn changes(&self) -> Vec<Change> {
        emaki_nixos::settings::changes(&self.current, &self.committed)
    }

    /// Each change as a few words, with the registry's labels where the
    /// snapshot has them: "Time zone → Europe/Oslo", "Tailscale off".
    pub fn summary(&self, snapshot: &Snapshot) -> Vec<String> {
        emaki_nixos::settings::summary(&self.changes(), &snapshot.entries)
    }

    /// A setting's list value as strings (empty when it's not a list).
    pub fn list(&self, id: &str, configured: &Value) -> Vec<String> {
        strings(Some(self.value(id, configured)))
    }

    /// The commit's subject for the changes.
    pub fn commit_message(&self, snapshot: &Snapshot) -> String {
        emaki_nixos::settings::commit_message(&self.changes(), &snapshot.entries)
    }

    /// Writes one setting and stages the file, so the flake sees it. The
    /// caller has checked the value against its entry.
    pub fn set(id: &str, value: Value) {
        // into the live map first, so a second set before the write lands
        // builds on this one
        let map = {
            let mut now = Self::write();
            now.current.insert(String::from(id), value);
            now.current.clone()
        };
        let path = settings_path();

        thread::spawn(move || {
            let result = save(&path, &map).and_then(|()| git(&["add", "--", SETTINGS_FILE]));
            Self::reload();
            if let Err(error) = result {
                Self::write().error = Some(error);
            }
        });
    }

    /// Puts HEAD's copy back.
    pub fn discard() {
        thread::spawn(|| {
            let result = git(&["checkout", "--", SETTINGS_FILE]);
            Self::reload();
            if let Err(error) = result {
                Self::write().error = Some(error);
            }
        });
    }
}
