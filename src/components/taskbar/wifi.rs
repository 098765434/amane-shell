//! WiFi panel — a bottom-right `LayerWindow` to scan for and join networks.
//!
//! One of the bar's [`panel`](super::panel)s, toggled by the network
//! indicator. The surface takes keyboard focus on demand, so the password
//! `TextInput` receives keystrokes once clicked into, while the bar itself
//! stays `Keyboard::None` and never steals focus.

use std::cell::RefCell;

use amane::{
    AccessPoint, Center, Column, LayerWindow, Network, Parent, Rectangle, Row, ScrollArea, Service,
    Size, TextInput, Widget, mark_all,
};

use super::panel::{self, Panel};
use crate::components::app_bar::TopAppBar;
use crate::components::buttons::{IconButton, PushButton, Shape, Style, Switch};
use crate::components::icons::{ICON_LOCK, ICON_REFRESH, ICON_WIFI, wifi_glyph};
use crate::components::layout::{boxed, caption, icon, inner_width, nothing, text_height};
use crate::components::list::{self, List, ListItem};
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
pub(crate) fn reset() {
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

/// Whether a password is being typed, which the networks below make room for.
pub(crate) fn asking_password() -> bool {
    password_target().is_some()
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
    let mut body = vec![boxed(header())];
    body.extend(networks_in(
        sys::size::list::max_height(),
        inner_width(sys::size::popover::width()),
        panel::close,
    ));
    panel::card(body, sys::space::gap::l())
}

/// How tall the password field is with the gap over it, which the list
/// above makes room for.
fn password_height() -> f32 {
    text_height(sys::r#type::caption()) + sys::space::gap::s() * 2.0 + sys::size::control::height()
}

/// The networks found and, when one is picked that needs it, the password
/// field: the panel's body, which Quick Settings' Wi‑Fi page shows too.
/// The list scrolls in `height` less the field's room, the field is
/// `width` wide, and `joined` runs once a network is asked to join (the
/// panel closes; a page stays to show it connecting).
pub(crate) fn networks_in(height: f32, width: f32, joined: fn()) -> Vec<Box<dyn Widget>> {
    let asking = asking_password();
    let list_h = if asking {
        (height - password_height()).max(1.0)
    } else {
        height.max(1.0)
    };
    vec![
        boxed(ap_list(list_h, joined)),
        password_section(width, joined),
    ]
}

fn header() -> Row {
    let enabled = Network::read().wifi_enabled();

    TopAppBar::new("Wi-Fi")
        .navigation_icon(ICON_WIFI)
        .actions(vec![
            boxed(
                Switch::new("amane.wifi.enabled", enabled)
                    .on_toggle(Network::set_wifi)
                    .build(),
            ),
            boxed(scan_button()),
        ])
        .build()
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

fn ap_list(height: f32, joined: fn()) -> ScrollArea {
    let net = Network::read();
    let enabled = net.wifi_enabled();
    let connecting = net.connecting().then(|| net.ssid().to_string());
    let items = net
        .access_points()
        .iter()
        .map(|ap| ap_item(ap, connecting.as_deref(), joined))
        .collect();
    drop(net);

    let note = if enabled {
        "Searching…"
    } else {
        "Wi‑Fi is off"
    };
    ScrollArea::new("amane.wifi.aps", List::new(items).empty(note).build())
        .height(Size::Fixed(height))
}

fn ap_item(ap: &AccessPoint, connecting: Option<&str>, joined: fn()) -> ListItem {
    let ssid = ap.ssid().to_string();
    let secured = ap.secured();
    let active = ap.active();
    let saved = ap.saved();

    let mut item = ListItem::new(format!("amane.wifi.ap.{ssid}"), ssid.clone())
        .leading(wifi_glyph(ap.strength()))
        .selected(active);
    if connecting == Some(ssid.as_str()) && !active {
        item = item.trailing(caption("Connecting…"));
    }
    if secured {
        item = item.trailing(icon(
            ICON_LOCK,
            sys::size::icon::s(),
            sys::color::content::secondary(),
        ));
    }
    if active {
        item = item.trailing(list::check());
    }

    item.on_press(move || {
        if secured && !saved {
            begin_password(ssid.clone());
        } else {
            Network::connect(&ssid, None);
            joined();
        }
    })
}

fn password_section(width: f32, joined: fn()) -> Box<dyn Widget> {
    let Some(ssid) = password_target() else {
        return nothing();
    };

    let inner = width;
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
            reset();
            joined();
        });

    let connect_ssid = ssid.clone();
    let connect = PushButton::new("amane.wifi.connect", "Connect")
        .style(Style::Filled)
        .width(connect_w)
        .on_press(move || {
            Network::connect(&connect_ssid, Some(&password_buf()));
            reset();
            joined();
        })
        .build();

    let field = Row::new(vec![boxed(input), boxed(connect)])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center);

    boxed(
        Column::new(vec![
            boxed(caption(format!("Password for {ssid}"))),
            boxed(field),
        ])
        .gap(sys::space::gap::s())
        .width(Parent),
    )
}
