//! Phone — KDE Connect drawn as part of the shell: `kdeconnectd` runs on
//! its own and nothing of KDE's ever shows. The tile names the phone and
//! its charge; its page lists "My Devices" (paired, here or not) and the
//! phones on the network that could pair, with a phone's request to pair
//! under them (an alert asks instead while the page isn't showing,
//! `alert::phone`). A paired phone's page: battery, Ring, Send Files…,
//! Messages and Phone Input (in `amane-messages`), its Screen (in
//! `amane-mirror`, set up by `phone_screen`), the clipboard, the phone's
//! remote for this computer, and Forget.
//!
//! The phone's notifications and media need nothing here: the shell is the
//! notification server and Now Playing reads every MPRIS player, so they
//! show like any other.
//!
//! Read from the fork's `KdeConnect` service, which follows the daemon's
//! signals.

use std::cell::{Cell, RefCell};

use amane::{Center, Column, FileDialog, Parent, Row, ScrollArea, Service, Text, Widget};
use amane_phone::{KdeConnect, PhoneDevice, PhonePairState as PairState};

use super::super::{Page, open};
use crate::chrome::battery;
use crate::services::installed;
use crate::services::phone::MESSAGES_APP;
use crate::services::phone::open_messages as open_app;
use crate::surfaces::alert;
use crate::surfaces::quick_settings::QuickSettings;
use crate::surfaces::quick_settings::detail::{DetailPage, page_width};
use crate::surfaces::quick_settings::tile::QuickToggle;
use crate::surfaces::status_bar::Indicator;
use crate::tokens::{Styled, sys};
use amane_kit::buttons::{IconButton, PushButton, Style, Switch};
use amane_kit::control_size::ControlSize;
use amane_kit::dialog::Dialog;
use amane_kit::icons::{
    ICON_COMPUTER, ICON_CONTENT_PASTE, ICON_DEVICE_OTHER, ICON_INFO, ICON_KEYBOARD, ICON_PHONE,
};
use amane_kit::layout::{boxed, caption, fixed_width, text_height};
use amane_kit::list::{self, List, ListItem};
use amane_kit::material::{self, Level};

/// `ring_volume`, `upload_file` and `chat`, for the device page's rows.
const ICON_RING: char = '\u{e0d1}';
const ICON_SEND_FILE: char = '\u{e9fc}';
const ICON_CHAT: char = '\u{e0b7}';

thread_local! {
    // the phone whose own page is shown, by its id
    static DEVICE: RefCell<Option<String>> = const { RefCell::new(None) };

    // the network was searched again for this showing of the page
    static SEARCHED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) use crate::services::phone::available;

/// The glyph for a kind of device, as KDE Connect names them.
pub(crate) fn kind_glyph(kind: &str) -> char {
    match kind {
        "phone" | "smartphone" | "tablet" => ICON_PHONE,
        "desktop" | "laptop" => ICON_COMPUTER,
        _ => ICON_DEVICE_OTHER,
    }
}

/// "Pixel · 54%", or why there's no phone to name.
fn summary(kdeconnect: &KdeConnect) -> (bool, String) {
    if !kdeconnect.running() {
        return (false, String::from("Off"));
    }
    if let Some(phone) = kdeconnect.devices().iter().find(|d| d.connected()) {
        let text = match phone.battery() {
            Some(battery) => format!("{} · {}%", phone.name(), battery.percent),
            None => phone.name().to_string(),
        };
        return (true, text);
    }
    let text = if kdeconnect.paired().next().is_some() {
        "Not Connected"
    } else {
        "No Phone"
    };
    (false, String::from(text))
}

pub(crate) fn toggle() -> Option<QuickToggle> {
    if !available() {
        return None;
    }
    let (connected, subtitle) = summary(&KdeConnect::read());

    Some(
        QuickToggle::new("amane.qs.phone", ICON_PHONE, "Phone")
            .subtitle(subtitle)
            .checked(connected)
            .on_open(|| open(Page::Phone)),
    )
}

/// The page: the device list, or one phone's own page.
pub(crate) fn page() -> DetailPage {
    let shown = DEVICE.with_borrow(Clone::clone);
    if let Some(id) = shown {
        let device = KdeConnect::read().device(&id).cloned();
        match device {
            Some(device) if device.paired() => {
                return super::phone_screen::page(&device).unwrap_or_else(|| device_page(device));
            }
            // forgotten: back to the list
            _ => DEVICE.set(None),
        }
    }

    let (_, subtitle) = summary(&KdeConnect::read());
    DetailPage::new("amane.qs.phone.page", "Phone")
        .subtitle(subtitle)
        .status_bar(Indicator::Phone)
        .body(devices)
}

/// The page was left: its own device page closes, and the next showing
/// searches the network again.
pub(crate) fn left() {
    super::phone_screen::left();
    DEVICE.set(None);
    SEARCHED.set(false);
}

fn open_device(device: &PhoneDevice) {
    DEVICE.set(Some(device.id().to_string()));
    QuickSettings::changed();
}

fn close_device() {
    DEVICE.set(None);
    QuickSettings::changed();
}

/// Asks before forgetting `device`, which then has to pair again.
fn ask_forget(device: &PhoneDevice) {
    let id = device.id().to_string();
    let ask = Dialog::new(
        "amane.alert.phone.forget",
        format!("Forget “{}”?", device.name()),
    )
    .message("It will have to pair again before it can connect.")
    .button("Cancel", || {})
    .destructive("Forget Phone", move || {
        KdeConnect::unpair(&id);
        if DEVICE.with_borrow(|shown| shown.as_deref() == Some(id.as_str())) {
            close_device();
        }
    });
    alert::show(ask);
}

// ───── the list ──────────────────────────────────────────────────────────────

fn devices(height: f32) -> Vec<Box<dyn Widget>> {
    let kdeconnect = KdeConnect::read();
    if !kdeconnect.running() {
        drop(kdeconnect);
        return vec![off_note()];
    }
    let mine: Vec<PhoneDevice> = kdeconnect.paired().cloned().collect();
    let others: Vec<PhoneDevice> = kdeconnect
        .available()
        .filter(|device| device.pair_state() != PairState::RequestedByPeer)
        .cloned()
        .collect();
    let request = kdeconnect.pairing_request().cloned();
    drop(kdeconnect);

    // phones answer a fresh announcement at once, so the list fills quickly
    if !SEARCHED.replace(true) {
        KdeConnect::refresh();
    }

    let sheet = request.map(|request| pairing_sheet(&request));
    let list_h = if sheet.is_some() {
        (height - sheet_height() - sys::space::gap::l()).max(1.0)
    } else {
        height.max(1.0)
    };

    let mut sections: Vec<Box<dyn Widget>> = Vec::new();
    if !mine.is_empty() {
        sections.push(boxed(
            List::new(mine.iter().map(paired_item).collect())
                .header("My Devices")
                .build(),
        ));
    }
    sections.push(boxed(
        List::new(others.iter().map(other_item).collect())
            .header("Other Devices")
            .empty("Open KDE Connect on your phone, on the same network")
            .build(),
    ));

    let column = Column::new(sections)
        .gap(sys::space::gap::xl())
        .width(Parent);
    let mut body = vec![boxed(
        ScrollArea::new("amane.qs.phone.devices", column).height(list_h),
    )];
    body.extend(sheet);
    body
}

/// The daemon isn't running (stopped, or not started yet): a way to start it.
fn off_note() -> Box<dyn Widget> {
    let start = PushButton::new("amane.qs.phone.start", "Turn On")
        .style(Style::Tonal)
        .on_press(KdeConnect::start)
        .build();
    boxed(
        Column::new(vec![
            boxed(caption("KDE Connect isn't running")),
            boxed(start),
        ])
        .gap(sys::space::gap::s())
        .width(Parent),
    )
}

/// What a paired phone's row says on its right: its charge, or that it's away.
fn paired_trailing(device: &PhoneDevice) -> Text {
    match (device.reachable(), device.battery()) {
        (true, Some(charge)) => list::trailing_text(
            format!("{}%", charge.percent),
            battery::color(
                charge.percent,
                charge.charging,
                material::label(Level::Secondary),
            ),
        ),
        (true, None) => list::check(),
        (false, _) => caption("Not Connected"),
    }
}

fn paired_item(device: &PhoneDevice) -> ListItem {
    let id = device.id();
    let mut item = ListItem::new(format!("amane.qs.phone.device.{id}"), device.name())
        .leading(kind_glyph(device.kind()))
        .selected(device.connected())
        .trailing(paired_trailing(device));

    if let Some(failure) = super::failure::recent(KdeConnect::read().failure(id)) {
        item = item.trailing(list::trailing_text(
            failure.to_string(),
            sys::color::status::critical::text(),
        ));
    }

    let details = device.clone();
    let pressed = device.clone();
    item.trailing(
        IconButton::new(format!("amane.qs.phone.device.{id}.info"), ICON_INFO)
            .size(sys::size::icon::s())
            .height(ControlSize::Medium.height())
            .width(ControlSize::Medium.height())
            .on_press(move || open_device(&details))
            .build(),
    )
    .on_press(move || open_device(&pressed))
}

/// A phone that could pair: a click asks it, and the row then shows the
/// code both screens show until the phone answers.
fn other_item(device: &PhoneDevice) -> ListItem {
    let id = device.id().to_string();
    let mut item = ListItem::new(format!("amane.qs.phone.other.{id}"), device.name())
        .leading(kind_glyph(device.kind()));

    if device.pair_state() == PairState::Requested {
        let waiting = match device.verification_key() {
            "" => String::from("Waiting…"),
            key => format!("Code {key}"),
        };
        let canceled = id.clone();
        return item
            .trailing(list::trailing_text(
                waiting,
                material::label(Level::Secondary),
            ))
            .on_press(move || KdeConnect::cancel_pairing(&canceled));
    }

    if let Some(failure) = super::failure::recent(KdeConnect::read().failure(&id)) {
        item = item.trailing(list::trailing_text(
            failure.to_string(),
            sys::color::status::critical::text(),
        ));
    }
    item.on_press(move || KdeConnect::request_pair(&id))
}

// ───── a phone asking to pair ────────────────────────────────────────────────

fn sheet_height() -> f32 {
    text_height(sys::r#type::caption()) + sys::space::gap::s() + sys::size::control::height()
}

/// The phone's request, with the code to compare and Decline / Pair.
fn pairing_sheet(device: &PhoneDevice) -> Box<dyn Widget> {
    let id = device.id().to_string();
    let declined = id.clone();
    let decline = PushButton::new("amane.qs.phone.pair.decline", "Decline")
        .style(Style::Tonal)
        .on_press(move || KdeConnect::cancel_pairing(&declined))
        .build();
    let accept = PushButton::new("amane.qs.phone.pair.accept", "Pair")
        .style(Style::Filled)
        .on_press(move || KdeConnect::accept_pairing(&id))
        .build();
    let buttons_w = fixed_width(&decline) + fixed_width(&accept) + sys::space::gap::s() * 2.0;
    let code_w = (page_width() - buttons_w).max(1.0);

    let code = Text::new(device.verification_key())
        .style(sys::r#type::title())
        .color(material::label(Level::Primary));
    let controls = Row::new(vec![
        boxed(Row::new(vec![boxed(code)]).width(code_w).align(Center)),
        boxed(decline),
        boxed(accept),
    ])
    .gap(sys::space::gap::s())
    .width(Parent)
    .align(Center);

    let question = if device.verification_key().is_empty() {
        format!("“{}” wants to pair", device.name())
    } else {
        format!("“{}” wants to pair. Does it show this code?", device.name())
    };
    boxed(
        Column::new(vec![boxed(caption(question)), boxed(controls)])
            .gap(sys::space::gap::s())
            .width(Parent),
    )
}

// ───── one phone ─────────────────────────────────────────────────────────────

fn device_page(device: PhoneDevice) -> DetailPage {
    let status = match (device.reachable(), device.battery()) {
        (true, Some(charge)) if charge.charging => {
            format!("Connected · {}%, charging", charge.percent)
        }
        (true, Some(charge)) => format!("Connected · {}%", charge.percent),
        (true, None) => String::from("Connected"),
        (false, _) => String::from("Not Connected"),
    };

    DetailPage::new("amane.qs.phone.device", device.name())
        .subtitle(status)
        .on_back(close_device)
        .body(move |height| device_body(&device, height))
}

fn device_body(device: &PhoneDevice, height: f32) -> Vec<Box<dyn Widget>> {
    let mut body: Vec<Box<dyn Widget>> = Vec::new();

    if let Some(failure) = super::failure::recent(KdeConnect::read().failure(device.id())) {
        body.push(boxed(
            Text::new(failure.to_string())
                .style(sys::r#type::caption())
                .color(sys::color::status::critical::text()),
        ));
    }

    if !device.reachable() {
        body.push(boxed(caption(
            "Not on this network. Open KDE Connect on the phone to connect.",
        )));
    }

    let actions = action_rows(device);
    if !actions.is_empty() {
        body.push(boxed(List::new(actions).build()));
    }

    let camera = super::phone_screen::camera_rows(device);
    if !camera.is_empty() {
        body.push(boxed(List::new(camera).header("Camera").build()));
        body.push(boxed(
            caption("The phone's camera shows as “Phone Camera” in video call apps.").wrap(),
        ));
    }

    let clipboard = clipboard_rows(device);
    if !clipboard.is_empty() {
        body.push(boxed(List::new(clipboard).header("Clipboard").build()));
    }

    if device.connected() && device.can_run_commands() {
        body.push(boxed(
            List::new(remote_rows(device))
                .header("Phone Remote")
                .build(),
        ));
        if device.run_commands() {
            let commands: Vec<&str> = KdeConnect::run_command_list()
                .iter()
                .map(|command| command.name)
                .collect();
            body.push(boxed(caption(format!(
                "In KDE Connect's Run Command on the phone: {}",
                commands.join(", ")
            ))));
        }
    }

    let forgotten = device.clone();
    body.push(boxed(
        List::new(vec![
            ListItem::new("amane.qs.phone.device.forget", "Forget This Phone…")
                .color(sys::color::status::critical::text())
                .on_press(move || ask_forget(&forgotten)),
        ])
        .build(),
    ));

    let column = Column::new(body).gap(sys::space::gap::l()).width(Parent);
    vec![boxed(
        ScrollArea::new("amane.qs.phone.device.page", column).height(height.max(1.0)),
    )]
}

/// Battery, Ring, Send Files…, Messages and Phone Input, as far as the
/// phone offers them.
fn action_rows(device: &PhoneDevice) -> Vec<ListItem> {
    let row =
        |part: &str, title: &str| ListItem::new(format!("amane.qs.phone.device.{part}"), title);
    let mut rows = Vec::new();
    if !device.connected() {
        return rows;
    }

    if let Some(charge) = device.battery() {
        let text = if charge.charging {
            format!("{}%, charging", charge.percent)
        } else {
            format!("{}%", charge.percent)
        };
        rows.push(row("battery", "Battery").trailing(list::trailing_text(
            text,
            battery::color(
                charge.percent,
                charge.charging,
                material::label(Level::Secondary),
            ),
        )));
    }

    let id = device.id().to_string();
    if device.can_ring() {
        let rung = id.clone();
        rows.push(
            row("ring", "Ring")
                .leading(ICON_RING)
                .trailing(caption("Even on silent"))
                .on_press(move || KdeConnect::ring(&rung)),
        );
    }
    if device.can_share() {
        let sent = id.clone();
        let name = device.name().to_string();
        rows.push(
            row("send", "Send Files…")
                .leading(ICON_SEND_FILE)
                .on_press(move || {
                    let to = sent.clone();
                    FileDialog::open(&format!("Send to {name}"))
                        .multiple(true)
                        .show(move |files| {
                            if !files.is_empty() {
                                KdeConnect::send_files(&to, files);
                            }
                        });
                }),
        );
    }
    if installed(MESSAGES_APP) {
        if device.can_message() {
            let messaged = id.clone();
            rows.push(
                row("messages", "Messages")
                    .leading(ICON_CHAT)
                    .trailing(list::chevron())
                    .on_press(move || open_app(&messaged, "messages")),
            );
        }
        if device.can_remote_input() {
            let typed = id.clone();
            rows.push(
                row("input", "Phone Input")
                    .leading(ICON_KEYBOARD)
                    .trailing(list::chevron())
                    .on_press(move || open_app(&typed, "input")),
            );
        }
    }
    rows.extend(super::phone_screen::row(device, list::chevron));
    rows
}

/// Automatic sync both ways, and sending this computer's clipboard now.
fn clipboard_rows(device: &PhoneDevice) -> Vec<ListItem> {
    let Some(auto) = device.clipboard_auto_sync() else {
        return Vec::new();
    };
    let id = device.id().to_string();
    let toggled = id.clone();
    let pressed = id.clone();
    vec![
        ListItem::new("amane.qs.phone.clipboard.auto", "Share Automatically")
            .trailing(
                Switch::new("amane.qs.phone.clipboard.auto.switch", auto)
                    .on_toggle(move |on| KdeConnect::set_clipboard_auto_sync(&toggled, on))
                    .build(),
            )
            .on_press(move || KdeConnect::set_clipboard_auto_sync(&pressed, !auto)),
        ListItem::new("amane.qs.phone.clipboard.send", "Send Clipboard to Phone")
            .leading(ICON_CONTENT_PASTE)
            .on_press(move || KdeConnect::send_clipboard(&id)),
    ]
}

/// The phone's own remote for this computer: lock, suspend, screenshot.
fn remote_rows(device: &PhoneDevice) -> Vec<ListItem> {
    let on = device.run_commands();
    let toggled = device.id().to_string();
    let pressed = toggled.clone();
    vec![
        ListItem::new("amane.qs.phone.remote", "Control This Computer")
            .trailing(
                Switch::new("amane.qs.phone.remote.switch", on)
                    .on_toggle(move |on| KdeConnect::set_run_commands(&toggled, on))
                    .build(),
            )
            .on_press(move || KdeConnect::set_run_commands(&pressed, !on)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_glyphs() {
        assert_eq!(kind_glyph("smartphone"), ICON_PHONE);
        assert_eq!(kind_glyph("phone"), ICON_PHONE);
        assert_eq!(kind_glyph("laptop"), ICON_COMPUTER);
        assert_eq!(kind_glyph(""), ICON_DEVICE_OTHER);
    }
}
