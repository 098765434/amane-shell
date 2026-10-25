//! A window's menu, opened by right-clicking its icon in the dock.

use amane::Windows;

use crate::components::context_menu::{self, ContextMenu, MenuItem};
use crate::components::workspace_menu;

/// Registry key for the dynamically-rebuilt per-window menu.
const WINDOW_MENU_KEY: &str = "amane.dock.window-menu";

/// Build a per-window menu bound to `id` and open it at `at` on the surface
/// `parent`. A bottom-anchored parent makes it open upward.
pub(crate) fn open_window_menu(id: u64, floating: bool, parent: &'static str, at: (i32, i32)) {
    let move_to = workspace_menu::all(move |target| Windows::move_to_workspace(id, target));

    let float_label = if floating {
        "Move to Tiling"
    } else {
        "Move to Floating"
    };

    let menu = ContextMenu::new()
        .item(MenuItem::label("Focus").on_select(move || Windows::focus(id)))
        .item(MenuItem::label("Toggle Fullscreen").on_select(move || Windows::fullscreen(id)))
        .item(MenuItem::label(float_label).on_select(move || Windows::toggle_floating(id)))
        .item(MenuItem::label("Move to Workspace").submenu(move_to))
        .separator()
        .item(
            MenuItem::label("Close")
                .destructive()
                .on_select(move || Windows::close(id)),
        );

    context_menu::show_under(WINDOW_MENU_KEY, menu, parent, at);
}
