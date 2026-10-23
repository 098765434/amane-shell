//! WiFi panel — a bottom-right `LayerWindow` to scan for and join networks.
//!
//! One of the bar's [`panel`](super::panel)s, toggled by the network
//! indicator. The surface takes keyboard focus on demand, so the password
//! `TextInput` receives keystrokes once clicked into, while the bar itself
//! stays `Keyboard::None` and never steals focus.

use std::cell::RefCell;

use amane::{
    AccessPoint, Button, Center, Column, LayerWindow, Network, Parent, Pointer, Rectangle, Row,
    ScrollArea, Service, Size, TextInput, Widget, mark_all,
};

use super::panel::{self, Panel};
use super::{ICON_LOCK, ICON_REFRESH, ICON_WIFI, boxed, icon, wifi_glyph};
use crate::components::buttons::{IconButton, PushButton, Shape, Style, Switch};
use crate::tokens::{sys, wifi};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const WIFI_NS: &str = "amane.taskbar.wifi";

/// TextInput identity for the password field (keys its text store).
const PW_ID: &str = "amane.wifi.password";

#[derive(Default)]
struct State {
    // SSID currently in password-entry mode, if any
    password_for: Option<String>,
    // live contents of the password field (TextInput has no getter)
    password_buf: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Forgets a half-typed password; the panel calls this as it closes.
pub(super) fn reset() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.password_for = None;
        s.password_buf.clear();
    });
    TextInput::set_text(PW_ID, "");
}

fn begin_password(ssid: String) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.password_for = Some(ssid);
        s.password_buf.clear();
    });
    TextInput::set_text(PW_ID, "");
    mark_all();
}

fn password_target() -> Option<String> {
    STATE.with(|s| s.borrow().password_for.clone())
}

fn set_password_buf(text: String) {
    STATE.with(|s| s.borrow_mut().password_buf = text);
}

fn password_buf() -> String {
    STATE.with(|s| s.borrow().password_buf.clone())
}

// ───── view ──────────────────────────────────────────────────────────────────

pub fn panel() -> LayerWindow {
    panel::surface(WIFI_NS, Panel::Wifi, card)
}

fn card() -> Rectangle {
    panel::card(
        vec![boxed(header()), boxed(ap_list()), password_section()],
        sys::space::gap::l(),
    )
}

fn header() -> Row {
    let enabled = Network::read().wifi_enabled();

    panel::header(
        ICON_WIFI,
        "Wi-Fi",
        sys::color::surface::panel::fg(),
        vec![
            boxed(
                Switch::new("amane.wifi.enabled", enabled)
                    .on_toggle(Network::set_wifi)
                    .build(),
            ),
            boxed(scan_button()),
        ],
    )
}

fn scan_button() -> Rectangle {
    IconButton::new("amane.wifi.scan", ICON_REFRESH)
        .style(Style::Tonal)
        .shape(Shape::Square)
        .width(wifi::icon_button::width())
        .height(sys::size::switch::height())
        .on_press(Network::scan)
        .build()
}

fn ap_list() -> ScrollArea {
    let net = Network::read();
    let rows: Vec<Box<dyn Widget>> = net
        .access_points()
        .iter()
        .map(|ap| boxed(ap_row(ap)))
        .collect();
    drop(net);

    let column = Column::new(rows).gap(sys::space::gap::xs()).width(Parent);

    ScrollArea::new("amane.wifi.aps", column).height(Size::Fixed(sys::size::list::max_height()))
}

fn ap_row(ap: &AccessPoint) -> Rectangle {
    let ssid = ap.ssid().to_string();
    let secured = ap.secured();
    let active = ap.active();
    let saved = ap.saved();

    let mut trailing: Vec<Box<dyn Widget>> = Vec::new();
    if secured {
        trailing.push(boxed(icon(
            ICON_LOCK,
            sys::size::icon::s(),
            sys::color::content::secondary(),
        )));
    }
    if active {
        trailing.push(panel::check());
    }

    panel::list_row(wifi_glyph(ap.strength()), ssid.clone(), trailing, active)
        .cursor(Pointer)
        .on_click(move |button| {
            if button != Button::Left {
                return;
            }
            if secured && !saved {
                begin_password(ssid.clone());
            } else {
                Network::connect(&ssid, None);
                panel::close();
            }
        })
}

fn password_section() -> Box<dyn Widget> {
    let Some(ssid) = password_target() else {
        return boxed(Row::new(Vec::new()));
    };

    let inner = sys::size::popover::width() - sys::space::inset::l() * 2.0;
    let connect_w = wifi::button::width();
    let input_w = (inner - connect_w - sys::space::gap::s()).max(1.0);

    let submit_ssid = ssid.clone();
    let input = TextInput::new(PW_ID)
        .password()
        .focused()
        .placeholder("Password")
        .size(sys::r#type::label().size)
        .color(sys::color::surface::panel::fg())
        .width(Size::Fixed(input_w))
        .on_change(set_password_buf)
        .on_submit(move |pw| {
            Network::connect(&submit_ssid, Some(&pw));
            panel::close();
        });

    let connect_ssid = ssid.clone();
    let connect = PushButton::new("amane.wifi.connect", "Connect")
        .style(Style::Filled)
        .width(connect_w)
        .on_press(move || {
            Network::connect(&connect_ssid, Some(&password_buf()));
            panel::close();
        })
        .build();

    let field = Row::new(vec![boxed(input), boxed(connect)])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center);

    boxed(
        Column::new(vec![
            boxed(panel::caption(format!("Password for {ssid}"))),
            boxed(field),
        ])
        .gap(sys::space::gap::s())
        .width(Parent),
    )
}
