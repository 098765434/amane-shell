mod chrome;
mod services;
mod surfaces;
mod tokens;

// the theme lives in the kit, so apps share it
use amane_kit::theme;

use std::process::ExitCode;

use amane::App;

use amane_kit::tooltip;
use surfaces::context_menu::{self, ContextMenu, Icon, MenuItem};
use surfaces::{
    alert, dock, fill, launcher, notifications, osd, quick_settings, status_bar, widgets,
};

/// `amane-shell layer-rules` prints the niri layer rules and exits instead of the shell, for
/// `cargo xtask layer-rules`. The token tools are `cargo xtask tokens …` only.
fn cli(args: &[String]) -> Option<ExitCode> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let ok: Result<(), ()> = match args.as_slice() {
        ["layer-rules"] => {
            print!(
                "{}",
                tokens::layer_rules::render(&tokens::layer_rules::rules())
            );
            Ok(())
        }
        ["tokens", ..] => {
            eprintln!("amane-shell: the token tools are `cargo xtask tokens …` now");
            Err(())
        }
        _ => return None,
    };

    Some(if ok.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Shows a sample alert, side by side or `stacked`, whose answers do
/// nothing: for seeing the alert without ending the session. Debug builds
/// only.
fn alert_test(args: &[String]) -> String {
    let sample = amane_kit::dialog::Dialog::new("amane.alert.test", "Discard changes?")
        .message("Your changes to this document will be lost if you don't save them.")
        .button("Cancel", || eprintln!("alert-test: cancel"));
    let sample = if args.first().map(String::as_str) == Some("stacked") {
        sample
            .destructive("Don't Save", || eprintln!("alert-test: don't save"))
            .main_button("Save", || eprintln!("alert-test: save"))
    } else {
        sample.main_button("Discard", || eprintln!("alert-test: discard"))
    };
    alert::show(sample);
    String::from("ok")
}

/// No core dumps of this process: it holds a password for a moment while filling a field.
fn no_core_dumps() {
    let none = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: setrlimit only reads the struct given
    unsafe {
        libc::setrlimit(libc::RLIMIT_CORE, &none);
    }
}

/// A sample file menu, nested three levels deep, whose items do nothing:
/// for trying the context menu with `arm` or `open`. Debug builds only.
fn sample_menu() -> ContextMenu {
    let share = ContextMenu::new()
        .item(
            MenuItem::label("Nearby Share")
                .icon(Icon::glyph("\u{E2BD}"))
                .on_select(|| eprintln!("airdrop")),
        )
        .item(
            MenuItem::label("Messages")
                .icon(Icon::glyph("\u{F8E3}"))
                .on_select(|| eprintln!("messages")),
        )
        .separator()
        .item(
            MenuItem::label("Mail")
                .icon(Icon::glyph("\u{2709}"))
                .on_select(|| eprintln!("mail")),
        );

    // "Code" carries a third-level submenu to exercise arbitrary nesting.
    let open_with = ContextMenu::new()
        .item(
            MenuItem::label("Code").submenu(
                ContextMenu::new()
                    .item(MenuItem::label("New Window").on_select(|| eprintln!("code: new window")))
                    .item(
                        MenuItem::label("New Workspace").on_select(|| eprintln!("code: workspace")),
                    ),
            ),
        )
        .item(MenuItem::label("Firefox").on_select(|| eprintln!("firefox")))
        .item(MenuItem::label("Preview").on_select(|| eprintln!("preview")));

    ContextMenu::new()
        .item(
            MenuItem::label("Open")
                .icon(Icon::glyph("\u{1F4C2}"))
                .shortcut("\u{2318}O")
                .on_select(|| eprintln!("open")),
        )
        .item(
            MenuItem::label("Open With")
                .icon(Icon::glyph("\u{2026}"))
                .submenu(open_with),
        )
        .separator()
        .item(
            MenuItem::label("Rename")
                .icon(Icon::glyph("\u{270F}"))
                .shortcut("\u{21B5}")
                .on_select(|| eprintln!("rename")),
        )
        .item(
            MenuItem::label("Duplicate")
                .icon(Icon::glyph("\u{2398}"))
                .shortcut("\u{2318}D")
                .disabled(),
        )
        .separator()
        .item(
            MenuItem::label("Share")
                .icon(Icon::glyph("\u{2BAD}"))
                .submenu(share),
        )
        .separator()
        .item(
            MenuItem::label("Move to Trash")
                .icon(Icon::glyph("\u{1F5D1}"))
                .shortcut("\u{2318}\u{232B}")
                .destructive()
                .on_select(|| eprintln!("trash")),
        )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli(&args) {
        return code;
    }

    // no core dump of the shell, which passes passwords on their way into a field; rbw and
    // its agent are started with none too
    no_core_dumps();

    // the shell answers BlueZ's pairing questions, and warns of a device running low
    amane::Bluetooth::serve_agent();
    // and it is the session's polkit agent: the password prompt is its alert
    amane::Polkit::serve_agent();
    // and the bar's own bus service: other programs' items and activities
    services::status_apps::serve();
    services::network::bt_battery::watch();
    // and lowers the displays' refresh rate on battery, when asked to
    services::power::power_refresh::watch();
    // and keeps the screen's brightness in HDR, which niri only holds until a reload
    services::display::hdr_white::watch();
    // and rbw's pinentry: the master password asked in the shell's own dialog
    services::vault::pinentry::serve();

    // a sample menu for `arm` and `open`, in debug builds
    if cfg!(debug_assertions) {
        context_menu::register("file", sample_menu());
    }

    // the bar's hidden icons, from the state file they were kept in before
    status_bar::migrate();

    let mut app = App::new()
        .window(quick_settings::surface)
        .window(notifications::side_sheet)
        .window(launcher::surface)
        .window(alert::surface)
        .window(osd::surface)
        .window(context_menu::anchor)
        .window_per_monitor(widgets::surface)
        .window_per_monitor(widgets::edit_surface)
        .window_per_monitor(dock::surface)
        .window_per_monitor(status_bar::bar);
    for toast in notifications::TOASTS {
        app = app.window(toast);
    }
    for level in context_menu::LEVELS {
        app = app.popup(level);
    }
    app = app.popup(tooltip::popup);
    app = app.popup(status_bar::activity_popup);

    // the seat's input method, for password fill in other apps; Shell Settings turns it off
    if amane_kit::settings::passwords().fill_in_apps {
        app = app.input_method(fill::popup);
    }

    let app = app
        .ipc("arm", |args| {
            // Expand the anchor fullscreen so the next right-click captures a
            // fresh input serial and opens the menu with a working grab.
            // Call: `printf 'arm\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
            let name = args.first().map(String::as_str).unwrap_or("file");
            if context_menu::arm(name) {
                String::from("ok")
            } else {
                format!("error: no menu {name}")
            }
        })
        .ipc("open", |args| {
            let name = args.first().map(String::as_str).unwrap_or("file");
            let x = args
                .get(1)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(240);
            let y = args
                .get(2)
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(180);
            if context_menu::open_named(name, x, y) {
                String::from("ok")
            } else {
                format!("error: no menu {name}")
            }
        })
        // One argument per line, e.g. Do Not Disturb from a keybind:
        // `printf 'notifications\ndnd\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("notifications", notifications::ipc)
        // `printf 'quick-settings\nopen\nwifi\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("quick-settings", quick_settings::ipc)
        // a niri keybind: `printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("launcher", launcher::ipc)
        .ipc("assistant", launcher::assistant_ipc)
        // the media and brightness keys, each acting and showing the new level:
        // `printf 'osd\nvolume\nup\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("osd", osd::ipc)
        // `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("widgets", widgets::ipc)
        // `printf 'status-bar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("status-bar", status_bar::ipc)
        // Shell Settings' Displays page:
        // `printf 'night-light\nschedule\nhours\n21:00\n07:00\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("night-light", services::display::night_light::ipc)
        // its name while it was the taskbar, for scripts and keybinds
        .ipc("taskbar", status_bar::ipc)
        // a niri keybind: `printf 'passwords\nsuggest\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        .ipc("passwords", fill::ipc)
        .ipc("close", |_| {
            context_menu::close();
            String::from("ok")
        });
    // a sample alert for trying it out, in debug builds:
    // `printf 'alert-test\nstacked\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    let app = if cfg!(debug_assertions) {
        app.ipc("alert-test", alert_test)
    } else {
        app
    };
    app.run();

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    /// The verbs `main` registers, from its `.ipc("…"` calls.
    fn registered() -> BTreeSet<String> {
        include_str!("main.rs")
            .split(".ipc(\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            // a verb's shape, so this module's own mentions don't count
            .filter(|verb| {
                !verb.is_empty() && verb.chars().all(|c| c.is_ascii_lowercase() || c == '-')
            })
            .map(String::from)
            .collect()
    }

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
        let (code, doc) = (registered(), documented());
        assert!(code.len() > 10, "found too few .ipc calls: {code:?}");
        let undocumented: Vec<_> = code.difference(&doc).collect();
        let missing: Vec<_> = doc.difference(&code).collect();
        assert!(
            undocumented.is_empty() && missing.is_empty(),
            "not in docs/integration/ipc.md: {undocumented:?}; documented but not registered: {missing:?}"
        );
    }
}
