//! The shell's side of the phone: the commands a paired phone's remote may
//! run on this computer, handed to amane-phone at start.

use amane_phone::{KdeConnect, RunCommand};

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

/// Hands the remote's commands to amane-phone; once, at start.
pub fn start() {
    KdeConnect::set_run_command_list(&RUN_COMMANDS);
}
