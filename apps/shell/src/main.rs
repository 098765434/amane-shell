mod chrome;
mod ipc;
mod services;
mod surfaces;
mod tokens;

// the theme lives in the kit, so apps share it
use amane_kit::theme;

use std::process::ExitCode;

use amane::App;

use amane_kit::menu::layer as context_menu;
use amane_kit::menu::{ContextMenu, Icon, MenuItem};
use amane_kit::tooltip;
use surfaces::{
    alert, dock, fill, launcher, notifications, osd, quick_settings, status_bar, widgets,
};

/// `amane-shell layer-rules` prints the niri layer rules and exits instead of the shell, for
/// `cargo xtask layer-rules`: the rules name the shell's own surfaces, so the shell builds them.
fn cli(args: &[String]) -> Option<ExitCode> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if args != ["layer-rules"] {
        return None;
    }
    print!(
        "{}",
        tokens::layer_rules::render(&tokens::layer_rules::rules())
    );
    Some(ExitCode::SUCCESS)
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
                .on_select(|| log::info!("airdrop")),
        )
        .item(
            MenuItem::label("Messages")
                .icon(Icon::glyph("\u{F8E3}"))
                .on_select(|| log::info!("messages")),
        )
        .separator()
        .item(
            MenuItem::label("Mail")
                .icon(Icon::glyph("\u{2709}"))
                .on_select(|| log::info!("mail")),
        );

    // "Code" carries a third-level submenu to exercise arbitrary nesting.
    let open_with = ContextMenu::new()
        .item(
            MenuItem::label("Code").submenu(
                ContextMenu::new()
                    .item(
                        MenuItem::label("New Window").on_select(|| log::info!("code: new window")),
                    )
                    .item(
                        MenuItem::label("New Workspace")
                            .on_select(|| log::info!("code: workspace")),
                    ),
            ),
        )
        .item(MenuItem::label("Firefox").on_select(|| log::info!("firefox")))
        .item(MenuItem::label("Preview").on_select(|| log::info!("preview")));

    ContextMenu::new()
        .item(
            MenuItem::label("Open")
                .icon(Icon::glyph("\u{1F4C2}"))
                .shortcut("\u{2318}O")
                .on_select(|| log::info!("open")),
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
                .on_select(|| log::info!("rename")),
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
                .on_select(|| log::info!("trash")),
        )
}

fn main() -> ExitCode {
    amane_log::init(amane_kit::app_ids::SHELL);
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
    // the commands a paired phone's remote may run here
    services::phone::start();
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

    // what the panels do as they open and close; a menu belongs to the surface it was
    // opened from, so it closes as the panel changes
    chrome::per_output::follow_outputs();
    quick_settings::register_panel();
    notifications::register_panel();
    chrome::panel::on_switch(context_menu::close);

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

    ipc::register(app).run();

    ExitCode::SUCCESS
}
