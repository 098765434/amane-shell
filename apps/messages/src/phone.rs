//! The phone's screen (`amane-mirror`) from Messages: whether it can be
//! shown, and the phone's own messaging app to show in a window.

use amane::Service;
use amane_phone::{Adb, PhoneDevice};

/// The phone's messaging apps, the one Samsung ships first.
const MESSAGING: [&str; 2] = [
    "com.samsung.android.messaging",
    "com.google.android.apps.messaging",
];

/// The phone's screen can be shown: adb has the phone at an address KDE
/// Connect knows, or its screen was set up on the Phone page before.
pub fn screen_available(device: &PhoneDevice) -> bool {
    let adb = Adb::read();
    if device
        .addresses()
        .iter()
        .any(|address| adb.ready_at(*address).is_some())
    {
        return true;
    }
    drop(adb);

    amane_phone::shared::screen::set_up(device.id())
}

/// The phone's messaging app, its package and name, from the list of its
/// apps `amane-mirror --list-apps` keeps.
pub fn messaging_app(device: &PhoneDevice) -> Option<(String, String)> {
    let list = amane_phone::shared::apps::read().filter(|list| list.device == device.id())?;
    MESSAGING.iter().find_map(|wanted| {
        let app = list.apps.iter().find(|app| app.package == *wanted)?;
        Some((app.package.clone(), app.name.clone()))
    })
}

/// Opens `package` from the phone in a window of its own.
pub fn open_app(device: &PhoneDevice, package: &str, name: &str) {
    amane::Launch::argv(&[
        "amane-mirror",
        "--device",
        device.id(),
        "--app",
        package,
        "--name",
        name,
    ])
    .activated();
}
