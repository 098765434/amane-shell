//! Which of the running system's packages are apps: the desktop entries in
//! `/run/current-system/sw/share/applications` are links into the store,
//! so each one's store path names the package that put it there
//! (`…-zed-editor-1.3.6/share/applications/dev.zed.Zed.desktop`). The Apps
//! page draws an installed package with its entry's name and icon, and can
//! open it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use amane::{Apps, DesktopApp, Service};

const APPLICATIONS: &str = "/run/current-system/sw/share/applications";

/// A store path's name (`zed-editor-1.3.6`) to the desktop entries it puts in
/// the system profile (`dev.zed.Zed.desktop`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Launchers {
    by_package: BTreeMap<String, Vec<String>>,
}

/// `/nix/store/<32-character hash>-<name>/…` to `<name>`.
fn store_name(path: &Path) -> Option<&str> {
    let mut parts = path.strip_prefix("/nix/store").ok()?.components();
    let first = parts.next()?.as_os_str().to_str()?;
    first.get(33..).filter(|name| !name.is_empty())
}

fn look() -> Launchers {
    let mut by_package: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(APPLICATIONS) else {
        return Launchers::default();
    };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if !id.ends_with(".desktop") {
            continue;
        }
        let Ok(target) = fs::canonicalize(entry.path()) else {
            continue;
        };
        if let Some(name) = store_name(&target) {
            by_package.entry(name.to_string()).or_default().push(id);
        }
    }
    for ids in by_package.values_mut() {
        ids.sort();
    }
    Launchers { by_package }
}

impl Service for Launchers {
    fn new() -> Self {
        look()
    }

    // an Apply changes the profile
    fn interval() -> Duration {
        Duration::from_secs(30)
    }

    fn update(&mut self) -> bool {
        let fresh = look();
        let changed = fresh != *self;
        *self = fresh;
        changed
    }
}

impl Launchers {
    /// Looks again now: an Apply just changed the profile.
    pub fn refresh() {
        std::thread::spawn(|| {
            let fresh = look();
            let mut now = Self::write();
            if fresh == *now {
                now.quiet();
            } else {
                *now = fresh;
            }
        });
    }

    /// The desktop entry ids a package with this `pname` and `version`
    /// installs, also when it's a wrapper round `<pname>-unwrapped`, as
    /// Steam and Heroic are.
    pub fn ids(&self, pname: &str, version: &str) -> &[String] {
        if pname.is_empty() {
            return &[];
        }
        let names = [
            format!("{pname}-{version}"),
            format!("{pname}-unwrapped-{version}"),
        ];
        names
            .iter()
            .find_map(|name| self.by_package.get(name))
            .map_or(&[], Vec::as_slice)
    }

    /// The app a package installs: of the entries it puts in the launcher
    /// (hidden ones left out), the one named after it, else the shortest
    /// (`foot.desktop` over `footclient.desktop`).
    pub fn app(&self, apps: &Apps, pname: &str, version: &str) -> Option<DesktopApp> {
        let ids = self.ids(pname, version);
        let listed: Vec<&DesktopApp> = apps
            .list()
            .iter()
            .filter(|app| ids.iter().any(|id| id == app.id()))
            .collect();
        let needle = pname.to_lowercase();
        listed
            .iter()
            .find(|app| app.id().to_lowercase().contains(&needle))
            .or_else(|| listed.iter().min_by_key(|app| app.id().len()))
            .map(|app| (*app).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_path_names_its_package() {
        let path = Path::new(
            "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-zed-editor-1.3.6/share/applications/dev.zed.Zed.desktop",
        );
        assert_eq!(store_name(path), Some("zed-editor-1.3.6"));
        assert_eq!(
            store_name(Path::new("/usr/share/applications/x.desktop")),
            None
        );
    }

    #[test]
    fn a_wrapper_finds_its_unwrapped_package() {
        let mut launchers = Launchers::default();
        launchers.by_package.insert(
            "heroic-unwrapped-2.22.1".into(),
            vec!["com.heroicgameslauncher.hgl.desktop".into()],
        );
        launchers
            .by_package
            .insert("foot-1.27.0".into(), vec!["foot.desktop".into()]);
        assert_eq!(
            launchers.ids("heroic", "2.22.1"),
            ["com.heroicgameslauncher.hgl.desktop"]
        );
        assert_eq!(launchers.ids("foot", "1.27.0"), ["foot.desktop"]);
        assert!(launchers.ids("foot", "1.28.0").is_empty());
        assert!(launchers.ids("", "").is_empty());
    }
}
