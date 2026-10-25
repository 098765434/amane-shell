//! The menus of the dock's icons, opened by right-clicking one: a window's
//! (focus, fullscreen, float, move to workspace, pin, close) and a pinned
//! app's (open, open on another workspace, unpin).

use amane::Windows;

use super::model::{self, Space, Win};
use super::pins::{self, Pin};
use crate::components::app_icon;
use crate::components::context_menu::{self, ContextMenu, MenuItem};
use crate::components::workspace_menu;

/// Registry key for the dynamically-rebuilt per-window menu.
const WINDOW_MENU_KEY: &str = "amane.dock.window-menu";
/// Registry key for a pinned app's menu.
const PIN_MENU_KEY: &str = "amane.dock.pin-menu";

/// Build `window`'s menu and open it at `at` on the surface `parent`. A
/// bottom-anchored parent makes it open upward. `spaces` are niri's
/// workspaces, for pinning it to the one it's on.
pub(crate) fn open_window_menu(
    window: &Win,
    spaces: &[Space],
    parent: &'static str,
    at: (i32, i32),
) {
    let id = window.id;
    let move_to = workspace_menu::all(move |target| Windows::move_to_workspace(id, target));

    let float_label = if window.floating {
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
        .item(pin_item(window, spaces))
        .separator()
        .item(
            MenuItem::label("Close")
                .destructive()
                .on_select(move || Windows::close(id)),
        );

    context_menu::show_under(WINDOW_MENU_KEY, menu, parent, at);
}

/// "Pin to Dock" for the workspace `window` is on, or "Unpin from Dock"
/// when its app is pinned there already; greyed out for a window whose
/// app has no desktop entry to open it again by.
fn pin_item(window: &Win, spaces: &[Space]) -> MenuItem {
    let here = spaces
        .iter()
        .find(|space| Some(space.id) == window.workspace);
    let pinned_here = here.and_then(|here| {
        pins::all().into_iter().find(|pin| {
            pin.app_id == window.app_id
                && model::resolve(pin, spaces).is_some_and(|at| at.id == here.id)
        })
    });
    if let Some(pin) = pinned_here {
        return MenuItem::label("Unpin from Dock").on_select(move || pins::remove(&pin));
    }
    let item = MenuItem::label("Pin to Dock");
    match here {
        Some(here) if app_icon::entry_for(&window.app_id).is_some() => {
            let pin = Pin::new(
                &window.app_id,
                &here.output,
                here.name.as_deref(),
                here.index,
            );
            item.on_select(move || pins::add(pin.clone()))
        }
        _ => item.disabled(),
    }
}

/// Build the menu of `pin`, whose workspace is `workspace`, and open it at
/// `at` on the surface `parent`.
pub(crate) fn open_pin_menu(pin: &Pin, workspace: i64, parent: &'static str, at: (i32, i32)) {
    let app_id = pin.app_id.clone();
    let open_on = workspace_menu::all(move |target| {
        pins::open(&app_id, target);
    });
    let app_id = pin.app_id.clone();
    let unpinned = pin.clone();

    let menu = ContextMenu::new()
        .item(MenuItem::label("Open").on_select(move || {
            pins::open(&app_id, workspace);
        }))
        .item(MenuItem::label("Open in Workspace").submenu(open_on))
        .separator()
        .item(MenuItem::label("Unpin from Dock").on_select(move || pins::remove(&unpinned)));

    context_menu::show_under(PIN_MENU_KEY, menu, parent, at);
}
