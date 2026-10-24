//! Keyboard — GNOME's keyboard backlight toggle: on at the level last used,
//! and a page to pick it, as steps (Off, Low, High) when the keyboard has
//! only a few, else a slider.

use std::cell::Cell as Slot;

use amane::Service;

use super::GridCell;
use crate::components::buttons::{Segment, SegmentedButton};
use crate::components::icons::ICON_KEYBOARD_LIGHT;
use crate::components::layout::boxed;
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::{QuickToggle, Span};
use crate::components::quick_settings::{Page, open};
use crate::components::slider::Slider;
use crate::services::keyboard_backlight::KeyboardBacklight;
use crate::tokens::sys;

/// Up to this many steps above off are picked from named segments.
const MOST_STEPS: u32 = 3;

thread_local! {
    // what turning it back on restores; none means the brightest
    static LAST: Slot<Option<u32>> = const { Slot::new(None) };
}

/// The names of a keyboard's steps from off, or none when there are too
/// many to name.
fn steps(max: u32) -> Option<&'static [&'static str]> {
    match max {
        1 => Some(&["Off", "On"]),
        2 => Some(&["Off", "Low", "High"]),
        3 => Some(&["Off", "Low", "Medium", "High"]),
        _ => None,
    }
}

fn describe(level: u32, max: u32) -> String {
    match steps(max) {
        Some(names) => names[level.min(max) as usize].to_string(),
        None if level == 0 => String::from("Off"),
        None => format!("{}%", level * 100 / max.max(1)),
    }
}

pub(crate) fn cell() -> Option<GridCell> {
    let light = KeyboardBacklight::read();
    if !light.present() {
        return None;
    }
    let (level, max) = (light.level(), light.max());
    drop(light);

    if level > 0 {
        LAST.set(Some(level));
    }
    let restore = LAST.get().unwrap_or(max);

    let tile = QuickToggle::new("amane.qs.keyboard", ICON_KEYBOARD_LIGHT, "Keyboard")
        .subtitle(describe(level, max))
        .checked(level > 0)
        .on_toggle(move |on| KeyboardBacklight::set(if on { restore } else { 0 }))
        .on_open(|| open(Page::Keyboard))
        .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn page() -> DetailPage {
    let light = KeyboardBacklight::read();
    let (level, max) = (light.level(), light.max());
    drop(light);
    let restore = LAST.get().unwrap_or(max);

    let picker = match steps(max) {
        Some(names) if max <= MOST_STEPS => {
            let segments = names.iter().map(|name| Segment::new(*name)).collect();
            SegmentedButton::new("amane.qs.keyboard.level", segments, Some(level as usize))
                .on_select(|index, _| KeyboardBacklight::set(index as u32))
                .build()
        }
        _ => Slider::new((level * 100 / max.max(1)) as u8, Span::Full.width())
            .large(ICON_KEYBOARD_LIGHT)
            .on_value_change(move |value| {
                KeyboardBacklight::set((u32::from(value) * max).div_ceil(100))
            })
            .build(),
    };

    DetailPage::new("amane.qs.keyboard.page", ICON_KEYBOARD_LIGHT, "Keyboard")
        .subtitle(describe(level, max))
        .switch(level > 0, move |on| {
            KeyboardBacklight::set(if on { restore } else { 0 })
        })
        .body(vec![boxed(picker)], sys::size::control::height())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_few_steps_have_names_and_many_a_percentage() {
        assert_eq!(describe(0, 2), "Off");
        assert_eq!(describe(1, 2), "Low");
        assert_eq!(describe(2, 2), "High");
        assert_eq!(describe(1, 1), "On");
        assert_eq!(describe(0, 255), "Off");
        assert_eq!(describe(51, 255), "20%");
    }
}
