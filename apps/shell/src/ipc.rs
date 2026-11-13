//! The shell's IPC: the one table of the verbs it answers on its socket
//! (`$XDG_RUNTIME_DIR/emaki.sock`). Each verb's lines are read into the kit's
//! [`ShellCommand`], which other programs send too, and handed to the surface
//! or service it's for. See `docs/developer/integration/ipc.md`.

use amane::App;
use emaki_kit::menu::layer as context_menu;
use emaki_kit::shell_ipc::ShellCommand;

use crate::services::display::night_light;
use crate::surfaces::{
    alert, fill, launcher, notifications, osd, quick_settings, status_bar, widgets,
};

/// What answers a verb's lines.
type Handler = fn(&[String]) -> String;

/// `(verb, handler)` for each verb, the handler reading the verb's lines.
macro_rules! handlers {
    ($($verb:literal),+ $(,)?) => {
        [$(($verb, (|args| answer($verb, args)) as Handler)),+]
    };
}

/// Every verb and its handler, which reads the lines and runs them.
const HANDLERS: [(&str, Handler); 14] = handlers![
    // the next right-click opens a menu with a working grab:
    // `printf 'arm\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "arm",
    "open",
    "close",
    // Do Not Disturb from a keybind:
    // `printf 'notifications\ndnd\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "notifications",
    // `printf 'quick-settings\nopen\nwifi\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "quick-settings",
    // a niri keybind: `printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "launcher",
    "assistant",
    // the media and brightness keys, each acting and showing the new level:
    // `printf 'osd\nvolume\nup\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "osd",
    // `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "widgets",
    // `printf 'status-bar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "status-bar",
    // Emaki Settings' Displays page:
    // `printf 'night-light\nschedule\nhours\n21:00\n07:00\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "night-light",
    // a niri keybind: `printf 'passwords\nsuggest\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "passwords",
    // the idle service turning the screen off, and on at the first input:
    // `printf 'screen\noff\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock`
    "screen",
    // a sample alert, in debug builds: `printf 'alert-test\nstacked\n' | nc -U -N …`
    "alert-test",
];

/// Answers every verb on `app`'s socket; `alert-test` only in debug builds.
pub fn register(mut app: App) -> App {
    for (verb, handler) in HANDLERS {
        if verb == "alert-test" && !cfg!(debug_assertions) {
            continue;
        }
        app = app.ipc(verb, handler);
    }
    app
}

fn answer(verb: &str, args: &[String]) -> String {
    match ShellCommand::parse(verb, args) {
        Ok(command) => run(command),
        Err(usage) => usage,
    }
}

/// Does `command`, and says how it went.
pub fn run(command: ShellCommand) -> String {
    match command {
        ShellCommand::Arm(name) => {
            if context_menu::arm(&name) {
                String::from("ok")
            } else {
                format!("error: no menu {name}")
            }
        }
        ShellCommand::OpenMenu { name, x, y } => {
            if context_menu::open_named(&name, x, y) {
                String::from("ok")
            } else {
                format!("error: no menu {name}")
            }
        }
        ShellCommand::CloseMenu => {
            context_menu::close();
            String::from("ok")
        }
        ShellCommand::Notifications(command) => notifications::run(command),
        ShellCommand::QuickSettings(command) => quick_settings::run(command),
        ShellCommand::Launcher(command) => launcher::run(command),
        ShellCommand::Assistant(command) => launcher::run_assistant(command),
        ShellCommand::Osd(command) => osd::run(command),
        ShellCommand::Widgets(command) => widgets::run(command),
        ShellCommand::StatusBar(command) => status_bar::run(command),
        ShellCommand::NightLight(command) => night_light::run(command),
        ShellCommand::Passwords(command) => fill::run(command),
        ShellCommand::Screen { off } => screen(off),
        ShellCommand::AlertTest { stacked } => alert_test(stacked),
    }
}

/// The monitors off, and the windows with them: nothing draws, and the
/// services only the windows read stop polling (`amane::set_screen_off`),
/// until this turns them on again or, by itself, the first input does. A
/// failure to reach niri leaves the windows as they were.
fn screen(off: bool) -> String {
    let done = if off {
        amane::niri::power_off_monitors()
    } else {
        amane::niri::power_on_monitors()
    };
    match done {
        Ok(()) => {
            amane::set_screen_off(off);
            String::from("ok")
        }
        Err(err) => format!("error: {err}"),
    }
}

/// Shows a sample alert, side by side or `stacked`, whose answers do
/// nothing: for seeing the alert without ending the session. Debug builds
/// only.
fn alert_test(stacked: bool) -> String {
    use emaki_kit::dialog::Dialog;
    let sample = Dialog::new("emaki.alert.test", "Discard changes?")
        .message("Your changes to this document will be lost if you don't save them.")
        .button("Cancel", || log::info!("alert-test: cancel"));
    let sample = if stacked {
        sample
            .destructive("Don't Save", || log::info!("alert-test: don't save"))
            .main_button("Save", || log::info!("alert-test: save"))
    } else {
        sample.main_button("Discard", || log::info!("alert-test: discard"))
    };
    alert::show(sample);
    String::from("ok")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The verbs `docs/developer/integration/ipc.md` documents: its `` ### `verb` ``
    /// headings under Commands, and the calls in the Context menus table.
    fn documented() -> BTreeSet<String> {
        let doc = include_str!("../../../docs/developer/integration/ipc.md");
        let commands = doc
            .split_once("\n## Commands\n")
            .map(|(_, rest)| rest.split("\n## ").next().unwrap_or(rest))
            .expect("ipc.md has a Commands section");
        let mut verbs = BTreeSet::new();
        let mut in_menus = false;
        for line in commands.lines() {
            if let Some(heading) = line.strip_prefix("### ") {
                in_menus = heading == "Context menus";
                if let Some(verb) = heading.strip_prefix('`').and_then(|h| h.strip_suffix('`')) {
                    verbs.insert(verb.to_string());
                }
            } else if in_menus
                && let Some(cell) = line.strip_prefix("| `")
                && let Some(verb) = cell.split([' ', '`']).next()
            {
                verbs.insert(verb.to_string());
            }
        }
        verbs
    }

    #[test]
    fn every_ipc_verb_is_documented_and_every_documented_verb_exists() {
        let code: BTreeSet<String> = HANDLERS.iter().map(|(v, _)| v.to_string()).collect();
        let doc = documented();
        let undocumented: Vec<_> = code.difference(&doc).collect();
        let missing: Vec<_> = doc.difference(&code).collect();
        assert!(
            undocumented.is_empty() && missing.is_empty(),
            "not in docs/developer/integration/ipc.md: {undocumented:?}; documented but not registered: {missing:?}"
        );
    }

    #[test]
    fn the_table_answers_every_verb_the_command_has() {
        let table: Vec<&str> = HANDLERS.iter().map(|(v, _)| *v).collect();
        assert_eq!(table, emaki_kit::shell_ipc::VERBS);
    }
}
