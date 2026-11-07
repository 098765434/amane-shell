//! The phone's screen (`amane-mirror`) from Messages: whether it can be
//! shown, and the phone's own messaging app to show in a window.

use std::path::PathBuf;

use amane::{Adb, PhoneDevice, Service};

/// The phone's messaging apps, the one Samsung ships first.
const MESSAGING: [&str; 2] = [
    "com.samsung.android.messaging",
    "com.google.android.apps.messaging",
];

fn config_dir() -> Option<PathBuf> {
    Some(amane_kit::paths::config())
}

fn cache_dir() -> Option<PathBuf> {
    Some(amane_kit::paths::cache())
}

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

    config_dir()
        .and_then(|dir| std::fs::read_to_string(dir.join("amane-shell/phone-screen.json")).ok())
        .is_some_and(|text| text.contains(&format!("\"{}\"", device.id())))
}

/// The phone's messaging app, its package and name, from the list of its
/// apps `amane-mirror --list-apps` keeps.
pub fn messaging_app(device: &PhoneDevice) -> Option<(String, String)> {
    let text = std::fs::read_to_string(cache_dir()?.join("amane-shell/phone-apps.json")).ok()?;
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
    let quote = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
    amane::spawn_activated(&format!(
        "amane-mirror --device {} --app {} --name {}",
        quote(device.id()),
        quote(package),
        quote(name),
    ));
}
