mod components;
mod services;
mod tokens;

// the theme lives in the kit, so apps share it
use amane_kit::theme;

use std::process::ExitCode;

use amane::App;

use components::context_menu::{self, ContextMenu, Icon, MenuItem};
use components::{
    alert, dock, launcher, notifications, quick_settings, status_bar, tooltip, widgets,
};

/// `amane-shell tokens …` runs a token tool and exits instead of the shell.
fn cli(args: &[String]) -> Option<ExitCode> {
    let read = |path: &str| {
        std::fs::read_to_string(path).map_err(|e| eprintln!("amane-shell: {path}: {e}"))
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let ok = match args.as_slice() {
        ["tokens", "normalize", path] => read(path).and_then(|json| {
            tokens::check::normalize(&json)
                .map(|out| print!("{out}"))
                .map_err(|e| eprintln!("amane-shell: {path}: {e}"))
        }),
        ["tokens", "check", path] => read(path).and_then(|json| {
            if tokens::check::check(&json) {
                Ok(())
            } else {
                Err(())
            }
        }),
        ["tokens", ..] => {
            eprintln!("usage: amane-shell tokens normalize <file> | tokens check <file>");
            Err(())
        }
        ["layer-rules"] => {
            print!(
                "{}",
                tokens::layer_rules::render(&tokens::layer_rules::rules())
            );
            Ok(())
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
    if !cfg!(debug_assertions) {
        return String::from("only in debug builds");
    }
    let sample = components::dialog::Dialog::new("amane.alert.test", "Discard changes?")
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli(&args) {
        return code;
    }

    // the shell answers BlueZ's pairing questions, and warns of a device running low
    amane::Bluetooth::serve_agent();
    // and it is the session's polkit agent: the password prompt is its alert
    amane::Polkit::serve_agent();
    services::bt_battery::watch();

    let share = ContextMenu::new()
        .item(
            MenuItem::label("AirDrop")
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

    let menu = ContextMenu::new()
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
        );

    context_menu::register("file", menu);

    // the bar's hidden icons, from the state file they were kept in before
    status_bar::migrate();

    let mut app = App::new()
        .window(status_bar::bar)
        .window(quick_settings::surface)
        .window(notifications::side_sheet)
        .window(launcher::surface)
        .window(alert::surface)
        .window(context_menu::anchor)
        .window_per_monitor(widgets::surface)
        .window_per_monitor(widgets::edit_surface)
        .window_per_monitor(dock::surface);
    for toast in notifications::TOASTS {
        app = app.window(toast);
    }
    for level in context_menu::LEVELS {
        app = app.popup(level);
    }
    app = app.popup(tooltip::popup);

    app.ipc("arm", |args| {
        // Expand the anchor fullscreen so the next right-click captures a
        // fresh input serial and opens the menu with a working grab.
        // Call: `printf 'arm\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
        let name = args.first().map(String::as_str).unwrap_or("file");
        context_menu::arm(name);
        String::from("ok")
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
        context_menu::open_named(name, x, y);
        String::from("ok")
    })
    // One argument per line, e.g. Do Not Disturb from a keybind:
    // `printf 'notifications\ndnd\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("notifications", notifications::ipc)
    // `printf 'quick-settings\nopen\nwifi\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("quick-settings", quick_settings::ipc)
    // a niri keybind: `printf 'launcher\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("launcher", launcher::ipc)
    // `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("widgets", widgets::ipc)
    // `printf 'status-bar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("status-bar", status_bar::ipc)
    // its name while it was the taskbar, for scripts and keybinds
    .ipc("taskbar", status_bar::ipc)
    // a sample alert for trying it out, in debug builds:
    // `printf 'alert-test\nstacked\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("alert-test", alert_test)
    .ipc("close", |_| {
        context_menu::close();
        String::from("ok")
    })
    .run();

    ExitCode::SUCCESS
}
