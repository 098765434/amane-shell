//! Bluetooth panel — a bottom-right `LayerWindow` to turn Bluetooth on and
//! off and to connect, pair and forget devices.
//!
//! One of the bar's [`panel`](super::panel)s, toggled by the Bluetooth
//! indicator. While it is open the adapter searches for new devices, like the
//! macOS menu, and stops again when it closes. Read from the fork's
//! `Bluetooth` service (BlueZ). Pairing only works for devices that need no
//! PIN: the fork has no BlueZ agent to show or type one.

use std::cell::{Cell, RefCell};
use std::thread;
use std::time::{Duration, Instant};

use amane::{
    Bluetooth, BluetoothDevice, Button, Column, LayerWindow, Parent, Pointer, Rectangle,
    ScrollArea, Service, Size, Text, Widget, mark_all,
};

use super::panel::{self, Panel};
use super::{
    ICON_BLUETOOTH, ICON_COMPUTER, ICON_DEVICE_OTHER, ICON_GAMEPAD, ICON_HEADPHONES, ICON_KEYBOARD,
    ICON_MOUSE, ICON_PHONE, ICON_SPEAKER, boxed,
};
use crate::components::context_menu::{self, ContextMenu, MenuItem};
use crate::tokens::{Styled, sys};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const BLUETOOTH_NS: &str = "amane.taskbar.bluetooth";

/// Registry key for the rebuilt-per-device right-click menu.
const DEVICE_MENU_KEY: &str = "amane.bluetooth.device-menu";

/// How long a row says "Connecting…" before giving up on hearing back;
/// BlueZ answers slow devices within this, or not at all.
const PENDING_FOR: Duration = Duration::from_secs(15);

/// A connect, pair or disconnect asked for but not yet seen to happen.
struct Pending {
    path: String,
    // the connected state that ends it
    connected: bool,
    since: Instant,
}

thread_local! {
    // discovery was started for this opening of the panel
    static SEARCHING: Cell<bool> = const { Cell::new(false) };

    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Stops the search the panel started; the panel calls this as it closes.
pub(super) fn closed() {
    if SEARCHING.replace(false) {
        Bluetooth::stop_scan();
    }
}

fn act(device: &BluetoothDevice) {
    let path = device.path().to_string();
    let connected = device.connected();

    if connected {
        Bluetooth::disconnect(&path);
    } else if device.paired() {
        Bluetooth::connect(&path);
    } else {
        Bluetooth::pair(&path);
    }

    PENDING.with(|p| {
        *p.borrow_mut() = Some(Pending {
            path,
            connected: !connected,
            since: Instant::now(),
        })
    });
    // redraw once the wait is over, so a call that never answers stops saying so
    thread::spawn(|| {
        thread::sleep(PENDING_FOR);
        mark_all();
    });
    mark_all();
}

/// "Connecting…" / "Disconnecting…" while `device` hasn't reached what was
/// asked for yet.
fn pending_label(device: &BluetoothDevice) -> Option<&'static str> {
    PENDING.with(|p| {
        let p = p.borrow();
        let pending = p.as_ref()?;
        let waiting = pending.path == device.path()
            && pending.connected != device.connected()
            && pending.since.elapsed() < PENDING_FOR;
        waiting.then_some(if pending.connected {
            "Connecting…"
        } else {
            "Disconnecting…"
        })
    })
}

fn open_device_menu(device: &BluetoothDevice) {
    let path = device.path().to_string();
    let connected = device.connected();
    let snapshot = device.clone();

    let menu = ContextMenu::new()
        .item(
            MenuItem::label(if connected { "Disconnect" } else { "Connect" })
                .on_select(move || act(&snapshot)),
        )
        .separator()
        .item(
            MenuItem::label("Forget This Device")
                .destructive()
                .on_select(move || Bluetooth::forget(&path)),
        );

    context_menu::register(DEVICE_MENU_KEY, menu);
    let (x, y) = panel::cursor();
    context_menu::open_under(DEVICE_MENU_KEY, BLUETOOTH_NS, x, y, 1, 1);
}

// ───── view ──────────────────────────────────────────────────────────────────

pub fn panel() -> LayerWindow {
    panel::surface(BLUETOOTH_NS, Panel::Bluetooth, card)
}

fn card() -> Rectangle {
    let bluetooth = Bluetooth::read();
    let powered = bluetooth.powered();
    let scanning = bluetooth.scanning();
    let (mine, others): (Vec<BluetoothDevice>, Vec<BluetoothDevice>) = bluetooth
        .devices()
        .iter()
        .cloned()
        .partition(BluetoothDevice::paired);
    drop(bluetooth);

    // Started from the view rather than on open, so it also starts when the
    // radio is switched on with the panel already open; the radio going off
    // ends a search by itself.
    if !powered {
        SEARCHING.set(false);
    } else if !SEARCHING.replace(true) {
        Bluetooth::start_scan();
    }

    let header = panel::header(
        ICON_BLUETOOTH,
        "Bluetooth",
        sys::color::surface::panel::fg(),
        vec![boxed(panel::switch(powered, Bluetooth::set_powered))],
    );

    let mut body = vec![boxed(header)];
    if powered {
        body.push(device_list(mine, others, scanning));
    } else {
        body.push(boxed(panel::caption("Bluetooth is off")));
    }

    panel::card(body, sys::space::gap::l())
}

fn device_list(
    mine: Vec<BluetoothDevice>,
    others: Vec<BluetoothDevice>,
    scanning: bool,
) -> Box<dyn Widget> {
    // unnamed devices only show their address, which nobody can pick out
    let others: Vec<BluetoothDevice> = others
        .into_iter()
        .filter(|device| device.name() != device.address())
        .collect();

    let mut sections: Vec<Box<dyn Widget>> = Vec::new();

    if !mine.is_empty() {
        sections.push(section("My Devices", &mine, None));
    }

    let empty_note = if scanning {
        "Searching…"
    } else {
        "No devices found"
    };
    sections.push(section("Other Devices", &others, Some(empty_note)));

    let column = Column::new(sections)
        .gap(sys::space::gap::xl())
        .width(Parent);

    boxed(
        ScrollArea::new("amane.bluetooth.devices", column)
            .height(Size::Fixed(sys::size::list::max_height())),
    )
}

fn section(title: &str, devices: &[BluetoothDevice], empty: Option<&str>) -> Box<dyn Widget> {
    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(panel::caption(title))];
    for device in devices {
        rows.push(boxed(device_row(device)));
    }
    if let (true, Some(note)) = (devices.is_empty(), empty) {
        rows.push(boxed(panel::caption(note)));
    }

    boxed(Column::new(rows).gap(sys::space::gap::xs()).width(Parent))
}

fn device_row(device: &BluetoothDevice) -> Rectangle {
    let mut trailing: Vec<Box<dyn Widget>> = Vec::new();

    if let Some(label) = pending_label(device) {
        trailing.push(boxed(panel::caption(label)));
    } else {
        if let Some(percent) = device.battery() {
            let color = if percent <= 15 {
                sys::color::status::critical::bg()
            } else {
                sys::color::content::secondary()
            };
            trailing.push(boxed(
                Text::new(format!("{percent}%"))
                    .style(sys::r#type::caption())
                    .color(color),
            ));
        }
        if device.connected() {
            trailing.push(panel::check());
        }
    }

    let clicked = device.clone();

    panel::list_row(
        device_glyph(device.icon()),
        device.name(),
        trailing,
        device.connected(),
    )
    .cursor(Pointer)
    .on_click(move |button| match button {
        Button::Left => act(&clicked),
        // only a paired device has anything to forget
        Button::Right if clicked.paired() => open_device_menu(&clicked),
        _ => {}
    })
}

/// The glyph for BlueZ's freedesktop icon name (`audio-headset`, `input-mouse` …).
fn device_glyph(icon: &str) -> char {
    match icon {
        i if i.starts_with("audio-head") => ICON_HEADPHONES,
        i if i.starts_with("audio") => ICON_SPEAKER,
        "input-mouse" | "input-tablet" => ICON_MOUSE,
        "input-keyboard" => ICON_KEYBOARD,
        "input-gaming" => ICON_GAMEPAD,
        "phone" => ICON_PHONE,
        "computer" => ICON_COMPUTER,
        _ => ICON_DEVICE_OTHER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_glyph_follows_the_bluez_icon() {
        assert_eq!(device_glyph("audio-headset"), ICON_HEADPHONES);
        assert_eq!(device_glyph("audio-headphones"), ICON_HEADPHONES);
        assert_eq!(device_glyph("audio-card"), ICON_SPEAKER);
        assert_eq!(device_glyph("input-mouse"), ICON_MOUSE);
        assert_eq!(device_glyph("phone"), ICON_PHONE);
        assert_eq!(device_glyph(""), ICON_DEVICE_OTHER);
    }
}
