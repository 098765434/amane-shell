//! The shell's side of the phone, over emaki-phone (KDE Connect and adb):
//! what its surfaces show and do with it, kept here so they only draw.
//!
//! - the commands a paired phone's remote may run here ([`start`]);
//! - [`screen`]: whether a phone's screen can be shown, and opening it;
//! - [`webcam`]: the phone's camera as this computer's webcam;
//! - [`apps`]: the phone's apps, for the launcher;
//! - [`notifications`]: the buttons a phone's notification gets;
//! - [`pairing`]: a phone asking to pair.

pub mod apps;
pub mod notifications;
pub mod pairing;
pub mod screen;
pub mod webcam;

use emaki_phone::{KdeConnect, RunCommand};

use crate::services::installed;

/// The app the phone's screen, its apps and its camera open in.
pub const MIRROR_APP: &str = emaki_kit::app_ids::MIRROR;

/// The app the phone's messages and remote input open in.
pub const MESSAGES_APP: &str = emaki_kit::app_ids::MESSAGES;

/// What the phone's "Run command" lists while its remote is on. The ids
/// are the ones the old Quickshell version wrote, so a phone that already
/// has them keeps them, and turning the remote off leaves the user's own.
const RUN_COMMANDS: [RunCommand; 4] = [
    RunCommand {
        id: "{c1a0b5e2-0000-4a00-8000-caelestia0001}",
        name: "Lock Screen",
        command: "loginctl lock-session",
    },
    RunCommand {
        id: "{c1a0b5e2-0000-4a00-8000-caelestia0002}",
        name: "Suspend",
        command: "systemctl suspend",
    },
    RunCommand {
        id: "{c1a0b5e2-0000-4a00-8000-caelestia0003}",
        name: "Lock & Suspend",
        command: "loginctl lock-session && systemctl suspend",
    },
    RunCommand {
        id: "{c1a0b5e2-0000-4a00-8000-caelestia0004}",
        name: "Screenshot",
        command: "niri msg action screenshot-screen",
    },
];

/// Hands the remote's commands to emaki-phone; once, at start.
pub fn start() {
    KdeConnect::set_run_command_list(&RUN_COMMANDS);
}

/// Whether KDE Connect is installed: its daemon is on `PATH` while it's on
/// in Emaki Settings → Services.
pub fn available() -> bool {
    installed("kdeconnectd")
}

/// Opens `emaki-messages` on `device`, at `view` ("messages" or "input").
pub fn open_messages(device: &str, view: &str) {
    amane::Launch::argv(&[MESSAGES_APP, "--device", device, "--view", view]).activated();
}
