//! The phone's apps in the launcher: found by name, each opens in a window of
//! its own (`amane-mirror --app`, a virtual display on the phone). The
//! list comes from `~/.cache/amane-shell/phone-apps.json`, which
//! `amane-mirror --list-apps` writes; it's asked for again when adb has
//! the phone and the list is missing, another phone's, or a day old.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use amane::serde::Deserialize;
use amane::{Adb, KdeConnect, Service};

use amane_kit::icons::ICON_PHONE;

use super::entries::{Action, Entry, Icon};
use super::mode::Mode;
use super::provider::{Candidate, Group, Provider, Query};
use super::rank;

/// How long a list is kept before it's asked for again.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

/// A phone's app counts a little under one of this computer's.
const WEIGHT: f32 = 0.9;

#[derive(Deserialize, Clone)]
#[serde(crate = "amane::serde")]
struct App {
    name: String,
    package: String,
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
        let _ = std::process::Command::new("amane-mirror")
            .args(["--list-apps", "--serial", &serial, "--device", &device])
            .status();
        LISTING.store(false, Ordering::SeqCst);
    });
}

// a word for `sh -c`, as amane's spawn runs commands
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

pub(super) struct PhoneApps;

impl Provider for PhoneApps {
    fn id(&self) -> &'static str {
        "phone-apps"
    }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Home, Mode::Apps]
    }

    fn find(&self, query: &Query, out: &mut Vec<Candidate>) {
        if query.needle.is_empty() {
            return;
        }

        // the phone KDE Connect has now
        let Some(phone) = KdeConnect::read().primary().cloned() else {
            return;
        };

        let serial = {
            let adb = Adb::read();
            phone
                .addresses()
                .iter()
                .find_map(|address| adb.ready_at(*address))
                .map(|device| device.serial.clone())
        };

        let list = list();
        if let Some(serial) = &serial {
            refresh(phone.id(), serial, list.as_ref());
        }

        let Some(list) = list.filter(|list| list.device == phone.id()) else {
            return;
        };

        for app in &list.apps {
            let Some(matched) = rank::fuzzy(&query.needle, &app.name) else {
                continue;
            };
            let command = format!(
                "amane-mirror --device {} --app {} --name {}",
                quoted(phone.id()),
                quoted(&app.package),
                quoted(&app.name)
            );
            let entry = Entry::new(
                format!("phone-app:{}", app.package),
                app.name.clone(),
                Icon::Glyph(ICON_PHONE),
                Action::Run(Rc::new(move || amane::spawn_activated(&command))),
            )
            .subtitle(format!("On {}", phone.name()))
            .detail(app.package.clone());
            out.push(Candidate {
                entry,
                matched: matched * WEIGHT,
                group: Group::Ranked,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_a_name_with_an_apostrophe() {
        assert_eq!(quoted("Kid's"), r"'Kid'\''s'");
    }

    #[test]
    fn reads_the_list_amane_mirror_writes() {
        let list: List = serde_json::from_str(
            r#"{"device":"abc","updated":1,"apps":[{"name":"Signal","package":"org.thoughtcrime.securesms","system":false}]}"#,
        )
        .expect("a list");
        assert_eq!(list.apps[0].package, "org.thoughtcrime.securesms");
    }
}
