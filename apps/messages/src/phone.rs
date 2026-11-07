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

    std::fs::read_to_string(amane_kit::paths::config().join("amane-shell/phone-screen.json"))
        .is_ok_and(|text| text.contains(&format!("\"{}\"", device.id())))
}

/// The phone's messaging app, its package and name, from the list of its
/// apps `amane-mirror --list-apps` keeps.
pub fn messaging_app(device: &PhoneDevice) -> Option<(String, String)> {
    let text =
        std::fs::read_to_string(amane_kit::paths::cache().join("amane-shell/phone-apps.json"))
            .ok()?;
    let list: serde_json::Value = serde_json::from_str(&text).ok()?;
    if list.get("device").and_then(|id| id.as_str()) != Some(device.id()) {
        return None;
    }
    let apps = list.get("apps")?.as_array()?;
    MESSAGING.iter().find_map(|wanted| {
        apps.iter().find_map(|app| {
            (app.get("package")?.as_str()? == *wanted).then(|| {
                let name = app.get("name").and_then(|name| name.as_str());
                (wanted.to_string(), name.unwrap_or("Messages").to_string())
            })
        })
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
