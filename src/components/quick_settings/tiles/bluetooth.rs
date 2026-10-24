//! Bluetooth — GNOME's toggle (hidden without an adapter; it names the
//! connected device, or how many) and its page, the Bluetooth panel's
//! device lists, searching while shown.

use amane::{Bluetooth, Service};

use super::{GridCell, settings_link};
use crate::components::icons::{ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF};
use crate::components::layout::boxed;
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::QuickToggle;
use crate::components::quick_settings::{Page, QS_NS, open};
use crate::components::taskbar::bluetooth;
use crate::tokens::sys;

pub(crate) fn cell() -> Option<GridCell> {
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
        (true, many) => (
            ICON_BLUETOOTH_CONNECTED,
            format!("{} Connected", many.len()),
        ),
    };

    let tile = QuickToggle::new("amane.qs.bluetooth", glyph, "Bluetooth")
        .subtitle(subtitle)
        .checked(powered)
        .on_toggle(Bluetooth::set_powered)
        .on_open(|| open(Page::Bluetooth))
        .build();
    Some(Cell::Half(boxed(tile)))
}

pub(crate) fn page() -> DetailPage {
    let bt = Bluetooth::read();
    let powered = bt.powered();
    let connected = bt.devices().iter().filter(|d| d.connected()).count();
    drop(bt);

    let mut page = DetailPage::new("amane.qs.bluetooth.page", ICON_BLUETOOTH, "Bluetooth")
        .switch(powered, Bluetooth::set_powered)
        .body(
            vec![bluetooth::devices(QS_NS)],
            sys::size::list::max_height(),
        );
    if connected > 0 {
        page = page.subtitle(format!("{connected} Connected"));
    }
    settings_link(page, "Bluetooth Settings…", "blueman-manager")
}
