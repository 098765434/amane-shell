//! Keep Awake — the screen held on, and the lid ignored if asked, as a
//! tile that lights while anything is overridden, and a page with how long
//! for, the two holds, and a way to turn the screen off without letting go.

use std::time::{Duration, SystemTime};

use amane::Service;

use super::super::{Page, open};
use crate::components::buttons::{Segment, SegmentedButton, Switch};
use crate::components::icons::{ICON_KEEP_AWAKE, ICON_SCREEN_OFF};
use crate::components::layout::boxed;
use crate::components::list::{List, ListItem};
use crate::surfaces::quick_settings::detail::DetailPage;
use crate::surfaces::quick_settings::tile::QuickToggle;
use crate::surfaces::status_bar::Indicator;
use crate::services::power::keep_awake::{KeepAwake, LIMITS};

/// "1 h 24 min", "2 h" or "15 min", to the minute rounded up, so a hold
/// never reads as over while it's still on.
fn duration(left: Duration) -> String {
    let minutes = left.as_secs().div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// What's held and for how long: "Off", "No limit", "1 h 24 min", "Lid
/// ignored" or "Lid · 1 h 24 min".
fn describe(screen: bool, lid: bool, left: Option<Duration>) -> String {
    match (screen, lid, left) {
        (false, false, _) => String::from("Off"),
        (_, true, None) => String::from("Lid ignored"),
        (_, true, Some(left)) => format!("Lid · {}", duration(left)),
        (true, false, None) => String::from("No limit"),
        (true, false, Some(left)) => duration(left),
    }
}

/// What the tile, the page and the bar's icon say.
pub(crate) fn status() -> String {
    let keep = KeepAwake::read();
    describe(keep.screen(), keep.lid(), keep.left(SystemTime::now()))
}

pub(crate) fn toggle() -> Option<QuickToggle> {
    let keep = KeepAwake::read();
    if !keep.present() {
        return None;
    }
    let on = keep.on();
    drop(keep);

    Some(
        QuickToggle::new("amane.qs.keep-awake", ICON_KEEP_AWAKE, "Keep Awake")
            .subtitle(status())
            .checked(on)
            .on_toggle(KeepAwake::set)
            .on_open(|| open(Page::KeepAwake)),
    )
}

pub(crate) fn page() -> DetailPage {
    let keep = KeepAwake::read();
    let (on, screen, lid) = (keep.on(), keep.screen(), keep.lid());
    let lid_present = keep.lid_present();
    // the limit shows as picked only while something's held
    let limit = on
        .then(|| LIMITS.iter().position(|(_, l)| *l == keep.limit()))
        .flatten();
    drop(keep);

    DetailPage::new("amane.qs.keep-awake.page", "Keep Awake")
        .subtitle(status())
        .switch(on, KeepAwake::set)
        .body(move |_| {
            let segments = LIMITS
                .iter()
                .map(|(label, _)| Segment::new(*label))
                .collect();
            let limits = SegmentedButton::new("amane.qs.keep-awake.limit", segments, limit)
                .on_select(|index, _| KeepAwake::set_limit(LIMITS[index].1))
                .build();

            let mut holds = vec![switch_row(
                "screen",
                "Keep screen on",
                screen,
                KeepAwake::set_screen,
            )];
            if lid_present {
                holds.push(switch_row(
                    "lid",
                    "Ignore lid close",
                    lid,
                    KeepAwake::set_lid,
                ));
            }
            holds.push(
                ListItem::new("amane.qs.keep-awake.screen-off", "Turn the screen off now")
                    .leading(ICON_SCREEN_OFF)
                    .on_press(KeepAwake::screen_off_now),
            );

            vec![boxed(limits), boxed(List::new(holds).build())]
        })
        .status_bar(Indicator::KeepAwake)
}

/// A row with a switch at its end; pressing the row flips it too.
fn switch_row(name: &str, label: &str, on: bool, set: fn(bool)) -> ListItem {
    ListItem::new(format!("amane.qs.keep-awake.{name}"), label)
        .trailing(
            Switch::new(format!("amane.qs.keep-awake.{name}.switch"), on)
                .on_toggle(set)
                .build(),
        )
        .on_press(move || set(!on))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left_rounds_up_to_the_minute() {
        assert_eq!(duration(Duration::from_secs(15 * 60)), "15 min");
        assert_eq!(duration(Duration::from_secs(1)), "1 min");
        assert_eq!(duration(Duration::from_secs(2 * 3600)), "2 h");
        assert_eq!(
            duration(Duration::from_secs(3600 + 23 * 60 + 5)),
            "1 h 24 min"
        );
    }

    #[test]
    fn says_whats_held() {
        let hour = Some(Duration::from_secs(3600));
        assert_eq!(describe(false, false, hour), "Off");
        assert_eq!(describe(true, false, None), "No limit");
        assert_eq!(describe(true, false, hour), "1 h");
        assert_eq!(describe(false, true, None), "Lid ignored");
        assert_eq!(describe(true, true, hour), "Lid · 1 h");
    }
}
