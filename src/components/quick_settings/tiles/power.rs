//! Power — GNOME's Power Mode as a round button with its page, and the
//! battery's page (opened from the battery tile), which is the power
//! panel's body.
//!
//! As in GNOME, Power Mode is "on" while it isn't Balanced; a click goes to
//! Balanced, or back to the mode last picked.

use std::cell::RefCell;

use amane::{Battery, PowerProfile, Service, Widget};

use super::super::{Page, open};
use crate::components::icons::power_profile_glyph;
use crate::components::layout::{boxed, caption};
use crate::components::list::{self, List, ListItem};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::taskbar::power;
use crate::tokens::sys;

const BALANCED: &str = "balanced";

thread_local! {
    // what a click on the button goes back to from Balanced
    static LAST: RefCell<String> = RefCell::new(String::from("power-saver"));
}

/// GNOME's names for the profiles, the panel's too.
fn label(profile: &str) -> &str {
    power::profile_label(profile)
}

pub(crate) fn mode_toggle() -> Option<QuickToggle> {
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

    Some(
        QuickToggle::new(
            "amane.qs.power-mode",
            power_profile_glyph(&active),
            "Power Mode",
        )
        .subtitle(label(&active))
        .checked(checked)
        .on_toggle(move |on| PowerProfile::set(if on { &back_to } else { BALANCED }))
        .on_open(|| open(Page::PowerMode)),
    )
}

pub(crate) fn mode_page() -> DetailPage {
    let profile = PowerProfile::read();
    let active = profile.active().to_string();
    let available = profile.available().to_vec();
    let degraded = profile.degraded().map(String::from);
    drop(profile);

    DetailPage::new("amane.qs.power-mode.page", "Power Mode")
        .subtitle(label(&active))
        .body(move |_| {
            let items: Vec<ListItem> = available
                .iter()
                .map(|name| {
                    let selected = *name == active;
                    let pick = name.clone();
                    let mut item =
                        ListItem::new(format!("amane.qs.power-mode.{name}"), label(name))
                            .leading(power_profile_glyph(name))
                            .selected(selected)
                            .on_press(move || PowerProfile::set(&pick));
                    if selected {
                        item = item.trailing(list::check());
                    }
                    item
                })
                .collect();
            let mut body: Vec<Box<dyn Widget>> = vec![boxed(List::new(items).build())];
            if let Some(reason) = degraded {
                body.push(boxed(
                    caption(format!("Performance is limited: {reason}"))
                        .color(sys::color::status::critical::text()),
                ));
            }
            body
        })
}

pub(crate) fn battery_page() -> DetailPage {
    let battery = Battery::read();
    let percent = battery.percent();
    let detail = power::battery_detail(&battery);
    drop(battery);

    let mut page =
        DetailPage::new("amane.qs.battery.page", format!("Battery {percent}%")).body(|height| {
            let mut body = Vec::new();
            let mut left = height;
            if PowerProfile::read().present() {
                body.push(power::profile_section());
                left -= sys::size::control::height() + sys::space::gap::l();
            }
            body.push(power::peripheral_section_in(left));
            body
        });
    if !detail.is_empty() {
        page = page.subtitle(detail);
    }
    page
}
