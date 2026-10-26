//! An app's menu, opened by right-clicking it in the grid or the results:
//! open it, open it on a workspace of the monitor in focus (a new one
//! too), or run one of the actions its desktop entry lists, like "New
//! Private Window". A file's menu opens it, opens it with another app, or
//! shows it in the file manager.

use std::path::Path;

use amane::{Apps, DesktopApp, Service, Windows, Workspaces};

use crate::components::context_menu::{self, ContextMenu, Icon, MenuItem};
use crate::components::icons::{ICON_ADD, ICON_OPEN_IN_NEW};
use crate::components::layout::cursor;
use crate::components::panel::close;
use crate::components::workspace_menu;
use crate::services::file_search;
use crate::tokens::sys;

use super::LAUNCHER_NS;
use super::entries::{self, Entry};

/// Registry key for the menu, built afresh for each app.
const APP_MENU_KEY: &str = "amane.launcher.app-menu";

/// Opens `app`'s menu at the pointer.
pub(super) fn open(app: &DesktopApp) {
    let launched = app.clone();
    let exec = app.exec().to_string();
    let on_workspace = workspace_menu::on_focused_output(move |workspace| {
        close();
        // both go through amane's one worker thread, in order: niri opens
        // the window on the workspace that has just been focused
        Workspaces::focus(workspace);
        Windows::spawn(vec![String::from("sh"), String::from("-c"), exec.clone()]);
    });

    let mut menu = ContextMenu::new()
        .item(
            MenuItem::label("Open")
                .icon(glyph(ICON_OPEN_IN_NEW))
                .on_select(move || entries::run(&Entry::app(&launched), "")),
        )
        .item(
            MenuItem::label("Open in Workspace")
                .icon(glyph(ICON_ADD))
                .submenu(on_workspace),
        );

    if !app.actions().is_empty() {
        menu = menu.separator();
        for action in app.actions() {
            let action = action.clone();
            menu = menu.item(MenuItem::label(action.name()).on_select(move || {
                close();
                action.launch();
            }));
        }
    }

    context_menu::show_under(APP_MENU_KEY, menu, LAUNCHER_NS, cursor());
}

/// A menu row's icon: one of the shell's glyphs, in their font.
fn glyph(glyph: char) -> Icon {
    Icon::glyph_with_font(glyph.to_string(), sys::r#type::icon())
}

/// Registry key for a file's menu, built afresh for each file.
const FILE_MENU_KEY: &str = "amane.launcher.file-menu";

/// Opens a file's menu at the pointer: open it, open it with one of the
/// apps that open its kind, or show it in the file manager.
pub(super) fn open_file(path: &Path) {
    let opened = path.to_path_buf();
    let revealed = path.to_path_buf();
    let openers: Vec<DesktopApp> = Apps::read()
        .list()
        .iter()
        .filter(|app| app.opens(path))
        .cloned()
        .collect();
    let mut menu = ContextMenu::new().item(
        MenuItem::label("Open")
            .icon(glyph(ICON_OPEN_IN_NEW))
            .on_select(move || {
                close();
                file_search::open(&opened);
            }),
    );
    if !openers.is_empty() {
        let mut with = ContextMenu::new();
        for app in openers {
            let file = path.to_path_buf();
            with = with.item(MenuItem::label(app.name()).on_select(move || {
                close();
                app.launch_with(std::slice::from_ref(&file));
            }));
        }
        menu = menu.item(MenuItem::label("Open With").submenu(with));
    }
    menu = menu
        .separator()
        .item(MenuItem::label("Show in Files").on_select(move || {
            close();
            file_search::reveal(&revealed);
        }));
    context_menu::show_under(FILE_MENU_KEY, menu, LAUNCHER_NS, cursor());
}
