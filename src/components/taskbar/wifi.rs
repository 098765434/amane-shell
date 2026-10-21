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

use super::theme::Theme;
use super::tokens::{Tokens, tokens};
use super::{ICON_CHECK, ICON_LOCK, ICON_REFRESH, ICON_WIFI, boxed, icon, pad};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const WIFI_NS: &str = "amane.taskbar.wifi";

/// TextInput identity for the password field (keys its text store).
const PW_ID: &str = "amane.wifi.password";

const CARD_PAD: f32 = 10.0;

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
    let t = tokens();
    let theme = Theme::auto();
    let open = is_open();

    let base = LayerWindow::new()
        .width(t.panel_w)
        .height(t.panel_h)
        .anchor_vertical(Vertical::Bottom)
        .anchor_horizontal(Horizontal::Right)
        .margin(Margin {
            top: 0,
            right: t.panel_gap as i32,
            bottom: (t.height_compact + t.panel_gap) as i32,
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

    base.child(card(theme, t))
}

fn card(theme: Theme, t: &Tokens) -> Rectangle {
    let body = Column::new(vec![
        boxed(header(theme, t)),
        boxed(ap_list(theme, t)),
        password_section(theme, t),
    ])
    .gap(10.0)
    .width(Parent);

    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .radius(t.radius_tile)
        .fill(theme.surface)
        .blur(t.blur)
        .padding(pad(CARD_PAD, CARD_PAD))
        .child(body)
}

fn header(theme: Theme, t: &Tokens) -> Row {
    let left = Row::new(vec![
        boxed(icon(ICON_WIFI, t.icon_size, theme.fg_strong, t)),
        boxed(
            Text::new("Wi-Fi")
                .size(t.label_size)
                .color(theme.fg_strong)
                .font(&t.font_family)
                .weight(t.weight_medium),
        ),
    ])
    .gap(6.0)
    .align(Center);

    let right = Row::new(vec![
        boxed(wifi_toggle(theme, t)),
        boxed(scan_button(theme, t)),
    ])
    .gap(6.0)
    .align(Center);

    Row::new(vec![boxed(left), boxed(right)])
        .width(Parent)
        .justify(SpaceBetween)
        .align(Center)
}

fn wifi_toggle(theme: Theme, t: &Tokens) -> Rectangle {
    let enabled = Network::read().wifi_enabled();
    let (label, color) = if enabled {
        ("On", theme.accent)
    } else {
        ("Off", theme.fg)
    };

    Rectangle::new()
        .width(Size::Fixed(44.0))
        .height(Size::Fixed(22.0))
        .radius(11.0)
        .fill(theme.pill)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(move |button| {
            if button == Button::Left {
                Network::set_wifi(!enabled);
            }
        })
        .child(
            Text::new(label)
                .size(t.window_size)
                .color(color)
                .font(&t.font_family),
        )
}

fn scan_button(theme: Theme, t: &Tokens) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(28.0))
        .height(Size::Fixed(22.0))
        .radius(t.radius_pill)
        .fill(theme.pill)
        .align_child(Center, Center)
        .cursor(Pointer)
        .on_click(|button| {
            if button == Button::Left {
                Network::scan();
            }
        })
        .child(icon(ICON_REFRESH, t.icon_size, theme.fg_strong, t))
}

fn ap_list(theme: Theme, t: &Tokens) -> ScrollArea {
    let net = Network::read();
    let rows: Vec<Box<dyn Widget>> = net
        .access_points()
        .iter()
        .map(|ap| boxed(ap_row(ap, theme, t)))
        .collect();
    drop(net);

    let column = Column::new(rows).gap(4.0).width(Parent);

    ScrollArea::new("amane.wifi.aps", column).height(Size::Fixed(t.list_h))
}

fn ap_row(ap: &AccessPoint, theme: Theme, t: &Tokens) -> Rectangle {
    let ssid = ap.ssid().to_string();
    let secured = ap.secured();
    let active = ap.active();
    let saved = ap.saved();

    let ssid_color = if active { theme.fg_strong } else { theme.fg };
    let ssid_w = (t.panel_w - 120.0).max(1.0);

    let left = Row::new(vec![
        boxed(icon(ICON_WIFI, t.icon_size, ssid_color, t)),
        boxed(
            Rectangle::new()
                .width(Size::Fixed(ssid_w))
                .height(Parent)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Center)
                .clip()
                .child(
                    Text::new(ssid.clone())
                        .size(t.label_size)
                        .color(ssid_color)
                        .font(&t.font_family)
                        .elide(),
                ),
        ),
    ])
    .gap(8.0)
    .align(Center);

    let mut right: Vec<Box<dyn Widget>> = Vec::new();
    if secured {
        right.push(boxed(icon(ICON_LOCK, t.window_size, theme.fg, t)));
    }
    if active {
        right.push(boxed(icon(ICON_CHECK, t.icon_size, theme.accent, t)));
    }
    let right = Row::new(right).gap(6.0).align(Center);

    let content = Row::new(vec![boxed(left), boxed(right)])
        .width(Parent)
        .justify(SpaceBetween)
        .align(Center);

    let fill = if active {
        theme.pill_focused
    } else {
        theme.pill
    };

    Rectangle::new()
        .width(Parent)
        .height(Size::Fixed(t.row_h))
        .radius(t.radius_pill)
        .fill(fill)
        .padding(pad(10.0, 0.0))
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

fn password_section(theme: Theme, t: &Tokens) -> Box<dyn Widget> {
    let Some(ssid) = password_target() else {
        return boxed(Row::new(Vec::new()));
    };

    let inner = t.panel_w - CARD_PAD * 2.0;
    let connect_w = 72.0;
    let input_w = (inner - connect_w - 6.0).max(1.0);

    let submit_ssid = ssid.clone();
    let input = TextInput::new(PW_ID)
        .password()
        .focused()
        .placeholder("Password")
        .size(t.label_size)
        .color(theme.fg_strong)
        .width(Size::Fixed(input_w))
        .on_change(set_password_buf)
        .on_submit(move |pw| {
            Network::connect(&submit_ssid, Some(&pw));
            close();
        });

    let connect_ssid = ssid.clone();
    let connect = Rectangle::new()
        .width(Size::Fixed(connect_w))
        .height(Size::Fixed(t.row_h - 10.0))
        .radius(t.radius_pill)
        .fill(theme.accent)
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
                .size(t.window_size)
                .color(theme.fg_strong)
                .font(&t.font_family),
        );

    let field = Row::new(vec![boxed(input), boxed(connect)])
        .gap(6.0)
        .width(Parent)
        .align(Center);

    boxed(
        Column::new(vec![
            boxed(
                Text::new(format!("Password for {ssid}"))
                    .size(t.window_size)
                    .color(theme.fg)
                    .font(&t.font_family),
            ),
            boxed(field),
        ])
        .gap(6.0)
        .width(Parent),
    )
}
