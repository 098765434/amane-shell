//! Bluetooth — GNOME's toggle (hidden without an adapter; it names the
//! connected device, or how many) as a tile, and its page, a whole
//! Bluetooth manager: "My Devices" and "Other Devices",
//! searching for new ones and visible to them while shown. A click
//! connects, disconnects or pairs; a row says what it's doing, or why it
//! failed. A paired device's ⓘ (or right click) opens its own page:
//! connect, battery, connect automatically, rename, forget.
//!
//! Pairing asks through the fork's BlueZ agent (`Bluetooth::serve_agent`):
//! a code to confirm, type or show sits under the list while the page is
//! open, and as an alert (`alert::bluetooth`) when it isn't.
//!
//! Read from the fork's `Bluetooth` service, which follows BlueZ's signals
//! and keeps each device's operation and failure.

mod device;
mod pairing;

use device::*;
use pairing::*;

use std::cell::{Cell, RefCell};

use amane::{
    Bluetooth, BluetoothDevice, BluetoothOperation as Operation, Center, Column, PairingAnswer,
    PairingKind, PairingRequest, Parent, PowerState, Row, ScrollArea, Service, Text, TextInput,
    Widget,
};

use super::super::{Page, QS_NS, open};
use super::connected_count;
use crate::chrome::battery;
use crate::services::network::rfkill::Rfkill;
use crate::surfaces::alert;
use crate::surfaces::quick_settings::QuickSettings;
use crate::surfaces::quick_settings::detail::{DetailPage, page_width};
use crate::surfaces::quick_settings::tile::QuickToggle;
use crate::surfaces::status_bar::Indicator;
use crate::tokens::{Styled, sys};
use emaki_kit::buttons::{IconButton, PushButton, Style, Switch};
use emaki_kit::control_size::ControlSize;
use emaki_kit::dialog::Dialog;
use emaki_kit::icons::{
    ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF, ICON_INFO, device_glyph,
};
use emaki_kit::layout::{boxed, caption, cursor, fixed_width, text_height};
use emaki_kit::list::{self, List, ListItem};
use emaki_kit::material::{self, Level};
use emaki_kit::menu::layer as context_menu;
use emaki_kit::menu::{ContextMenu, MenuItem};

/// Registry key for the rebuilt-per-device right-click menu.
const DEVICE_MENU_KEY: &str = "emaki.qs.bluetooth.device-menu";

/// The field a PIN or passkey is typed in.
const CODE_ID: &str = "emaki.qs.bluetooth.code";

/// The field a device is renamed in.
const NAME_ID: &str = "emaki.qs.bluetooth.name";

/// A PIN is at most 16 characters; a passkey is six digits.
const PIN_MAX: usize = 16;
const PASSKEY_MAX: usize = 6;

thread_local! {
    // discovery was started for this showing of the page
    static SEARCHING: Cell<bool> = const { Cell::new(false) };

    // this computer was made visible for this showing of the page
    static VISIBLE: Cell<bool> = const { Cell::new(false) };

    // the device whose own page is shown, by its path
    static DEVICE: RefCell<Option<String>> = const { RefCell::new(None) };

    // what's typed in the code field, as the field has no getter
    static CODE: RefCell<String> = const { RefCell::new(String::new()) };
}

pub(crate) fn toggle() -> Option<QuickToggle> {
    let bt = Bluetooth::read();
    if !bt.available() {
        return None;
    }
    let power = bt.power();
    let connected: Vec<String> = bt
        .devices()
        .iter()
        .filter(|device| device.connected())
        .map(|device| device.name().to_string())
        .collect();
    drop(bt);

    let (glyph, subtitle) = match (power, connected.as_slice()) {
        (PowerState::On, []) => (ICON_BLUETOOTH, String::from("On")),
        (PowerState::On, [one]) => (ICON_BLUETOOTH_CONNECTED, one.clone()),
        (PowerState::On, many) => (ICON_BLUETOOTH_CONNECTED, connected_count(many.len())),
        (power, _) => (ICON_BLUETOOTH_OFF, power_label(power).to_string()),
    };

    Some(
        QuickToggle::new("emaki.qs.bluetooth", glyph, "Bluetooth")
            .subtitle(subtitle)
            .checked(matches!(power, PowerState::On | PowerState::TurningOn))
            .on_toggle(Bluetooth::set_powered)
            .on_open(|| open(Page::Bluetooth)),
    )
}

/// The page: the device list, or one device's own page.
pub(crate) fn page() -> DetailPage {
    let shown = DEVICE.with_borrow(Clone::clone);
    if let Some(path) = shown {
        let device = Bluetooth::read().device(&path).cloned();
        match device {
            Some(device) => return device_page(device),
            // forgotten, or out of reach: back to the list
            None => DEVICE.set(None),
        }
    }

    let bt = Bluetooth::read();
    let power = bt.power();
    let connected = bt.devices().iter().filter(|d| d.connected()).count();
    drop(bt);

    let subtitle = match (power, connected) {
        (PowerState::On, 0) => String::from("On"),
        (PowerState::On, n) => connected_count(n),
        (power, _) => power_label(power).to_string(),
    };
    DetailPage::new("emaki.qs.bluetooth.page", "Bluetooth")
        .subtitle(subtitle)
        .switch(
            matches!(power, PowerState::On | PowerState::TurningOn),
            Bluetooth::set_powered,
        )
        .status_bar(Indicator::Bluetooth)
        .body(devices)
}

/// The radio's state in a word or two, for a subtitle.
fn power_label(power: PowerState) -> &'static str {
    match power {
        PowerState::On => "On",
        PowerState::TurningOn => "Turning On…",
        PowerState::TurningOff => "Turning Off…",
        PowerState::Off | PowerState::Blocked => "Off",
    }
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// The page was left: the search and visibility it started end, its own
/// device page closes, and a pairing started from it that's still asking
/// is canceled (one the device started is left to the alert).
pub(crate) fn left() {
    if SEARCHING.replace(false) {
        Bluetooth::stop_scan();
    }
    if VISIBLE.replace(false) {
        Bluetooth::set_discoverable(false);
    }
    DEVICE.set(None);
    forget_code();

    let bt = Bluetooth::read();
    let started_here = bt.pairing_request().and_then(|request| {
        let ours = bt.operation(request.device()) == Some(Operation::Pairing);
        ours.then(|| (request.id(), request.device().to_string(), request.asks()))
    });
    drop(bt);
    if let Some((id, device, asks)) = started_here {
        if asks {
            Bluetooth::answer(id, PairingAnswer::Reject);
        }
        Bluetooth::cancel_pairing(&device);
    }
    // a device's own pairing asked here goes on in the alert
    alert::Alert::changed();
}

/// What a click on a device's row asks for.
fn act(device: &BluetoothDevice) {
    let path = device.path();
    if device.connected() {
        Bluetooth::disconnect(path);
    } else if device.paired() {
        Bluetooth::connect(path);
    } else {
        Bluetooth::pair(path);
    }
}

fn open_device(device: &BluetoothDevice) {
    TextInput::set_text(NAME_ID, device.name());
    DEVICE.set(Some(device.path().to_string()));
    QuickSettings::changed();
}

fn close_device() {
    DEVICE.set(None);
    QuickSettings::changed();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The devices, scrolling in `height`, searching for new ones and visible
/// to them while shown and the radio is on; a pairing being asked about
/// under them.
fn devices(height: f32) -> Vec<Box<dyn Widget>> {
    let bluetooth = Bluetooth::read();
    let power = bluetooth.power();
    let adapter_failure = bluetooth
        .adapter()
        .and_then(|adapter| super::failure::recent(bluetooth.failure(adapter.path())))
        .map(|failure| super::bluetooth_text::description(failure).to_string());
    let name = bluetooth
        .adapter()
        .map(|adapter| adapter.name().to_string())
        .unwrap_or_default();
    let request = bluetooth.pairing_request().cloned();
    let (mine, others): (Vec<BluetoothDevice>, Vec<BluetoothDevice>) = bluetooth
        .devices()
        .iter()
        .cloned()
        .partition(BluetoothDevice::paired);
    drop(bluetooth);

    // Started from the view rather than on open, so they also start when
    // the radio is switched on with the page already shown; the radio
    // going off ends both by itself.
    if power != PowerState::On {
        SEARCHING.set(false);
        VISIBLE.set(false);
        return vec![boxed(caption(off_note(power, adapter_failure)))];
    }
    // Only while this is the page shown: the view is also built while the
    // page slides out after Back, and a scan started then would outlive it.
    if super::super::showing(Page::Bluetooth) {
        if !SEARCHING.replace(true) {
            Bluetooth::start_scan();
        }
        if !VISIBLE.replace(true) {
            Bluetooth::set_discoverable(true);
        }
    }

    let sheet = request.map(|request| {
        let device = mine
            .iter()
            .chain(&others)
            .find(|device| device.path() == request.device())
            .map_or_else(|| String::from("the device"), |d| d.name().to_string());
        pairing_sheet(&request, &device)
    });
    let list_h = if sheet.is_some() {
        (height - sheet_height() - sys::space::gap::l()).max(1.0)
    } else {
        height.max(1.0)
    };

    let mut body = vec![device_list(mine, others, &name, list_h)];
    body.extend(sheet);
    body
}

/// Why there's no list: the radio is off, held off, or on its way.
fn off_note(power: PowerState, failure: Option<String>) -> String {
    if let Some(failure) = failure {
        return failure;
    }
    match power {
        PowerState::Blocked if Rfkill::read().hard_blocked("bluetooth") => {
            String::from("Turned off by a hardware switch")
        }
        PowerState::Blocked => String::from("Bluetooth is off for airplane mode"),
        PowerState::TurningOn => String::from("Turning on…"),
        _ => String::from("Bluetooth is off"),
    }
}

fn device_list(
    mine: Vec<BluetoothDevice>,
    others: Vec<BluetoothDevice>,
    name: &str,
    height: f32,
) -> Box<dyn Widget> {
    let bluetooth = Bluetooth::read();
    let scanning = bluetooth.scanning();
    let discoverable = bluetooth.discoverable();
    drop(bluetooth);

    let others = nearby(others);

    let mut sections: Vec<Box<dyn Widget>> = Vec::new();

    if !mine.is_empty() {
        sections.push(boxed(device_section("My Devices", &mine).build()));
    }

    let empty_note = if scanning {
        "Searching…"
    } else {
        "No devices found"
    };
    sections.push(boxed(
        device_section("Other Devices", &others)
            .empty(empty_note)
            .build(),
    ));

    if discoverable && !name.is_empty() {
        sections.push(boxed(caption(format!("Now discoverable as “{name}”"))));
    }

    let column = Column::new(sections)
        .gap(sys::space::gap::xl())
        .width(Parent);

    boxed(ScrollArea::new("emaki.qs.bluetooth.devices", column).height(height))
}

/// The devices worth offering to pair: named ones (a bare address means
/// nothing to anyone), the strongest signal first.
fn nearby(mut devices: Vec<BluetoothDevice>) -> Vec<BluetoothDevice> {
    devices.retain(BluetoothDevice::named);
    devices.sort_by(|a, b| {
        b.signal()
            .cmp(&a.signal())
            .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
    });
    devices
}

fn device_section(title: &str, devices: &[BluetoothDevice]) -> List {
    List::new(devices.iter().map(device_item).collect()).header(title)
}

/// What a row says about `device`'s operation, in place of its battery.
fn operation_label(operation: Operation) -> &'static str {
    match operation {
        Operation::Connecting => "Connecting…",
        Operation::Disconnecting => "Disconnecting…",
        Operation::Pairing => "Pairing…",
        Operation::Forgetting => "Forgetting…",
    }
}

fn device_item(device: &BluetoothDevice) -> ListItem {
    let bluetooth = Bluetooth::read();
    let operation = bluetooth.operation(device.path());
    let failure = super::failure::recent(bluetooth.failure(device.path()))
        .map(|failure| super::bluetooth_text::label(failure).to_string());
    drop(bluetooth);

    let mut item = ListItem::new(
        format!("emaki.qs.bluetooth.device.{}", device.path()),
        device.name(),
    )
    .leading(device_glyph(device.icon()))
    .selected(device.connected());

    if let Some(operation) = operation {
        item = item.trailing(caption(operation_label(operation)));
    } else if let Some(failure) = failure {
        item = item.trailing(list::trailing_text(
            failure,
            sys::color::status::critical::text(),
        ));
    } else {
        if let Some(percent) = device.battery() {
            // a caption, smaller than the bar's numeric percentages (manual §11.4)
            item = item.trailing(list::trailing_text(
                format!("{percent}%"),
                battery::color(percent, false, material::label(Level::Secondary)),
            ));
        }
        if device.connected() {
            item = item.trailing(list::check());
        }
    }

    let pressed = device.clone();
    item = item.on_press(move || act(&pressed));
    // only a paired device has a page and anything to forget
    if device.paired() {
        let details = device.clone();
        item = item.trailing(
            IconButton::new(
                format!("emaki.qs.bluetooth.device.{}.info", device.path()),
                ICON_INFO,
            )
            .size(sys::size::icon::s())
            .height(ControlSize::Medium.height())
            .width(ControlSize::Medium.height())
            .on_press(move || open_device(&details))
            .build(),
        );
        let clicked = device.clone();
        item = item.on_secondary(move || open_device_menu(&clicked));
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_passkey_keeps_its_leading_zeros() {
        assert_eq!(digits(42), "000042");
        assert_eq!(digits(123456), "123456");
    }

    #[test]
    fn bluez_icons_name_the_kind_of_device() {
        assert_eq!(kind_name("audio-headset"), "Headphones");
        assert_eq!(kind_name("input-keyboard"), "Keyboard");
        assert_eq!(kind_name(""), "Device");
    }

    #[test]
    fn every_operation_says_what_it_is_doing() {
        for operation in [
            Operation::Connecting,
            Operation::Disconnecting,
            Operation::Pairing,
            Operation::Forgetting,
        ] {
            assert!(operation_label(operation).ends_with('…'));
        }
    }
}
