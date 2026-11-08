//! Do Not Disturb's menu of timed choices, from the bar's icon and the
//! center's header. Do Not Disturb itself is a service
//! (`services::do_not_disturb`).

use amane::Service;
use chrono::Local;

use crate::services::do_not_disturb::{DoNotDisturb, Until, choices};
use emaki_kit::menu::layer as context_menu;
use emaki_kit::menu::{ContextMenu, MenuItem};

const MENU_KEY: &str = "emaki.notifications.dnd";

/// The menu of timed choices, hung under `parent_ns` at `point`.
pub fn open(parent_ns: &'static str, point: (i32, i32)) {
    let on = DoNotDisturb::read().on();

    let mut menu = ContextMenu::new();
    for (label, end) in choices(Local::now().naive_local()) {
        menu = menu.item(MenuItem::label(label).on_select(move || DoNotDisturb::set(Some(end))));
    }
    menu = menu.separator();
    menu = if on {
        menu.item(MenuItem::label("Turn Off").on_select(|| DoNotDisturb::set(None)))
    } else {
        menu.item(MenuItem::label("Turn On").on_select(|| DoNotDisturb::set(Some(Until::Forever))))
    };

    context_menu::show_under(MENU_KEY, menu, parent_ns, point);
}
