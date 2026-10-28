//! Bluetooth — GNOME's toggle (hidden without an adapter; it names the
//! connected device, or how many) as a tile, and its page, a whole
//! Bluetooth manager like the macOS one: "My Devices" and "Other Devices",
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

use std::cell::{Cell, RefCell};

use amane::{
    Bluetooth, BluetoothDevice, BluetoothOperation as Operation, Center, Column, PairingAnswer,
    PairingKind, PairingRequest, Parent, PowerState, Row, ScrollArea, Service, Size, Text,
    TextInput, Widget,
};

use super::super::{Page, QS_NS, open};
use super::connected_count;
use crate::surfaces::alert;
use crate::components::battery;
use crate::components::buttons::{IconButton, PushButton, Style, Switch};
use crate::surfaces::context_menu::{self, ContextMenu, MenuItem};
use crate::components::control_size::ControlSize;
use crate::components::dialog::Dialog;
use crate::components::icons::{
    ICON_BLUETOOTH, ICON_BLUETOOTH_CONNECTED, ICON_BLUETOOTH_OFF, ICON_INFO, device_glyph,
};
use crate::components::layout::{boxed, caption, cursor, fixed_width, text_height};
use crate::components::list::{self, List, ListItem};
use crate::components::material::{self, Level};
use crate::surfaces::quick_settings::QuickSettings;
use crate::surfaces::quick_settings::detail::{DetailPage, page_width};
use crate::surfaces::quick_settings::tile::QuickToggle;
use crate::surfaces::status_bar::Indicator;
use crate::services::network::rfkill::Rfkill;
use crate::tokens::{Styled, sys};

/// Registry key for the rebuilt-per-device right-click menu.
const DEVICE_MENU_KEY: &str = "amane.qs.bluetooth.device-menu";

/// The field a PIN or passkey is typed in.
const CODE_ID: &str = "amane.qs.bluetooth.code";

/// The field a device is renamed in.
const NAME_ID: &str = "amane.qs.bluetooth.name";

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
        QuickToggle::new("amane.qs.bluetooth", glyph, "Bluetooth")
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
    DetailPage::new("amane.qs.bluetooth.page", "Bluetooth")
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

fn forget_code() {
    CODE.with_borrow_mut(String::clear);
    TextInput::set_text(CODE_ID, "");
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

/// Asks before forgetting `device`, which then has to pair again.
fn ask_forget(device: &BluetoothDevice) {
    let path = device.path().to_string();
    let ask = Dialog::new(
        "amane.alert.bluetooth.forget",
        format!("Forget “{}”?", device.name()),
    )
    .message("It will have to pair again before it can connect.")
    .button("Cancel", || {})
    .destructive("Forget Device", move || {
        Bluetooth::forget(&path);
        if DEVICE.with_borrow(|shown| shown.as_deref() == Some(path.as_str())) {
            close_device();
        }
    });
    alert::show(ask);
}

fn open_device_menu(device: &BluetoothDevice) {
    let connected = device.connected();
    let for_act = device.clone();
    let for_details = device.clone();
    let for_forget = device.clone();

    let menu = ContextMenu::new()
        .item(
            MenuItem::label(if connected { "Disconnect" } else { "Connect" })
                .on_select(move || act(&for_act)),
        )
        .item(MenuItem::label("Details…").on_select(move || open_device(&for_details)))
        .separator()
        .item(
            MenuItem::label("Forget This Device…")
                .destructive()
                .on_select(move || ask_forget(&for_forget)),
        );

    context_menu::show_under(DEVICE_MENU_KEY, menu, QS_NS, cursor());
}

/// Answers the pairing asked about with the code typed, if it's one that
/// can be sent: digits only for a passkey.
fn send_code(request: &PairingRequest) {
    let typed = CODE.with_borrow(|code| code.trim().to_string());
    let answer = match request.kind() {
        PairingKind::EnterPin if !typed.is_empty() => PairingAnswer::Pin(typed),
        PairingKind::EnterPasskey => match typed.parse::<u32>() {
            Ok(passkey) if typed.len() <= PASSKEY_MAX => PairingAnswer::Passkey(passkey),
            _ => return,
        },
        _ => return,
    };
    Bluetooth::answer(request.id(), answer);
    forget_code();
}

/// Says no to the request, and stops the pairing it belongs to.
fn refuse(request: &PairingRequest) {
    if request.asks() {
        Bluetooth::answer(request.id(), PairingAnswer::Reject);
    }
    Bluetooth::cancel_pairing(request.device());
    forget_code();
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
        .and_then(|adapter| bluetooth.failure(adapter.path()))
        .map(|failure| failure.description().to_string());
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
    if !SEARCHING.replace(true) {
        Bluetooth::start_scan();
    }
    if !VISIBLE.replace(true) {
        Bluetooth::set_discoverable(true);
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

    boxed(ScrollArea::new("amane.qs.bluetooth.devices", column).height(Size::Fixed(height)))
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
    let failure = bluetooth
        .failure(device.path())
        .map(|failure| failure.label().to_string());
    drop(bluetooth);

    let mut item = ListItem::new(
        format!("amane.qs.bluetooth.device.{}", device.path()),
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
                format!("amane.qs.bluetooth.device.{}.info", device.path()),
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

// ───── pairing ───────────────────────────────────────────────────────────────

/// How tall the pairing sheet is: a line saying what's asked over a row
/// of controls.
fn sheet_height() -> f32 {
    text_height(sys::r#type::caption()) + sys::space::gap::s() + sys::size::control::height()
}

/// What pairing `device` is asking: a code to confirm, type or show, or
/// a yes or no.
fn pairing_sheet(request: &PairingRequest, device: &str) -> Box<dyn Widget> {
    let (question, controls) = match request.kind() {
        PairingKind::Confirm(passkey) => (
            format!("Is this code showing on “{device}”?"),
            confirm_row(request, &digits(*passkey), "Pair"),
        ),
        PairingKind::EnterPin => (
            format!("Enter the PIN for “{device}” (often 0000)"),
            code_row(request, "PIN", PIN_MAX),
        ),
        PairingKind::EnterPasskey => (
            format!("Enter the code shown on “{device}”"),
            code_row(request, "Code", PASSKEY_MAX),
        ),
        PairingKind::DisplayPin(pin) => (
            format!("Type this on “{device}”, then Return"),
            shown_row(request, pin, 0),
        ),
        PairingKind::DisplayPasskey { passkey, entered } => (
            format!("Type this on “{device}”, then Return"),
            shown_row(request, &digits(*passkey), usize::from(*entered)),
        ),
        PairingKind::Authorize => (
            format!("“{device}” wants to pair"),
            confirm_row(request, "", "Pair"),
        ),
        PairingKind::AuthorizeService(_) => (
            format!("Allow “{device}” to connect?"),
            confirm_row(request, "", "Allow"),
        ),
    };

    boxed(
        Column::new(vec![boxed(caption(question)), controls])
            .gap(sys::space::gap::s())
            .width(Parent),
    )
}

/// A passkey as BlueZ means it: six digits, leading zeros kept.
fn digits(passkey: u32) -> String {
    format!("{passkey:06}")
}

/// The code (when there is one) and Cancel / `yes`.
fn confirm_row(request: &PairingRequest, code: &str, yes: &str) -> Box<dyn Widget> {
    let id = request.id();
    let refused = request.clone();
    let cancel = PushButton::new("amane.qs.bluetooth.pair.cancel", "Cancel")
        .style(Style::Tonal)
        .on_press(move || refuse(&refused))
        .build();
    let accept = PushButton::new("amane.qs.bluetooth.pair.accept", yes)
        .style(Style::Filled)
        .on_press(move || Bluetooth::answer(id, PairingAnswer::Accept))
        .build();
    let buttons_w = fixed_width(&cancel) + fixed_width(&accept) + sys::space::gap::s() * 2.0;
    let code_w = (page_width() - buttons_w).max(1.0);

    boxed(
        Row::new(vec![
            boxed(code_text(code, 0).width(Size::Fixed(code_w))),
            boxed(cancel),
            boxed(accept),
        ])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center),
    )
}

/// A field for the code and Pair, with Cancel.
fn code_row(request: &PairingRequest, placeholder: &str, max: usize) -> Box<dyn Widget> {
    let refused = request.clone();
    let cancel = PushButton::new("amane.qs.bluetooth.pair.cancel", "Cancel")
        .style(Style::Tonal)
        .on_press(move || refuse(&refused))
        .build();
    let sent = request.clone();
    let pair = PushButton::new("amane.qs.bluetooth.pair.send", "Pair")
        .style(Style::Filled)
        .on_press(move || send_code(&sent))
        .build();
    let buttons_w = fixed_width(&cancel) + fixed_width(&pair) + sys::space::gap::s() * 2.0;
    let input_w = (page_width() - buttons_w).max(1.0);

    let submitted = request.clone();
    let input = TextInput::new(CODE_ID)
        .focused()
        .placeholder(placeholder)
        .size(sys::r#type::label().size)
        .color(material::label(Level::Primary))
        .width(Size::Fixed(input_w))
        .on_change(move |text| {
            let text: String = text.chars().take(max).collect();
            CODE.set(text);
        })
        .on_submit(move |_| send_code(&submitted));

    boxed(
        Row::new(vec![boxed(input), boxed(cancel), boxed(pair)])
            .gap(sys::space::gap::s())
            .width(Parent)
            .align(Center),
    )
}

/// A code to type on the device, the digits typed so far in accent, and
/// Cancel.
fn shown_row(request: &PairingRequest, code: &str, entered: usize) -> Box<dyn Widget> {
    let refused = request.clone();
    let cancel = PushButton::new("amane.qs.bluetooth.pair.cancel", "Cancel")
        .style(Style::Tonal)
        .on_press(move || refuse(&refused))
        .build();
    let code_w = (page_width() - fixed_width(&cancel) - sys::space::gap::s()).max(1.0);

    boxed(
        Row::new(vec![
            boxed(code_text(code, entered).width(Size::Fixed(code_w))),
            boxed(cancel),
        ])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center),
    )
}

/// A code in large numerals, the first `entered` characters in accent:
/// plain text, so it's drawn as letters with the rest of the page.
fn code_text(code: &str, entered: usize) -> Row {
    let split = code
        .char_indices()
        .nth(entered)
        .map_or(code.len(), |(at, _)| at);
    let (typed, rest) = code.split_at(split);
    let style = sys::r#type::title();
    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if !typed.is_empty() {
        parts.push(boxed(
            Text::new(typed)
                .style(style)
                .color(sys::color::accent::bg()),
        ));
    }
    if !rest.is_empty() {
        parts.push(boxed(
            Text::new(rest)
                .style(style)
                .color(material::label(Level::Primary)),
        ));
    }
    Row::new(parts).align(Center)
}

// ───── one device ────────────────────────────────────────────────────────────

fn device_page(device: BluetoothDevice) -> DetailPage {
    let bluetooth = Bluetooth::read();
    let operation = bluetooth.operation(device.path());
    let failure = bluetooth
        .failure(device.path())
        .map(|failure| failure.description().to_string());
    drop(bluetooth);

    let status = match operation {
        Some(operation) => operation_label(operation).to_string(),
        None if device.connected() => String::from("Connected"),
        None => String::from("Not Connected"),
    };

    DetailPage::new("amane.qs.bluetooth.device", device.name())
        .subtitle(status)
        .on_back(close_device)
        .body(move |height| device_body(&device, operation, failure, height))
}

fn device_body(
    device: &BluetoothDevice,
    operation: Option<Operation>,
    failure: Option<String>,
    height: f32,
) -> Vec<Box<dyn Widget>> {
    let id = |part: &str| format!("amane.qs.bluetooth.device.page.{part}");
    let mut rows: Vec<ListItem> = Vec::new();

    let label = if device.connected() {
        "Disconnect"
    } else {
        "Connect"
    };
    let mut connect = ListItem::new(id("connect"), label).leading(device_glyph(device.icon()));
    if let Some(operation) = operation {
        connect = connect.trailing(caption(operation_label(operation)));
    }
    let pressed = device.clone();
    rows.push(connect.on_press(move || act(&pressed)));

    if let Some(percent) = device.battery() {
        rows.push(
            ListItem::new(id("battery"), "Battery").trailing(list::trailing_text(
                format!("{percent}%"),
                battery::color(percent, false, material::label(Level::Secondary)),
            )),
        );
    }

    let trusted = device.trusted();
    let path = device.path().to_string();
    let toggled = path.clone();
    rows.push(
        ListItem::new(id("trusted"), "Connect Automatically")
            .trailing(
                Switch::new(id("trusted.switch"), trusted)
                    .on_toggle(move |on| Bluetooth::set_trusted(&toggled, on))
                    .build(),
            )
            .on_press(move || Bluetooth::set_trusted(&path, !trusted)),
    );

    let forgotten = device.clone();
    rows.push(
        ListItem::new(id("forget"), "Forget This Device…")
            .color(sys::color::status::critical::text())
            .on_press(move || ask_forget(&forgotten)),
    );

    let mut body: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(failure) = failure {
        body.push(boxed(
            Text::new(failure)
                .style(sys::r#type::caption())
                .color(sys::color::status::critical::text()),
        ));
    }
    body.push(boxed(List::new(rows).build()));
    body.push(rename_row(device));
    body.push(boxed(caption(format!(
        "{} · {}",
        kind_name(device.icon()),
        device.address()
    ))));

    let column = Column::new(body).gap(sys::space::gap::l()).width(Parent);
    vec![boxed(
        ScrollArea::new("amane.qs.bluetooth.device.page", column)
            .height(Size::Fixed(height.max(1.0))),
    )]
}

/// The name this computer shows for the device, changed on Return.
fn rename_row(device: &BluetoothDevice) -> Box<dyn Widget> {
    let path = device.path().to_string();
    let input = TextInput::new(NAME_ID)
        .placeholder(device.name())
        .size(sys::r#type::label().size)
        .color(material::label(Level::Primary))
        .width(Size::Fixed(page_width()))
        .on_submit(move |name| {
            if !name.trim().is_empty() {
                Bluetooth::rename(&path, &name);
            }
        });

    boxed(
        Column::new(vec![boxed(caption("Name")), boxed(input)])
            .gap(sys::space::gap::s())
            .width(Parent),
    )
}

/// What kind of device a BlueZ icon name says it is.
fn kind_name(icon: &str) -> &'static str {
    match icon {
        i if i.starts_with("audio-head") => "Headphones",
        "audio-card" | "audio-speakers" => "Speaker",
        "input-keyboard" => "Keyboard",
        "input-mouse" => "Mouse",
        "input-gaming" => "Game Controller",
        "input-tablet" => "Tablet",
        "phone" => "Phone",
        "computer" => "Computer",
        "camera-photo" | "camera-video" => "Camera",
        "printer" => "Printer",
        _ => "Device",
    }
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
