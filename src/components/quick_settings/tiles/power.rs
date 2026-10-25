//! Power — GNOME's Power Mode as a round button with its page, and the
//! battery's page (opened from the battery tile): the power-profile picker
//! and the batteries of connected peripherals. Read from the fork's
//! `Battery` (UPower) and `PowerProfile` (power-profiles-daemon) services.
//!
//! As in GNOME, Power Mode is "on" while it isn't Balanced; a click goes to
//! Balanced, or back to the mode last picked.

use std::cell::RefCell;
use std::time::Duration;

use amane::{
    Battery, BatteryState, Column, Parent, PowerDevice, PowerProfile, ScrollArea, Service, Size,
    Text, Widget,
};

use super::super::{Page, open};
use crate::components::battery;
use crate::components::buttons::{Segment, SegmentedButton};
use crate::components::icons::{peripheral_glyph, power_profile_glyph};
use crate::components::layout::{boxed, caption};
use crate::components::list::{self, List, ListItem};
use crate::components::quick_settings::detail::{DetailPage, page_width};
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::status_bar::Indicator;
use crate::tokens::{Styled, sys};

const BALANCED: &str = "balanced";

thread_local! {
    // what a click on the button goes back to from Balanced
    static LAST: RefCell<String> = RefCell::new(String::from("power-saver"));
}

/// GNOME's names for the profiles.
fn label(name: &str) -> &str {
    match name {
        "power-saver" => "Power Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
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
    let detail = battery_detail(&battery);
    drop(battery);

    let mut page = DetailPage::new("amane.qs.battery.page", format!("Battery {percent}%"))
        .status_bar(Indicator::Battery)
        .body(|height| {
            let mut body = Vec::new();
            let mut left = height;
            if PowerProfile::read().present() {
                body.push(profile_section());
                left -= sys::size::control::height() + sys::space::gap::l();
            }
            body.push(peripheral_section(left));
            body
        });
    if !detail.is_empty() {
        page = page.subtitle(detail);
    }
    page
}

/// "2 h 10 min left", "full in 45 min", "on AC" …
pub(crate) fn battery_detail(battery: &Battery) -> String {
    match battery.state() {
        BatteryState::Discharging => battery
            .time_to_empty()
            .map(|left| format!("{} left", duration(left)))
            .unwrap_or_else(|| String::from("on battery")),
        BatteryState::Charging => battery
            .time_to_full()
            .map(|left| format!("full in {}", duration(left)))
            .unwrap_or_else(|| String::from("charging")),
        BatteryState::Full => String::from("full"),
        _ if battery.on_ac() => String::from("on AC"),
        _ => String::new(),
    }
}

fn duration(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// The profile picker, and why performance is held back if it is.
fn profile_section() -> Box<dyn Widget> {
    let profile = PowerProfile::read();
    let active = profile.active().to_string();
    let available = profile.available().to_vec();
    let degraded = profile.degraded().map(String::from);
    drop(profile);

    // only what this machine offers, so a 2-profile laptop shows no "performance"
    let segments = available
        .iter()
        .map(|name| Segment::new(label(name)))
        .collect();
    let selected = available.iter().position(|name| *name == active);

    let picker = SegmentedButton::new("amane.qs.battery.profile", segments, selected)
        .width(page_width())
        .on_select(move |index, _| PowerProfile::set(&available[index]))
        .build();

    let mut rows = vec![boxed(picker)];

    if let Some(reason) = degraded {
        rows.push(boxed(
            caption(format!("Performance is limited: {reason}"))
                .color(sys::color::status::critical::text()),
        ));
    }

    boxed(Column::new(rows).gap(sys::space::gap::s()).width(Parent))
}

/// The batteries of connected devices, scrolling in `height`.
fn peripheral_section(height: f32) -> Box<dyn Widget> {
    let battery = Battery::read();
    let items: Vec<ListItem> = battery.peripherals().iter().map(peripheral_item).collect();
    drop(battery);

    boxed(
        ScrollArea::new(
            "amane.qs.battery.devices",
            List::new(items)
                .header("Devices")
                .empty("No devices with a battery")
                .build(),
        )
        .height(Size::Fixed(height.max(1.0))),
    )
}

fn peripheral_item(device: &PowerDevice) -> ListItem {
    let model = match device.model() {
        "" => String::from("Unknown device"),
        model => model.to_string(),
    };
    let charging = device.state() == BatteryState::Charging;
    let percent = device.percent();

    // nothing to pick, so full-strength like a selected item, but unfilled
    ListItem::new(format!("amane.qs.battery.device.{model}"), model)
        .leading(peripheral_glyph(device))
        .color(sys::color::surface::panel::fg())
        .trailing(
            // percentages are numeric (manual §11.4)
            Text::new(format!("{percent}%"))
                .style(sys::r#type::numeric())
                .color(battery::color(
                    percent,
                    charging,
                    sys::color::content::secondary(),
                )),
        )
}
