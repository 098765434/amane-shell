//! The launcher's and the assistant's IPC (`launcher …`, `assistant …`).

use super::*;

pub fn run(command: shell_ipc::Launcher) -> String {
    const USAGE: &str =
        "usage: launcher [toggle | open | close] [apps | files | actions | clipboard | passwords]";
    let mode = match command.mode.as_deref().map(Mode::named) {
        None => None,
        Some(Some(mode)) => Some(mode),
        Some(None) => return String::from(USAGE),
    };
    let open = is_open(Panel::Launcher);
    match (command.action, mode) {
        (Action::Toggle, None) => panel::toggle(Panel::Launcher),
        // the mode showing closes; another switches to it
        (Action::Toggle, Some(mode)) if open && with(|state| state.mode) == mode => close(),
        (Action::Toggle | Action::Open, Some(mode)) => {
            switch_mode(mode);
            if !open {
                panel::toggle(Panel::Launcher);
            }
        }
        (Action::Open, None) => {
            if !open {
                panel::toggle(Panel::Launcher);
            }
        }
        (Action::Close, _) => {
            if open {
                close();
            }
        }
    }
    Launcher::changed();
    String::from("ok")
}

/// `emaki` IPC: `assistant listen [start | stop | toggle | cancel]`
/// (niri binds a key to it: held, or pressed to start and again to stop)
/// and `assistant status`.
pub fn run_assistant(command: shell_ipc::Assistant) -> String {
    use shell_ipc::{Assistant, Listen};
    match command {
        Assistant::Listen(Listen::Start) => listen(),
        Assistant::Listen(Listen::Stop) => assistant_service::listen_stop(),
        Assistant::Listen(Listen::Toggle) if assistant_service::listening().on => {
            assistant_service::listen_stop();
        }
        Assistant::Listen(Listen::Toggle) => listen(),
        Assistant::Listen(Listen::Cancel) => assistant_service::listen_cancel(),
        Assistant::Status => {
            return assistant_service::status().unwrap_or_else(|| String::from("not connected"));
        }
    }
    Launcher::changed();
    String::from("ok")
}
