//! The phone's apps for the launcher: `emaki-mirror --list-apps --serial
//! <serial> --device <kdeconnect id>` asks scrcpy's server for them and
//! writes `~/.cache/emaki/phone-apps.json`, which the shell's
//! Phone Apps search reads. The shell runs it when adb has the phone and
//! the list is missing or a day old.

use emaki_phone::shared::apps::{self, App, List};

use crate::server;
use emaki_kit::args::flag;

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
        updated: emaki_kit::time::unix_s(),
        apps,
    };

    // whole, so the shell never reads half of it
    apps::write(&list).map_err(|error| error.to_string())
}
