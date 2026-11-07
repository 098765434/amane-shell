//! A phone's screen: whether it can be shown (adb has the phone, or it was
//! set up before), and opening it in `amane-mirror`. A phone set up once is
//! remembered in `~/.config/amane-shell/phone-screen.json`; Messages reads
//! that file too, looking for the phone's id in quotes.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

use amane::Service;
use amane::serde::{Deserialize, Serialize};
use amane_phone::{Adb, PhoneDevice};

use super::MIRROR_APP;

type Known = Option<(Option<std::time::SystemTime>, BTreeSet<String>)>;

thread_local! {
    // the remembered phones, and when their file was written
    static KNOWN: RefCell<Known> = const { RefCell::new(None) };
}

#[derive(Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde")]
struct Remembered {
    /// KDE Connect ids of the phones whose screen was set up.
    devices: BTreeSet<String>,
}

fn file() -> Option<PathBuf> {
    let config = amane_kit::paths::config();
    Some(config.join("amane-shell").join("phone-screen.json"))
}

fn remembered() -> Remembered {
    let Some(path) = file() else {
        return Remembered::default();
    };
    // read again only when the file changed: cards ask on every draw
    let modified = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok();
    if let Some(known) = KNOWN.with_borrow(|known| {
        known
            .as_ref()
            .filter(|(when, _)| *when == modified)
            .map(|(_, devices)| devices.clone())
    }) {
        return Remembered { devices: known };
    }
    let known: Remembered = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    KNOWN.set(Some((modified, known.devices.clone())));
    known
}

pub fn remember(device: &str) {
    let mut known = remembered();
    if !known.devices.insert(device.to_string()) {
        return;
    }
    let Some(path) = file() else {
        return;
    };
    // Messages reads this file too, looking for the id in quotes
    if let Ok(text) = serde_json::to_string_pretty(&known)
        && let Err(error) = amane_kit::fs::write_whole(&path, text)
    {
        log::warn!("couldn't save {}: {error}", path.display());
    }
}

/// Whether `device`'s screen was set up before.
pub fn set_up(device: &str) -> bool {
    remembered().devices.contains(device)
}

/// Whether adb has `device` now, at one of the addresses KDE Connect knows.
pub fn ready(device: &PhoneDevice) -> bool {
    serial(device).is_some()
}

/// Opens the phone's screen.
pub fn open_screen(device: &str) {
    amane::Launch::argv(&[MIRROR_APP, "--device", device]).activated();
}

/// The serial adb has `device` by, at one of the addresses KDE Connect knows.
pub fn serial(device: &PhoneDevice) -> Option<String> {
    let adb = Adb::read();
    device
        .addresses()
        .iter()
        .find_map(|address| adb.ready_at(*address))
        .map(|found| found.serial.clone())
}
