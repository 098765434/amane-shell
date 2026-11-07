//! The phone's apps, for the launcher to find by name; each opens in a
//! window of its own (`amane-mirror --app`, a virtual display on the
//! phone). The list comes from `~/.cache/amane-shell/phone-apps.json`,
//! which `amane-mirror --list-apps` writes; it's asked for again when adb
//! has the phone and the list is missing, another phone's, or a day old.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use amane::serde::Deserialize;
use amane_phone::PhoneDevice;

use super::{MIRROR_APP, quoted, screen};

/// How long a list is kept before it's asked for again.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

/// One of the phone's apps.
#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
pub struct App {
    pub name: String,
    pub package: String,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
struct List {
    device: String,
    updated: u64,
    apps: Vec<App>,
}

thread_local! {
    // the list as last read, and when its file was written
    static CACHED: RefCell<Option<(SystemTime, List)>> = const { RefCell::new(None) };
}

// a listing under way, so another isn't started
static LISTING: AtomicBool = AtomicBool::new(false);

fn file() -> Option<PathBuf> {
    let cache = amane_kit::paths::cache();
    Some(cache.join("amane-shell").join("phone-apps.json"))
}

fn list() -> Option<List> {
    let path = file()?;
    let modified = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()?;

    let cached = CACHED.with_borrow(|cached| {
        cached
            .as_ref()
            .filter(|(when, _)| *when == modified)
            .map(|(_, list)| list.clone())
    });
    if cached.is_some() {
        return cached;
    }

    let list: List = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    CACHED.set(Some((modified, list.clone())));
    Some(list)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

/// Asks the phone for its apps, when adb has it and the list is old.
fn refresh(device: &str, serial: &str, list: Option<&List>) {
    let fresh = list.is_some_and(|list| {
        list.device == device && now().saturating_sub(list.updated) < FRESH.as_secs()
    });
    if fresh || LISTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let (device, serial) = (device.to_string(), serial.to_string());
    std::thread::spawn(move || {
        let listed = std::process::Command::new(MIRROR_APP)
            .args(["--list-apps", "--serial", &serial, "--device", &device])
            .status();
        match listed {
            Ok(status) if !status.success() => log::warn!("amane-mirror --list-apps: {status}"),
            Err(error) => log::warn!("amane-mirror --list-apps: {error}"),
            Ok(_) => {}
        }
        LISTING.store(false, Ordering::SeqCst);
    });
}

/// `phone`'s apps, as last listed; asks for the list again when adb has the
/// phone and it's old.
pub fn of(phone: &PhoneDevice) -> Vec<App> {
    let list = list();
    if let Some(serial) = screen::serial(phone) {
        refresh(phone.id(), &serial, list.as_ref());
    }
    list.filter(|list| list.device == phone.id())
        .map(|list| list.apps)
        .unwrap_or_default()
}

/// Opens `app` of `phone` in a window of its own.
pub fn open(phone: &str, app: &App) {
    amane::spawn_activated(&format!(
        "{MIRROR_APP} --device {} --app {} --name {}",
        quoted(phone),
        quoted(&app.package),
        quoted(&app.name)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_list_amane_mirror_writes() {
        let list: List = serde_json::from_str(
            r#"{"device":"abc","updated":1,"apps":[{"name":"Signal","package":"org.thoughtcrime.securesms","system":false}]}"#,
        )
        .expect("a list");
        assert_eq!(list.apps[0].package, "org.thoughtcrime.securesms");
    }
}
