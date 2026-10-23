//! Power panel — a bottom-right `LayerWindow` with the battery's details,
//! the power-profile toggle and the batteries of connected peripherals.
//!
//! Same shape as the [`wifi`](super::wifi) panel: a separate surface toggled
//! by the bar's battery indicator, dismissed with Escape or by toggling
//! again. Everything shown is read from the fork's `Battery` (UPower) and
//! `PowerProfile` (power-profiles-daemon) services, so it updates live.

use std::cell::Cell;
use std::time::Duration;

use amane::{
    Battery, BatteryState, Button, Center, Color, Column, DeviceKind, Horizontal, Key, Keyboard,
    Layer, LayerWindow, Margin, Parent, Pointer, PowerDevice, PowerProfile, Rectangle, Row,
    ScrollArea, Service, Size, SpaceBetween, Start, Text, Vertical, WarningLevel, Widget, Zone,
    mark_all,
};

use super::{
    ICON_BATTERY, ICON_BATTERY_CHARGING, ICON_GAMEPAD, ICON_HEADSET, ICON_KEYBOARD, ICON_MOUSE,
    boxed, icon, pad,
};
use crate::tokens::{Styled, sys};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const POWER_NS: &str = "amane.taskbar.power";

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

pub fn toggle() {
    OPEN.with(|open| open.set(!open.get()));
    mark_all();
}

pub fn close() {
    OPEN.with(|open| open.set(false));
    mark_all();
}

fn is_open() -> bool {
    OPEN.with(Cell::get)
}

// ───── view ──────────────────────────────────────────────────────────────────

pub fn panel() -> LayerWindow {
    let open = is_open();

    let base = LayerWindow::new()
        .width(sys::size::popover::width())
        .height(sys::size::popover::height())
        .anchor_vertical(Vertical::Bottom)
        .anchor_horizontal(Horizontal::Right)
        .margin(Margin {
            top: 0,
            right: sys::space::gap::m() as i32,
            bottom: (sys::size::panel::height() + sys::space::gap::m()) as i32,
            left: 0,
        })
        .layer(Layer::Overlay)
        .space(Zone::Respect)
        .keyboard(Keyboard::OnDemand)
        .namespace(POWER_NS)
        .visible(open)
        .on_key(|key| {
            if matches!(key, Key::Escape) {
                close();
            }
        });

    if !open {
        return base.child(
            Rectangle::new()
                .width(Parent)
                .height(Parent)
                .fill(Color::TRANSPARENT),
        );
    }

    base.child(card())
}

fn card() -> Rectangle {
    let mut body = vec![boxed(header())];

    if PowerProfile::read().present() {
        body.push(profile_section());
    }

    body.push(peripheral_section());

    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(sys::radius::popover())
        .fill(sys::color::surface::panel::bg())
        .blur(sys::effect::backdrop_blur())
        .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
        .child(Column::new(body).gap(sys::space::gap::xl()).width(Parent))
}

fn header() -> Row {
    let battery = Battery::read();
    let present = battery.present();
    let percent = battery.percent();
    let on_ac = battery.on_ac();
    let low = battery.warning() >= WarningLevel::Low;
    let detail = battery_detail(&battery);
    drop(battery);

    let glyph = if on_ac {
        ICON_BATTERY_CHARGING
    } else {
        ICON_BATTERY
    };
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

    let left = Row::new(vec![
        boxed(icon(glyph, sys::size::icon::m(), color)),
        boxed(Text::new(title).style(sys::r#type::title()).color(color)),
    ])
    .gap(sys::space::gap::s())
    .align(Center);

    Row::new(vec![
        boxed(left),
        boxed(
            Text::new(detail)
                .style(sys::r#type::caption())
                .color(sys::color::content::secondary()),
        ),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center)
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

    let inner = sys::size::popover::width() - sys::space::inset::l() * 2.0;
    let gap = sys::space::gap::xs();
    let count = available.len().max(1) as f32;
    let segment_w = ((inner - gap * (count - 1.0)) / count).max(1.0);

    // only what this machine offers, so a 2-profile laptop shows no "performance"
    let segments: Vec<Box<dyn Widget>> = available
        .into_iter()
        .map(|name| boxed(profile_segment(name, &active, segment_w)))
        .collect();

    let mut rows = vec![boxed(Row::new(segments).gap(gap).width(Parent))];

    if let Some(reason) = degraded {
        rows.push(boxed(
            Text::new(format!("performance limited: {reason}"))
                .style(sys::r#type::caption())
                .color(sys::color::status::critical::bg()),
        ));
    }

    boxed(Column::new(rows).gap(sys::space::gap::s()).width(Parent))
}

fn profile_segment(name: String, active: &str, width: f32) -> Rectangle {
    let selected = name == active;
    let label = match name.as_str() {
        "power-saver" => "Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
    .to_string();

    let (fill, color) = if selected {
        (sys::color::accent::bg(), sys::color::surface::panel::fg())
    } else {
        (
            sys::color::surface::raised::bg(),
            sys::color::content::secondary(),
        )
    };

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(sys::size::control::height()))
        .radius(sys::radius::raised())
        .fill(fill)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                PowerProfile::set(&name);
            }
        })
        .child(Text::new(label).style(sys::r#type::caption()).color(color))
}

// ───── peripherals ───────────────────────────────────────────────────────────

fn peripheral_section() -> Box<dyn Widget> {
    let battery = Battery::read();
    let rows: Vec<Box<dyn Widget>> = battery
        .peripherals()
        .iter()
        .map(|device| boxed(peripheral_row(device)))
        .collect();
    drop(battery);

    if rows.is_empty() {
        return boxed(
            Text::new("No devices with a battery")
                .style(sys::r#type::caption())
                .color(sys::color::content::secondary()),
        );
    }

    let column = Column::new(rows).gap(sys::space::gap::xs()).width(Parent);

    boxed(
        ScrollArea::new("amane.power.devices", column).height(Size::Fixed(
            sys::size::list::max_height() - sys::size::row::height(),
        )),
    )
}

fn peripheral_row(device: &PowerDevice) -> Rectangle {
    let glyph = match device.kind() {
        DeviceKind::Mouse => ICON_MOUSE,
        DeviceKind::Keyboard => ICON_KEYBOARD,
        DeviceKind::Headset => ICON_HEADSET,
        DeviceKind::GamingInput => ICON_GAMEPAD,
        _ => ICON_BATTERY,
    };
    let model = match device.model() {
        "" => String::from("Unknown device"),
        model => model.to_string(),
    };
    let model_w = (sys::size::popover::width() - sys::size::row::trailing_width()).max(1.0);
    let charging = device.state() == BatteryState::Charging;
    let percent_color = if device.percent() <= 15 && !charging {
        sys::color::status::critical::bg()
    } else {
        sys::color::content::secondary()
    };

    let left = Row::new(vec![
        boxed(icon(
            glyph,
            sys::size::icon::m(),
            sys::color::surface::panel::fg(),
        )),
        boxed(
            Rectangle::new()
                .width(Size::Fixed(model_w))
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .clip()
                .child(
                    Text::new(model)
                        .style(sys::r#type::label())
                        .color(sys::color::surface::panel::fg())
                        .elide(),
                ),
        ),
    ])
    .gap(sys::space::gap::m())
    .align(Center);

    let content = Row::new(vec![
        boxed(left),
        boxed(
            Text::new(format!("{}%", device.percent()))
                .style(sys::r#type::label())
                .color(percent_color),
        ),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center);

    Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(sys::size::row::height()))
        .radius(sys::radius::raised())
        .fill(sys::color::surface::raised::bg())
        .padding(pad(sys::space::inset::l(), 0.0))
        .align_child(Center, Center)
        .clip()
        .child(content)
}
