//! The shell's IPC: the one table of the verbs it answers on its socket
//! (`$XDG_RUNTIME_DIR/amane.sock`). Each verb's lines are read into the kit's
//! [`ShellCommand`], which other programs send too, and handed to the surface
//! or service it's for. See `docs/integration/ipc.md`.

use amane::App;
use amane_kit::menu::layer as context_menu;
use amane_kit::shell_ipc::ShellCommand;

use crate::services::display::night_light;
use crate::surfaces::{
    alert, fill, launcher, notifications, osd, quick_settings, status_bar, widgets,
};

/// What answers a verb's lines.
type Handler = fn(&[String]) -> String;

/// Every verb and its handler, which reads the lines and runs them.
const HANDLERS: [(&str, Handler); 13] = [
    // the next right-click opens a menu with a working grab:
    // `printf 'arm\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("arm", |args| answer("arm", args)),
    ("open", |args| answer("open", args)),
    ("close", |args| answer("close", args)),
    // Do Not Disturb from a keybind:
    // `printf 'notifications\ndnd\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("notifications", |args| answer("notifications", args)),
    // `printf 'quick-settings\nopen\nwifi\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("quick-settings", |args| answer("quick-settings", args)),
    // a niri keybind: `printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("launcher", |args| answer("launcher", args)),
    ("assistant", |args| answer("assistant", args)),
    // the media and brightness keys, each acting and showing the new level:
    // `printf 'osd\nvolume\nup\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("osd", |args| answer("osd", args)),
    // `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("widgets", |args| answer("widgets", args)),
    // `printf 'status-bar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("status-bar", |args| answer("status-bar", args)),
    // Shell Settings' Displays page:
    // `printf 'night-light\nschedule\nhours\n21:00\n07:00\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("night-light", |args| answer("night-light", args)),
    // a niri keybind: `printf 'passwords\nsuggest\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    ("passwords", |args| answer("passwords", args)),
    // a sample alert, in debug builds: `printf 'alert-test\nstacked\n' | nc -U -N …`
    ("alert-test", |args| answer("alert-test", args)),
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
        ShellCommand::AlertTest { stacked } => alert_test(stacked),
    }
}

/// Shows a sample alert, side by side or `stacked`, whose answers do
/// nothing: for seeing the alert without ending the session. Debug builds
/// only.
fn alert_test(stacked: bool) -> String {
    use amane_kit::dialog::Dialog;
    let sample = Dialog::new("amane.alert.test", "Discard changes?")
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

    /// The verbs `docs/integration/ipc.md` documents: its `` ### `verb` ``
    /// headings under Commands, and the calls in the Context menus table.
    fn documented() -> BTreeSet<String> {
        let doc = include_str!("../../../docs/integration/ipc.md");
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
            "not in docs/integration/ipc.md: {undocumented:?}; documented but not registered: {missing:?}"
        );
    }

    #[test]
    fn the_table_answers_every_verb_the_command_has() {
        let table: Vec<&str> = HANDLERS.iter().map(|(v, _)| *v).collect();
        assert_eq!(table, amane_kit::shell_ipc::VERBS);
    }
}
