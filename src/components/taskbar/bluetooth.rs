//! Bluetooth panel — a bottom-right `LayerWindow` to turn Bluetooth on and
//! off and to connect, pair and forget devices.
//!
//! One of the bar's [`panel`](super::panel)s, toggled by the Bluetooth
//! indicator. While it is open the adapter searches for new devices, like the
//! macOS menu, and stops again when it closes. Read from the fork's
//! `Bluetooth` service (BlueZ). Pairing only works for devices that need no
//! PIN: the fork has no BlueZ agent to show or type one.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use amane::{
    Bluetooth, BluetoothDevice, Column, LayerWindow, Parent, Rectangle, ScrollArea, Service, Size,
    Widget, mark_all,
};

use super::battery_color;
use super::panel::{self, Panel};
use crate::components::app_bar::TopAppBar;
use crate::components::buttons::Switch;
use crate::components::context_menu::{self, ContextMenu, MenuItem};
use crate::components::icons::{ICON_BLUETOOTH, device_glyph};
use crate::components::layout::{boxed, caption, cursor};
use crate::components::list::{self, List, ListItem};
use crate::components::motion::redraw_after;
use crate::tokens::sys;

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

    // the surface the device list is on, which its menu hangs under
    static MENU_PARENT: Cell<&'static str> = const { Cell::new(BLUETOOTH_NS) };

    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Stops the search the panel started; the panel calls this as it closes.
pub(crate) fn closed() {
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
    redraw_after(PENDING_FOR);
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

    context_menu::show_under(DEVICE_MENU_KEY, menu, MENU_PARENT.get(), cursor());
}

// ───── view ──────────────────────────────────────────────────────────────────

pub fn panel() -> LayerWindow {
    panel::surface(BLUETOOTH_NS, Panel::Bluetooth, card)
}

fn card() -> Rectangle {
    let powered = Bluetooth::read().powered();

    let header = TopAppBar::new("Bluetooth")
        .navigation_icon(ICON_BLUETOOTH)
        .actions(vec![boxed(
            Switch::new("amane.bluetooth.powered", powered)
                .on_toggle(Bluetooth::set_powered)
                .build(),
        )])
        .build();

    panel::card(
        vec![boxed(header), devices(BLUETOOTH_NS)],
        sys::space::gap::l(),
    )
}

/// The devices, searching for new ones while shown and the radio is on:
/// the panel's body, which Quick Settings' Bluetooth page shows too. A
/// device's menu hangs under `parent_ns`, the surface it's drawn on.
pub(crate) fn devices(parent_ns: &'static str) -> Box<dyn Widget> {
    MENU_PARENT.set(parent_ns);

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
        return boxed(caption("Bluetooth is off"));
    }
    if !SEARCHING.replace(true) {
        Bluetooth::start_scan();
    }

    device_list(mine, others, scanning)
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

    let column = Column::new(sections)
        .gap(sys::space::gap::xl())
        .width(Parent);

    boxed(
        ScrollArea::new("amane.bluetooth.devices", column)
            .height(Size::Fixed(sys::size::list::max_height())),
    )
}

fn device_section(title: &str, devices: &[BluetoothDevice]) -> List {
    List::new(devices.iter().map(device_item).collect()).header(title)
}

fn device_item(device: &BluetoothDevice) -> ListItem {
    let mut item = ListItem::new(
        format!("amane.bluetooth.device.{}", device.path()),
        device.name(),
    )
    .leading(device_glyph(device.icon()))
    .selected(device.connected());

    if let Some(label) = pending_label(device) {
        item = item.trailing(caption(label));
    } else {
        if let Some(percent) = device.battery() {
            // a caption, smaller than the bar's numeric percentages (manual §11.4)
            item = item.trailing(list::trailing_text(
                format!("{percent}%"),
                battery_color(percent, false, sys::color::content::secondary()),
            ));
        }
        if device.connected() {
            item = item.trailing(list::check());
        }
    }

    let pressed = device.clone();
    item = item.on_press(move || act(&pressed));
    // only a paired device has anything to forget
    if device.paired() {
        let clicked = device.clone();
        item = item.on_secondary(move || open_device_menu(&clicked));
    }
    item
}
