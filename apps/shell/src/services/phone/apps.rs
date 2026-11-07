//! The phone's apps, for the launcher to find by name; each opens in a
//! window of its own (`amane-mirror --app`, a virtual display on the
//! phone). The list comes from `~/.cache/amane-shell/phone-apps.json`,
//! which `amane-mirror --list-apps` writes; it's asked for again when adb
//! has the phone and the list is missing, another phone's, or a day old.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use amane_phone::PhoneDevice;

use super::{MIRROR_APP, screen};

/// How long a list is kept before it's asked for again.
const FRESH: Duration = Duration::from_secs(24 * 60 * 60);

pub use amane_phone::shared::apps::App;
use amane_phone::shared::apps::{self as shared, List};

// a listing under way, so another isn't started
static LISTING: AtomicBool = AtomicBool::new(false);

/// Asks the phone for its apps, when adb has it and the list is old.
fn refresh(device: &str, serial: &str, list: Option<&List>) {
    let fresh = list.is_some_and(|list| {
        list.device == device
            && amane_kit::time::unix_s().saturating_sub(list.updated) < FRESH.as_secs()
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
    let list = shared::read();
    if let Some(serial) = screen::serial(phone) {
        refresh(phone.id(), &serial, list.as_ref());
    }
    list.filter(|list| list.device == phone.id())
        .map(|list| list.apps)
        .unwrap_or_default()
}

/// Opens `app` of `phone` in a window of its own.
pub fn open(phone: &str, app: &App) {
    amane::Launch::argv(&[
        MIRROR_APP,
        "--device",
        phone,
        "--app",
        &app.package,
        "--name",
        &app.name,
    ])
    .activated();
}
