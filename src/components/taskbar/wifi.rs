//! WiFi panel — a bottom-right `LayerWindow` to scan for and join networks.
//!
//! It is a separate surface (not a popup) with `Keyboard::OnDemand`, so the
//! password `TextInput` receives keystrokes once clicked into, while the bar
//! itself stays `Keyboard::None` and never steals focus. Toggled by the bar's
//! network indicator; dismissed with Escape or by toggling again.

use std::cell::RefCell;

use amane::{
    AccessPoint, Button, Center, Color, Column, Horizontal, Key, Keyboard, Layer, LayerWindow,
    Margin, Network, Parent, Pointer, Rectangle, Row, ScrollArea, Service, Size, SpaceBetween,
    Start, Text, TextInput, Vertical, Widget, Zone, mark_all,
};

use super::{ICON_CHECK, ICON_LOCK, ICON_REFRESH, ICON_WIFI, boxed, icon, pad};
use crate::tokens::{Styled, sys, wifi};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const WIFI_NS: &str = "amane.taskbar.wifi";

/// TextInput identity for the password field (keys its text store).
const PW_ID: &str = "amane.wifi.password";

#[derive(Default)]
struct State {
    open: bool,
    // SSID currently in password-entry mode, if any
    password_for: Option<String>,
    // live contents of the password field (TextInput has no getter)
    password_buf: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// ───── runtime ───────────────────────────────────────────────────────────────

pub fn toggle() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.open = !s.open;
        if !s.open {
            reset(&mut s);
        }
    });
    mark_all();
}

pub fn close() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.open = false;
        reset(&mut s);
    });
    mark_all();
}

fn reset(s: &mut State) {
    s.password_for = None;
    s.password_buf.clear();
    TextInput::set_text(PW_ID, "");
}

fn is_open() -> bool {
    STATE.with(|s| s.borrow().open)
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
    let open = is_open();

    let base = LayerWindow::new()
        .width(sys::size::popover::width())
        .height(sys::size::popover::height())
        .anchor_vertical(Vertical::Bottom)
        .anchor_horizontal(Horizontal::Right)
        .margin(Margin {
            top: 0,
            right: sys::space::gap::m() as i32,
            bottom: (sys::size::panel::height() + sys::space::gap::m()) as i32,
            left: 0,
        })
        .layer(Layer::Overlay)
        .space(Zone::Respect)
        .keyboard(Keyboard::OnDemand)
        .namespace(WIFI_NS)
        .visible(open)
        .on_key(|key| {
            if matches!(key, Key::Escape) {
                close();
            }
        });

    if !open {
        return base.child(
            Rectangle::new()
                .width(Parent)
                .height(Parent)
                .fill(Color::TRANSPARENT),
        );
    }

    base.child(card())
}

fn card() -> Rectangle {
    let body = Column::new(vec![boxed(header()), boxed(ap_list()), password_section()])
        .gap(sys::space::gap::l())
        .width(Parent);

    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(sys::radius::popover())
        .fill(sys::color::surface::panel::bg())
        .blur(sys::effect::backdrop_blur())
        .padding(pad(sys::space::inset::l(), sys::space::inset::l()))
        .child(body)
}

fn header() -> Row {
    let left = Row::new(vec![
        boxed(icon(
            ICON_WIFI,
            sys::size::icon::m(),
            sys::color::surface::panel::fg(),
        )),
        boxed(
            Text::new("Wi-Fi")
                .style(sys::r#type::title())
                .color(sys::color::surface::panel::fg()),
        ),
    ])
    .gap(sys::space::gap::s())
    .align(Center);

    let right = Row::new(vec![boxed(wifi_toggle()), boxed(scan_button())])
        .gap(sys::space::gap::s())
        .align(Center);

    Row::new(vec![boxed(left), boxed(right)])
        .width(Parent)
        .justify(SpaceBetween)
        .align(Center)
}

fn wifi_toggle() -> Rectangle {
    let enabled = Network::read().wifi_enabled();
    let (label, color) = if enabled {
        ("On", sys::color::accent::bg())
    } else {
        ("Off", sys::color::content::secondary())
    };

    Rectangle::new()
        .width(Size::Fixed(wifi::switch::width()))
        .height(Size::Fixed(wifi::switch::height()))
        .radius(wifi::switch::height() / 2.0)
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                Network::set_wifi(!enabled);
            }
        })
        .child(Text::new(label).style(sys::r#type::caption()).color(color))
}

fn scan_button() -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(wifi::icon_button::width()))
        .height(Size::Fixed(wifi::switch::height()))
        .radius(sys::radius::raised())
        .fill(sys::color::surface::raised::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(|button| {
            if button == Button::Left {
                Network::scan();
            }
        })
        .child(icon(
            ICON_REFRESH,
            sys::size::icon::m(),
            sys::color::surface::panel::fg(),
        ))
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

    let ssid_color = if active {
        sys::color::surface::panel::fg()
    } else {
        sys::color::content::secondary()
    };
    let ssid_w = (sys::size::popover::width() - sys::size::row::trailing_width()).max(1.0);

    let left = Row::new(vec![
        boxed(icon(ICON_WIFI, sys::size::icon::m(), ssid_color)),
        boxed(
            Rectangle::new()
                .width(Size::Fixed(ssid_w))
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .clip()
                .child(
                    Text::new(ssid.clone())
                        .style(sys::r#type::label())
                        .color(ssid_color)
                        .elide(),
                ),
        ),
    ])
    .gap(sys::space::gap::m())
    .align(Center);

    let mut right: Vec<Box<dyn Widget>> = Vec::new();
    if secured {
        right.push(boxed(icon(
            ICON_LOCK,
            sys::size::icon::s(),
            sys::color::content::secondary(),
        )));
    }
    if active {
        right.push(boxed(icon(
            ICON_CHECK,
            sys::size::icon::m(),
            sys::color::accent::bg(),
        )));
    }
    let right = Row::new(right).gap(sys::space::gap::s()).align(Center);

    let content = Row::new(vec![boxed(left), boxed(right)])
        .width(Parent)
        .justify(SpaceBetween)
        .align(Center);

    let fill = if active {
        sys::color::state::selected()
    } else {
        sys::color::surface::raised::bg()
    };

    Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(sys::size::row::height()))
        .radius(sys::radius::raised())
        .fill(fill)
        .padding(pad(sys::space::inset::l(), 0.0))
        .align_child(Center, Center)
        .clip()
        .cursor(Pointer)
        .on_click(move |button| {
            if button != Button::Left {
                return;
            }
            if secured && !saved {
                begin_password(ssid.clone());
            } else {
                Network::connect(&ssid, None);
                close();
            }
        })
        .child(content)
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
            close();
        });

    let connect_ssid = ssid.clone();
    let connect = Rectangle::new()
        .width(Size::Fixed(connect_w))
        .height(Size::Fixed(sys::size::control::height()))
        .radius(sys::radius::raised())
        .fill(sys::color::accent::bg())
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                Network::connect(&connect_ssid, Some(&password_buf()));
                close();
            }
        })
        .child(
            Text::new("Connect")
                .style(sys::r#type::caption())
                .color(sys::color::surface::panel::fg()),
        );

    let field = Row::new(vec![boxed(input), boxed(connect)])
        .gap(sys::space::gap::s())
        .width(Parent)
        .align(Center);

    boxed(
        Column::new(vec![
            boxed(
                Text::new(format!("Password for {ssid}"))
                    .style(sys::r#type::caption())
                    .color(sys::color::content::secondary()),
            ),
            boxed(field),
        ])
        .gap(sys::space::gap::s())
        .width(Parent),
    )
}
