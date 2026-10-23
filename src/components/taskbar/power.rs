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

use super::theme::Theme;
use super::tokens::{Tokens, tokens};
use super::{
    ICON_BATTERY, ICON_BATTERY_CHARGING, ICON_GAMEPAD, ICON_HEADSET, ICON_KEYBOARD, ICON_MOUSE,
    boxed, icon, pad,
};

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
    let t = tokens();
    let theme = Theme::auto();
    let open = is_open();

    let base = LayerWindow::new()
        .width(t.panel_w)
        .height(t.panel_h)
        .anchor_vertical(Vertical::Bottom)
        .anchor_horizontal(Horizontal::Right)
        .margin(Margin {
            top: 0,
            right: t.gap_m as i32,
            bottom: (t.height_compact + t.gap_m) as i32,
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

    base.child(card(theme, t))
}

fn card(theme: Theme, t: &Tokens) -> Rectangle {
    let mut body = vec![boxed(header(theme, t))];

    if PowerProfile::read().present() {
        body.push(profile_section(theme, t));
    }

    body.push(peripheral_section(theme, t));

    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(t.radius_popover)
        .fill(theme.surface)
        .blur(t.blur)
        .padding(pad(t.inset_l, t.inset_l))
        .child(Column::new(body).gap(t.gap_xl).width(Parent))
}

fn header(theme: Theme, t: &Tokens) -> Row {
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
    let color = if low { theme.urgent } else { theme.fg_strong };
    let title = if present {
        format!("Battery {percent}%")
    } else {
        String::from("Power")
    };

    let left = Row::new(vec![
        boxed(icon(glyph, t.icon_size, color, t)),
        boxed(
            Text::new(title)
                .size(t.title_size)
                .color(color)
                .font(&t.font_family)
                .weight(t.weight_medium),
        ),
    ])
    .gap(t.gap_s)
    .align(Center);

    Row::new(vec![
        boxed(left),
        boxed(
            Text::new(detail)
                .size(t.caption_size)
                .color(theme.fg)
                .font(&t.font_family),
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

fn profile_section(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    let profile = PowerProfile::read();
    let active = profile.active().to_string();
    let available = profile.available().to_vec();
    let degraded = profile.degraded().map(String::from);
    drop(profile);

    let inner = t.panel_w - t.inset_l * 2.0;
    let gap = t.gap_xs;
    let count = available.len().max(1) as f32;
    let segment_w = ((inner - gap * (count - 1.0)) / count).max(1.0);

    // only what this machine offers, so a 2-profile laptop shows no "performance"
    let segments: Vec<Box<dyn Widget>> = available
        .into_iter()
        .map(|name| boxed(profile_segment(name, &active, segment_w, theme, t)))
        .collect();

    let mut rows = vec![boxed(Row::new(segments).gap(gap).width(Parent))];

    if let Some(reason) = degraded {
        rows.push(boxed(
            Text::new(format!("performance limited: {reason}"))
                .size(t.caption_size)
                .color(theme.urgent)
                .font(&t.font_family),
        ));
    }

    boxed(Column::new(rows).gap(t.gap_s).width(Parent))
}

fn profile_segment(name: String, active: &str, width: f32, theme: Theme, t: &Tokens) -> Rectangle {
    let selected = name == active;
    let label = match name.as_str() {
        "power-saver" => "Saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
    .to_string();

    let (fill, color) = if selected {
        (theme.accent, theme.fg_strong)
    } else {
        (theme.pill, theme.fg)
    };

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(t.control_h))
        .radius(t.radius_pill)
        .fill(fill)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                PowerProfile::set(&name);
            }
        })
        .child(
            Text::new(label)
                .size(t.caption_size)
                .color(color)
                .font(&t.font_family),
        )
}

// ───── peripherals ───────────────────────────────────────────────────────────

fn peripheral_section(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    let battery = Battery::read();
    let rows: Vec<Box<dyn Widget>> = battery
        .peripherals()
        .iter()
        .map(|device| boxed(peripheral_row(device, theme, t)))
        .collect();
    drop(battery);

    if rows.is_empty() {
        return boxed(
            Text::new("No devices with a battery")
                .size(t.caption_size)
                .color(theme.fg)
                .font(&t.font_family),
        );
    }

    let column = Column::new(rows).gap(t.gap_xs).width(Parent);

    boxed(ScrollArea::new("amane.power.devices", column).height(Size::Fixed(t.list_h - t.row_h)))
}

fn peripheral_row(device: &PowerDevice, theme: Theme, t: &Tokens) -> Rectangle {
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
    let model_w = (t.panel_w - t.row_trailing_w).max(1.0);
    let charging = device.state() == BatteryState::Charging;
    let percent_color = if device.percent() <= 15 && !charging {
        theme.urgent
    } else {
        theme.fg
    };

    let left = Row::new(vec![
        boxed(icon(glyph, t.icon_size, theme.fg_strong, t)),
        boxed(
            Rectangle::new()
                .width(Size::Fixed(model_w))
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .clip()
                .child(
                    Text::new(model)
                        .size(t.label_size)
                        .color(theme.fg_strong)
                        .font(&t.font_family)
                        .elide(),
                ),
        ),
    ])
    .gap(t.gap_m)
    .align(Center);

    let content = Row::new(vec![
        boxed(left),
        boxed(
            Text::new(format!("{}%", device.percent()))
                .size(t.label_size)
                .color(percent_color)
                .font(&t.font_family),
        ),
    ])
    .width(Parent)
    .justify(SpaceBetween)
    .align(Center);

    Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(t.row_h))
        .radius(t.radius_pill)
        .fill(theme.pill)
        .padding(pad(t.inset_l, 0.0))
        .align_child(Center, Center)
        .clip()
        .child(content)
}
