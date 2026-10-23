//! Power panel — a bottom-right `LayerWindow` with the battery's details,
//! the power-profile toggle and the batteries of connected peripherals.
//!
//! One of the bar's [`panel`](super::panel)s, toggled by the battery
//! indicator. Everything shown is read from the fork's `Battery` (UPower) and
//! `PowerProfile` (power-profiles-daemon) services, so it updates live.

use std::time::Duration;

use amane::{
    Battery, BatteryState, Column, LayerWindow, Parent, PowerDevice, PowerProfile, Rectangle, Row,
    ScrollArea, Service, Size, Text, Widget,
};

use super::panel::{self, Panel};
use super::{battery_color, battery_low};
use crate::components::app_bar::TopAppBar;
use crate::components::buttons::{Segment, SegmentedButton};
use crate::components::icons::{battery_glyph, peripheral_glyph};
use crate::components::layout::{boxed, caption};
use crate::components::list::{List, ListItem};
use crate::tokens::{Styled, sys};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const POWER_NS: &str = "amane.taskbar.power";

// ───── view ──────────────────────────────────────────────────────────────────

pub fn panel() -> LayerWindow {
    panel::surface(POWER_NS, Panel::Power, card)
}

fn card() -> Rectangle {
    let mut body = vec![boxed(header())];

    if PowerProfile::read().present() {
        body.push(profile_section());
    }

    body.push(peripheral_section());

    panel::card(body, sys::space::gap::xl())
}

fn header() -> Row {
    let battery = Battery::read();
    let present = battery.present();
    let percent = battery.percent();
    let on_ac = battery.on_ac();
    let low = battery_low(&battery);
    let detail = battery_detail(&battery);
    drop(battery);

    let glyph = battery_glyph(percent, on_ac);
    let color = if low {
        sys::color::status::critical::bg()
    } else {
        sys::color::surface::panel::fg()
    };
    let title = if present {
        format!("Battery {percent}%")
    } else {
        String::from("Power")
    };

    TopAppBar::new(title)
        .navigation_icon(glyph)
        .color(color)
        .actions(vec![boxed(caption(detail))])
        .build()
}

/// "2 h 10 min left", "full in 45 min", "on AC" …
fn battery_detail(battery: &Battery) -> String {
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

// ───── power profile ─────────────────────────────────────────────────────────

fn profile_section() -> Box<dyn Widget> {
    let profile = PowerProfile::read();
    let active = profile.active().to_string();
    let available = profile.available().to_vec();
    let degraded = profile.degraded().map(String::from);
    drop(profile);

    // only what this machine offers, so a 2-profile laptop shows no "performance"
    let segments = available
        .iter()
        .map(|name| Segment::new(profile_label(name)))
        .collect();
    let selected = available.iter().position(|name| *name == active);

    // as wide as the card's content, the button's default
    let picker = SegmentedButton::new("amane.power.profile", segments, selected)
        .on_select(move |index, _| PowerProfile::set(&available[index]))
        .build();

    let mut rows = vec![boxed(picker)];

    if let Some(reason) = degraded {
        rows.push(boxed(
            caption(format!("performance limited: {reason}"))
                .color(sys::color::status::critical::bg()),
        ));
    }

    boxed(Column::new(rows).gap(sys::space::gap::s()).width(Parent))
}

fn profile_label(name: &str) -> &str {
    match name {
        "power-saver" => "Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

// ───── peripherals ───────────────────────────────────────────────────────────

fn peripheral_section() -> Box<dyn Widget> {
    let battery = Battery::read();
    let items: Vec<ListItem> = battery.peripherals().iter().map(peripheral_item).collect();
    drop(battery);

    if items.is_empty() {
        return boxed(caption("No devices with a battery"));
    }

    boxed(
        ScrollArea::new("amane.power.devices", List::new(items).build()).height(Size::Fixed(
            sys::size::list::max_height() - sys::size::row::height(),
        )),
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
    ListItem::new(format!("amane.power.device.{model}"), model)
        .leading(peripheral_glyph(device))
        .color(sys::color::surface::panel::fg())
        .trailing(
            // percentages are numeric (manual §11.4)
            Text::new(format!("{percent}%"))
                .style(sys::r#type::numeric())
                .color(battery_color(
                    percent,
                    charging,
                    sys::color::content::secondary(),
                )),
        )
}
