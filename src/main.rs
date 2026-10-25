mod components;
mod services;
mod theme;
mod tokens;

use std::process::ExitCode;

use amane::App;

use components::context_menu::{self, ContextMenu, Icon, MenuItem};
use components::{dock, launcher, notifications, quick_settings, status_bar, tooltip};

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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli(&args) {
        return code;
    }

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

    let mut app = App::new()
        .window(status_bar::bar)
        .window(quick_settings::surface)
        .window(notifications::side_sheet)
        .window(launcher::surface)
        .window(context_menu::anchor)
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
    // `printf 'status-bar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`
    .ipc("status-bar", status_bar::ipc)
    // its name while it was the taskbar, for scripts and keybinds
    .ipc("taskbar", status_bar::ipc)
    .ipc("close", |_| {
        context_menu::close();
        String::from("ok")
    })
    .run();

    ExitCode::SUCCESS
}
