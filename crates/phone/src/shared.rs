//! The files about the phone that the shell, Messages and Mirror share,
//! each written whole and read again only when it changed.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use amane::serde::{Deserialize, Serialize, de::DeserializeOwned};
use emaki_base::sync::Locked as _;
use emaki_base::{app_ids, fs, paths};

/// `path` read as JSON, kept in `cache` until the file changes: views ask
/// on every draw.
fn read_cached<T: DeserializeOwned + Clone>(
    path: &Path,
    cache: &Mutex<Option<(Option<SystemTime>, T)>>,
) -> Option<T> {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    let mut cache = cache.locked();
    if let Some((when, value)) = cache.as_ref()
        && *when == modified
    {
        return Some(value.clone());
    }
    let value: T = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    *cache = Some((modified, value.clone()));
    Some(value)
}

/// The phone's apps, `~/.cache/emaki/phone-apps.json`: Mirror lists
/// them (`emaki-mirror --list-apps`), the launcher finds them by name, and
/// Messages finds the phone's messaging app among them.
pub mod apps {
    use super::*;

    /// One of the phone's apps.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    #[serde(crate = "amane::serde")]
    pub struct App {
        /// What it's called.
        pub name: String,
        /// Its Android package.
        pub package: String,
        /// Whether it came with the phone.
        #[serde(default)]
        pub system: bool,
    }

    /// The apps of one phone.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    #[serde(crate = "amane::serde")]
    pub struct List {
        /// KDE Connect's id for the phone.
        pub device: String,
        /// When it was listed, in seconds since 1970.
        pub updated: u64,
        /// Its apps.
        pub apps: Vec<App>,
    }

    static CACHE: Mutex<Option<(Option<SystemTime>, List)>> = Mutex::new(None);

    /// Where the list is kept.
    pub fn path() -> PathBuf {
        paths::cache().join(app_ids::SHELL).join("phone-apps.json")
    }

    /// The list kept, if there is one.
    pub fn read() -> Option<List> {
        read_cached(&path(), &CACHE)
    }

    /// Keeps `list`.
    pub fn write(list: &List) -> io::Result<()> {
        fs::save_json(&path(), list)
    }
}

/// The phones whose screen was set up, `~/.config/emaki/phone-screen.json`.
pub mod screen {
    use super::*;

    #[derive(Default, Serialize, Deserialize, Clone)]
    #[serde(crate = "amane::serde")]
    struct Remembered {
        /// KDE Connect ids of the phones.
        devices: BTreeSet<String>,
    }

    static CACHE: Mutex<Option<(Option<SystemTime>, Remembered)>> = Mutex::new(None);

    fn path() -> PathBuf {
        paths::config()
            .join(app_ids::SHELL)
            .join("phone-screen.json")
    }

    /// Whether `device`'s screen was set up before.
    pub fn set_up(device: &str) -> bool {
        read_cached::<Remembered>(&path(), &CACHE)
            .is_some_and(|known| known.devices.contains(device))
    }

    /// Remembers that `device`'s screen was set up.
    pub fn remember(device: &str) -> io::Result<()> {
        let mut known = read_cached::<Remembered>(&path(), &CACHE).unwrap_or_default();
        if !known.devices.insert(device.to_string()) {
            return Ok(());
        }
        fs::save_json(&path(), &known)
    }
}

#[cfg(test)]
mod tests {
    use super::apps::List;

    #[test]
    fn reads_the_list_emaki_mirror_writes() {
        let list: List = serde_json::from_str(
            r#"{"device":"abc","updated":1,"apps":[{"name":"Signal","package":"org.thoughtcrime.securesms","system":false}]}"#,
        )
        .expect("a list");
        assert_eq!(list.apps[0].package, "org.thoughtcrime.securesms");
    }
}
