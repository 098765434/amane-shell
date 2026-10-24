//! Bluetooth — GNOME's toggle (hidden without an adapter; it names the
//! connected device, or how many) as a tile, and
//! its page, the Bluetooth panel's device lists, searching while shown.

use amane::{Bluetooth, Service};

use super::super::{Page, QS_NS, open};
use super::{connected_count, settings_link};
use crate::components::icons::{ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::taskbar::bluetooth;

pub(crate) fn toggle() -> Option<QuickToggle> {
    let bt = Bluetooth::read();
    if !bt.available() {
        return None;
    }
    let powered = bt.powered();
    let connected: Vec<String> = bt
        .devices()
        .iter()
        .filter(|device| device.connected())
        .map(|device| device.name().to_string())
        .collect();
    drop(bt);

    let (glyph, subtitle) = match (powered, connected.as_slice()) {
        (false, _) => (ICON_BLUETOOTH_OFF, String::from("Off")),
        (true, []) => (ICON_BLUETOOTH, String::from("On")),
        (true, [one]) => (ICON_BLUETOOTH_CONNECTED, one.clone()),
        (true, many) => (ICON_BLUETOOTH_CONNECTED, connected_count(many.len())),
    };

    Some(
        QuickToggle::new("amane.qs.bluetooth", glyph, "Bluetooth")
            .subtitle(subtitle)
            .checked(powered)
            .on_toggle(Bluetooth::set_powered)
            .on_open(|| open(Page::Bluetooth)),
    )
}

pub(crate) fn page() -> DetailPage {
    let bt = Bluetooth::read();
    let powered = bt.powered();
    let connected = bt.devices().iter().filter(|d| d.connected()).count();
    drop(bt);

    let subtitle = match (powered, connected) {
        (false, _) => String::from("Off"),
        (true, 0) => String::from("On"),
        (true, n) => connected_count(n),
    };
    let page = DetailPage::new("amane.qs.bluetooth.page", "Bluetooth")
        .subtitle(subtitle)
        .switch(powered, Bluetooth::set_powered)
        .body(|height| vec![bluetooth::devices_in(QS_NS, height)]);
    settings_link(page, "Bluetooth Settings…", "blueman-manager")
}
