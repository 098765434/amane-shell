//! Focus — GNOME's Do Not Disturb toggle as Control Center's Focus tile
//! (the tile says "Focus", its subtitle what's on), and a page with the
//! notification center's timed choices: for an hour, until this evening,
//! until tomorrow.

use std::time::Duration;

use amane::Service;
use chrono::Local;

use super::super::{Page, open};
use crate::components::icons::ICON_DND;
use crate::components::layout::boxed;
use crate::components::list::{self, List, ListItem};
use crate::surfaces::notifications::DoNotDisturb;
use crate::surfaces::notifications::card::local;
use crate::surfaces::notifications::dnd::{self, Until};
use crate::surfaces::quick_settings::detail::DetailPage;
use crate::surfaces::quick_settings::tile::QuickToggle;

/// "On", "Until 19:00" or "Off".
fn status(until: Option<Until>) -> String {
    match until {
        None => String::from("Off"),
        Some(Until::Forever) => String::from("On"),
        Some(Until::At(end)) => format!("Until {}", local(end, "%H:%M")),
    }
}

/// The Focus tile's subtitle: the mode that's on and until when, or
/// "Off", short enough for the tile's two lines.
fn focus(until: Option<Until>) -> String {
    match until {
        None => String::from("Off"),
        Some(Until::Forever) => String::from("Do Not Disturb"),
        Some(Until::At(end)) => format!("Until {}", local(end, "%H:%M")),
    }
}

pub(crate) fn toggle() -> QuickToggle {
    let until = DoNotDisturb::read().until();

    QuickToggle::new("amane.qs.dnd", ICON_DND, "Focus")
        .subtitle(focus(until))
        .checked(until.is_some())
        .on_toggle(|on| DoNotDisturb::set(on.then_some(Until::Forever)))
        .on_open(|| open(Page::DoNotDisturb))
}

/// Whether the timed choice `end` is the one that's on: set from the same
/// choice a little earlier, so to the minute.
fn chosen(until: Option<Until>, end: Until) -> bool {
    match (until, end) {
        (Some(Until::At(set)), Until::At(end)) => {
            let apart = set
                .duration_since(end)
                .or_else(|_| end.duration_since(set))
                .unwrap_or(Duration::MAX);
            apart < Duration::from_secs(60)
        }
        (Some(Until::Forever), Until::Forever) => true,
        _ => false,
    }
}

pub(crate) fn page() -> DetailPage {
    let until = DoNotDisturb::read().until();

    DetailPage::new("amane.qs.dnd.page", "Focus")
        .subtitle(match until {
            None => String::from("Off"),
            Some(_) => format!("Do Not Disturb · {}", status(until)),
        })
        .switch(until.is_some(), |on| {
            DoNotDisturb::set(on.then_some(Until::Forever))
        })
        .body(move |_| {
            let items: Vec<ListItem> = dnd::choices(Local::now().naive_local())
                .into_iter()
                .map(|(label, end)| {
                    let on = chosen(until, end);
                    let mut item = ListItem::new(format!("amane.qs.dnd.{label}"), label)
                        .selected(on)
                        .on_press(move || DoNotDisturb::set(Some(end)));
                    if on {
                        item = item.trailing(list::check());
                    }
                    item
                })
                .collect();
            vec![boxed(List::new(items).header("Do Not Disturb for").build())]
        })
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    #[test]
    fn the_choice_that_is_on_is_the_one_set_to_the_minute() {
        let now = SystemTime::now();
        assert!(chosen(
            Some(Until::At(now)),
            Until::At(now + Duration::from_secs(20))
        ));
        assert!(!chosen(
            Some(Until::At(now)),
            Until::At(now + Duration::from_secs(3600))
        ));
        assert!(!chosen(None, Until::At(now)));
        assert!(!chosen(Some(Until::Forever), Until::At(now)));
    }

    #[test]
    fn says_how_long_it_lasts() {
        assert_eq!(status(None), "Off");
        assert_eq!(status(Some(Until::Forever)), "On");
        assert!(status(Some(Until::At(SystemTime::now()))).starts_with("Until "));
        assert_eq!(focus(None), "Off");
        assert!(focus(Some(Until::At(SystemTime::now()))).starts_with("Until "));
    }
}
