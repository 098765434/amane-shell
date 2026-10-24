//! Power — GNOME's Power Mode toggle and its page, and the battery's page
//! (opened from the system row), which is the power panel's body.
//!
//! As in GNOME, Power Mode is "on" while it isn't Balanced; a click goes to
//! Balanced, or back to the mode last picked.

use std::cell::RefCell;

use amane::{Battery, PowerProfile, Service, Widget};

use super::GridCell;
use crate::components::icons::{battery_glyph, power_profile_glyph};
use crate::components::layout::{boxed, caption, text_height};
use crate::components::list::{self, List, ListItem};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::quick_settings::{Page, open};
use crate::components::taskbar::power;
use crate::tokens::sys;

const BALANCED: &str = "balanced";

thread_local! {
    // what a click on the tile goes back to from Balanced
    static LAST: RefCell<String> = RefCell::new(String::from("power-saver"));
}

/// GNOME's names for the profiles.
fn label(profile: &str) -> &str {
    match profile {
        "power-saver" => "Power Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

pub(crate) fn mode_cell() -> Option<GridCell> {
    let profile = PowerProfile::read();
    if !profile.present() {
        return None;
    }
    let active = profile.active().to_string();
    let available = profile.available().to_vec();
    drop(profile);

    if active != BALANCED {
        LAST.with_borrow_mut(|last| last.clone_from(&active));
    }
    let checked = active != BALANCED;
    let back_to = LAST.with_borrow(Clone::clone);
    let back_to = if available.contains(&back_to) {
        back_to
    } else {
        available
            .iter()
            .find(|name| *name != BALANCED)
            .cloned()
            .unwrap_or_else(|| String::from(BALANCED))
    };

    let tile = QuickToggle::new(
        "amane.qs.power-mode",
        power_profile_glyph(&active),
        "Power Mode",
    )
    .subtitle(label(&active))
    .checked(checked)
    .on_toggle(move |on| PowerProfile::set(if on { &back_to } else { BALANCED }))
    .on_open(|| open(Page::PowerMode))
    .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn mode_page() -> DetailPage {
    let profile = PowerProfile::read();
    let active = profile.active().to_string();
    let available = profile.available().to_vec();
    let degraded = profile.degraded().map(String::from);
    drop(profile);

    let items: Vec<ListItem> = available
        .iter()
        .map(|name| {
            let selected = *name == active;
            let pick = name.clone();
            let mut item = ListItem::new(format!("amane.qs.power-mode.{name}"), label(name))
                .leading(power_profile_glyph(name))
                .selected(selected)
                .on_press(move || PowerProfile::set(&pick));
            if selected {
                item = item.trailing(list::check());
            }
            item
        })
        .collect();

    let rows = items.len() as f32;
    let mut height =
        rows * sys::size::row::height() + (rows - 1.0).max(0.0) * sys::space::gap::xs();
    let mut body: Vec<Box<dyn Widget>> = vec![boxed(List::new(items).build())];
    if let Some(reason) = degraded {
        height += sys::space::gap::l() + text_height(sys::r#type::caption());
        body.push(boxed(
            caption(format!("Performance is limited: {reason}"))
                .color(sys::color::status::critical::bg()),
        ));
    }

    DetailPage::new(
        "amane.qs.power-mode.page",
        power_profile_glyph(&active),
        "Power Mode",
    )
    .subtitle(label(&active))
    .body(body, height.max(sys::size::row::height()))
}

pub(crate) fn battery_page() -> DetailPage {
    let battery = Battery::read();
    let percent = battery.percent();
    let on_ac = battery.on_ac();
    let detail = power::battery_detail(&battery);
    drop(battery);

    let mut body = Vec::new();
    // the peripherals' list, or the note that there are none
    let mut height = if Battery::read().peripherals().is_empty() {
        text_height(sys::r#type::caption())
    } else {
        sys::size::list::max_height() - sys::size::row::height()
    };
    if PowerProfile::read().present() {
        body.push(power::profile_section());
        height += sys::size::control::height() + sys::space::gap::l();
    }
    body.push(power::peripheral_section());

    let mut page = DetailPage::new(
        "amane.qs.battery.page",
        battery_glyph(percent, on_ac),
        format!("Battery {percent}%"),
    )
    .body(body, height);
    if !detail.is_empty() {
        page = page.subtitle(detail);
    }
    page
}
