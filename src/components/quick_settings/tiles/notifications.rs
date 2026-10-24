//! Do Not Disturb — GNOME's toggle, and a page with the notification
//! center's timed choices: for an hour, until this evening, until tomorrow.

use amane::{Service, Widget};
use chrono::Local;

use super::GridCell;
use crate::components::icons::ICON_DND;
use crate::components::layout::boxed;
use crate::components::list::{List, ListItem};
use crate::components::notifications::DoNotDisturb;
use crate::components::notifications::card::local;
use crate::components::notifications::dnd::{self, Until};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::quick_settings::{Page, open};
use crate::tokens::sys;

/// "On", "Until 19:00" or "Off".
fn status(until: Option<Until>) -> String {
    match until {
        None => String::from("Off"),
        Some(Until::Forever) => String::from("On"),
        Some(Until::At(end)) => format!("Until {}", local(end, "%H:%M")),
    }
}

pub(crate) fn cell() -> Option<GridCell> {
    let until = DoNotDisturb::read().until();

    let tile = QuickToggle::new("amane.qs.dnd", ICON_DND, "Do Not Disturb")
        .subtitle(status(until))
        .checked(until.is_some())
        .on_toggle(|on| DoNotDisturb::set(on.then_some(Until::Forever)))
        .on_open(|| open(Page::DoNotDisturb))
        .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn page() -> DetailPage {
    let until = DoNotDisturb::read().until();

    let items: Vec<ListItem> = dnd::choices(Local::now().naive_local())
        .into_iter()
        .map(|(label, end)| {
            ListItem::new(format!("amane.qs.dnd.{label}"), label)
                .on_press(move || DoNotDisturb::set(Some(end)))
        })
        .collect();
    let rows = items.len() as f32;
    let height = rows * sys::size::row::height() + (rows - 1.0) * sys::space::gap::xs();
    let body: Vec<Box<dyn Widget>> = vec![boxed(List::new(items).build())];

    DetailPage::new("amane.qs.dnd.page", ICON_DND, "Do Not Disturb")
        .subtitle(status(until))
        .switch(until.is_some(), |on| {
            DoNotDisturb::set(on.then_some(Until::Forever))
        })
        .body(body, height)
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    #[test]
    fn says_how_long_it_lasts() {
        assert_eq!(status(None), "Off");
        assert_eq!(status(Some(Until::Forever)), "On");
        assert!(status(Some(Until::At(SystemTime::now()))).starts_with("Until "));
    }
}
