//! The phone's apps for the launcher: `amane-mirror --list-apps --serial
//! <serial> --device <kdeconnect id>` asks scrcpy's server for them and
//! writes `~/.cache/amane-shell/phone-apps.json`, which the shell's
//! Phone Apps search reads. The shell runs it when adb has the phone and
//! the list is missing or a day old.

use amane_kit::args::flag;
use std::path::PathBuf;

use amane::serde::{Deserialize, Serialize};

use crate::server;

#[derive(Serialize, Deserialize)]
#[serde(crate = "amane::serde")]
pub struct App {
    pub name: String,
    pub package: String,
    pub system: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "amane::serde")]
pub struct List {
    /// KDE Connect's id for the phone.
    pub device: String,
    /// When it was listed, in seconds since 1970.
    pub updated: u64,
    pub apps: Vec<App>,
}

pub fn file() -> Option<PathBuf> {
    let cache = amane_kit::paths::cache();
    Some(
        cache
            .join(amane_kit::app_ids::SHELL)
            .join("phone-apps.json"),
    )
}

pub fn write(args: &[String]) -> Result<(), String> {
    let serial = flag(args, "--serial").ok_or("--list-apps needs --serial")?;
    let device = flag(args, "--device").ok_or("--list-apps needs --device")?;

    let apps = server::list_apps(&serial)?
        .into_iter()
        .map(|app| App {
            name: app.name,
            package: app.package,
            system: app.system,
        })
        .collect();

    let list = List {
        device,
        updated: amane_kit::time::unix_s(),
        apps,
    };

    let path = file().ok_or("no cache directory")?;
    // whole, so the shell never reads half of it
    amane_kit::fs::save_json(&path, &list).map_err(|error| error.to_string())
}
