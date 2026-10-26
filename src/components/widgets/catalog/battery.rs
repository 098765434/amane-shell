//! Batteries — macOS's widget: the machine's battery and each device's
//! charge in a ring with the device in the middle. Small shows one (the
//! one its settings name, else the first), Medium up to four side by side,
//! Large a list with the time the machine's battery has left.

use std::time::Duration;

use amane::{Battery, BatteryState, Bluetooth, Service, Widget};

use super::{
    Face, accent, bar, caption_style, figure_style, header, header_height, label_style, line,
    percent, primary, ring_with, secondary,
};
use crate::components::battery::LOW;
use crate::components::icons::{ICON_COMPUTER, battery_glyph, device_glyph, peripheral_glyph};
use crate::components::layout::text_height;
use crate::components::widgets::kind::{WidgetKind, WidgetSize};
use crate::tokens::sys;

/// One thing with a battery.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    /// What a widget's settings name it by: "system", a peripheral's
    /// model, or a Bluetooth device's address.
    pub id: String,
    pub name: String,
    pub glyph: char,
    pub percent: u8,
    pub charging: bool,
}

impl Device {
    fn low(&self) -> bool {
        self.percent <= LOW && !self.charging
    }
}

/// The machine's battery, then UPower's peripherals, then the Bluetooth
/// devices that report a charge UPower doesn't know.
pub fn devices() -> Vec<Device> {
    let battery = Battery::read();
    let mut out = Vec::new();
    if battery.present() {
        out.push(Device {
            id: String::from("system"),
            name: String::from("This Computer"),
            glyph: ICON_COMPUTER,
            percent: battery.percent(),
            charging: battery.charging(),
        });
    }
    for device in battery.peripherals() {
        out.push(Device {
            id: device.model().to_string(),
            name: device.model().to_string(),
            glyph: peripheral_glyph(device),
            percent: device.percent(),
            charging: device.state() == BatteryState::Charging,
        });
    }
    for device in Bluetooth::read().devices() {
        let Some(percent) = device.battery().filter(|_| device.connected()) else {
            continue;
        };
        if out.iter().any(|known| known.name == device.name()) {
            continue;
        }
        out.push(Device {
            id: device.address().to_string(),
            name: device.name().to_string(),
            glyph: device_glyph(device.icon()),
            percent,
            charging: false,
        });
    }
    out
}

/// "2 h 14 min left", "1 h 5 min until full", or nothing to say.
fn time_left(battery: &Battery) -> Option<String> {
    let span = |d: Duration| {
        let minutes = d.as_secs() / 60;
        match (minutes / 60, minutes % 60) {
            (0, m) => format!("{m} min"),
            (h, 0) => format!("{h} h"),
            (h, m) => format!("{h} h {m} min"),
        }
    };
    if battery.charging() {
        battery
            .time_to_full()
            .map(|d| format!("{} until full", span(d)))
    } else if battery.full() {
        Some(String::from("Charged"))
    } else {
        battery.time_to_empty().map(|d| format!("{} left", span(d)))
    }
}

pub(super) fn face(face: &Face) -> Vec<Box<dyn Widget>> {
    let devices = devices();
    match face.size {
        WidgetSize::Small => small(face, &devices),
        WidgetSize::Medium => medium(face, &devices),
        _ => large(face, &devices),
    }
}

fn none(face: &Face) -> Vec<Box<dyn Widget>> {
    let mut out = header(WidgetKind::Battery, "Batteries", face.width);
    out.push(line(
        "No Devices",
        label_style(),
        secondary(),
        (0.0, face.height / 2.0 - text_height(label_style()) / 2.0),
        face.width,
        false,
    ));
    out
}

fn small(face: &Face, devices: &[Device]) -> Vec<Box<dyn Widget>> {
    let wanted = face.settings.device.as_deref();
    let Some(device) = devices
        .iter()
        .find(|d| Some(d.id.as_str()) == wanted)
        .or_else(|| devices.first())
    else {
        return none(face);
    };
    let figure = figure_style(true);
    let figure_h = text_height(figure);
    let name_h = text_height(caption_style());
    let diameter = (face.height - figure_h - name_h).min(face.width * 0.6);
    let mut out = vec![super::place(
        ring_with(
            f32::from(device.percent) / 100.0,
            diameter,
            diameter * 0.09,
            accent(device.low()),
            device.glyph,
        ),
        0.0,
        0.0,
        diameter,
        diameter,
    )];
    if device.charging {
        let side = text_height(label_style());
        out.push(super::place(
            crate::components::layout::icon(battery_glyph(device.percent, true), side, primary()),
            face.width - side,
            0.0,
            side,
            side,
        ));
    }
    out.push(line(
        percent(f32::from(device.percent) / 100.0),
        figure,
        primary(),
        (0.0, face.height - figure_h - name_h),
        face.width,
        false,
    ));
    out.push(line(
        device.name.clone(),
        caption_style(),
        secondary(),
        (0.0, face.height - name_h),
        face.width,
        false,
    ));
    out
}

fn medium(face: &Face, devices: &[Device]) -> Vec<Box<dyn Widget>> {
    if devices.is_empty() {
        return none(face);
    }
    let caption = caption_style();
    let caption_h = text_height(caption);
    let column = face.width / 4.0;
    let diameter =
        (face.height - caption_h - sys::space::gap::s()).min(column - sys::space::gap::m());
    let top = (face.height - diameter - caption_h - sys::space::gap::s()) / 2.0;
    let mut out = Vec::new();
    for (i, device) in devices.iter().take(4).enumerate() {
        let x = column * i as f32 + (column - diameter) / 2.0;
        out.push(super::place(
            ring_with(
                f32::from(device.percent) / 100.0,
                diameter,
                diameter * 0.09,
                accent(device.low()),
                device.glyph,
            ),
            x,
            top,
            diameter,
            diameter,
        ));
        let label = percent(f32::from(device.percent) / 100.0);
        let width = crate::components::layout::text_width(&label, caption);
        out.push(line(
            label,
            caption,
            primary(),
            (
                column * i as f32 + (column - width) / 2.0,
                top + diameter + sys::space::gap::s(),
            ),
            width + 1.0,
            false,
        ));
    }
    out
}

fn large(face: &Face, devices: &[Device]) -> Vec<Box<dyn Widget>> {
    if devices.is_empty() {
        return none(face);
    }
    let mut out = header(WidgetKind::Battery, "Batteries", face.width);
    let label = label_style();
    let caption = caption_style();
    let row = text_height(label) + text_height(caption) + sys::space::gap::m();
    let mut y = header_height() + sys::space::gap::m();
    let side = text_height(label) + text_height(caption);
    let battery = Battery::read();
    for device in devices {
        if y + row > face.height {
            break;
        }
        out.push(super::place(
            crate::components::layout::icon(device.glyph, side * 0.8, primary()),
            0.0,
            y + side * 0.1,
            side,
            side,
        ));
        let x = side + sys::space::gap::s();
        let text_w = face.width - x;
        let figure = percent(f32::from(device.percent) / 100.0);
        out.push(line(
            device.name.clone(),
            label,
            primary(),
            (x, y),
            text_w * 0.7,
            false,
        ));
        out.push(line(figure, label, primary(), (x, y), text_w, true));
        let below = if device.id == "system" {
            time_left(&battery)
        } else if device.charging {
            Some(String::from("Charging"))
        } else {
            None
        };
        let bar_y = y + text_height(label) + sys::space::gap::xs();
        match below {
            Some(text) => out.push(line(text, caption, secondary(), (x, bar_y), text_w, false)),
            None => out.push(super::place(
                bar(
                    f32::from(device.percent) / 100.0,
                    text_w,
                    4.0,
                    accent(device.low()),
                ),
                x,
                bar_y + text_height(caption) / 2.0 - 2.0,
                text_w,
                4.0,
            )),
        }
        y += row;
    }
    out
}
