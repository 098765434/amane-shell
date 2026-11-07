//! The phone's apps for the launcher: `amane-mirror --list-apps --serial
//! <serial> --device <kdeconnect id>` asks scrcpy's server for them and
//! writes `~/.cache/amane-shell/phone-apps.json`, which the shell's
//! Phone Apps search reads. The shell runs it when adb has the phone and
//! the list is missing or a day old.

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
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(cache.join("amane-shell").join("phone-apps.json"))
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let at = args.iter().position(|arg| arg == name)?;
    args.get(at + 1).cloned()
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
        updated: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_secs()),
        apps,
    };

    let path = file().ok_or("no cache directory")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string(&list).map_err(|error| error.to_string())?;

    // written whole, then moved over, so the shell never reads half of it
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, text).map_err(|error| error.to_string())?;
    std::fs::rename(&partial, &path).map_err(|error| error.to_string())
}
