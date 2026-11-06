//! Phone Input: this computer as the phone's trackpad and keyboard. Drag
//! on the pad to move the phone's pointer, tap to click, right-click for
//! its long press, scroll to scroll; the buttons under it click on their
//! own. Keys typed while this shows go to the field focused on the phone.
//!
//! The other way round, the phone driving this computer (KDE Connect's
//! touchpad and presenter), needs nothing here: KDE Connect does it itself.

use std::cell::Cell;

use amane::{
    Button, Center, Column, Cursor, KdeConnect, Key, PhoneClick, PhoneDevice, PhoneKey,
    PhoneModifiers, Point, Radius, Rectangle, Row, Size, Text,
};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::icons::ICON_KEYBOARD;
use amane_kit::layout::{boxed, caption, icon};
use amane_kit::macform::theme as mac;
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};

use crate::title;

/// The phone's pixels per pixel moved here: a phone screen is dense.
const SPEED: f32 = 2.0;

/// A press that moves less than this is a tap.
const TAP_SLOP: f32 = 4.0;

thread_local! {
    // where the last drag event was, none between drags
    static LAST: Cell<Option<Point>> = const { Cell::new(None) };

    // how far this drag has gone in all, to tell a tap from a drag
    static TRAVEL: Cell<f32> = const { Cell::new(0.0) };

    // movement under a whole phone pixel, kept for the next event
    static REST: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
}

pub fn view(device: &PhoneDevice) -> Column {
    if !device.can_remote_input() {
        return Column::new(vec![
            boxed(title("Phone Input")),
            boxed(caption(if device.reachable() {
                "Turn on Remote Input and Remote Keyboard for this computer in KDE Connect on the phone."
            } else {
                "The phone isn't on this network."
            })),
        ])
        .gap(sys::space::gap::s())
        .width(Size::Parent);
    }

    let id = device.id().to_string();
    let hint = if device.keyboard_ready() {
        "Typing here types on the phone."
    } else {
        "Tap a text field on the phone to type on it."
    };

    let heading: Box<dyn amane::Widget> = if crate::phone::screen_available(device) {
        let shown = id.clone();
        boxed(
            Row::new(vec![
                boxed(title("Phone Input")),
                boxed(
                    PushButton::new("amane.messages.input.screen", "Show Screen")
                        .style(Style::Tonal)
                        .on_press(move || {
                            amane::spawn_activated(&format!("amane-mirror --device {shown}"));
                        })
                        .build(),
                ),
            ])
            .justify(amane::SpaceBetween)
            .align(Center)
            .width(Size::Parent),
        )
    } else {
        boxed(title("Phone Input"))
    };

    Column::new(vec![
        heading,
        boxed(trackpad(id.clone())),
        boxed(buttons(id)),
        boxed(
            Row::new(vec![
                boxed(icon(
                    ICON_KEYBOARD,
                    sys::size::icon::s(),
                    if device.keyboard_ready() {
                        sys::color::accent::bg()
                    } else {
                        material::label(Level::Secondary)
                    },
                )),
                boxed(caption(hint)),
            ])
            .gap(sys::space::gap::s())
            .align(Center),
        ),
    ])
    .gap(sys::space::gap::l())
    .width(Size::Parent)
    .height(Size::Parent)
}

fn trackpad(id: String) -> Rectangle {
    let moved = id.clone();
    let released = id.clone();
    let clicked = id.clone();
    let scrolled = id;

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .radius(Radius::Fixed(mac::group_radius()))
        .fill(mac::group_fill())
        .border(1.0, mac::field_border())
        .cursor(Cursor::Default)
        .align_child(Center, Center)
        .child(
            Text::new("Drag to move · tap to click · scroll to scroll")
                .style(sys::r#type::caption())
                .color(material::label(Level::Tertiary)),
        )
        .on_drag(move |point| drag(&moved, point))
        .on_release(move |_| release(&released))
        .on_click(move |button| match button {
            Button::Right => KdeConnect::click(&clicked, PhoneClick::Right),
            Button::Middle => KdeConnect::click(&clicked, PhoneClick::Middle),
            _ => {}
        })
        .on_scroll(move |scroll| {
            let (dx, dy) = scroll.pixels();
            KdeConnect::scroll(&scrolled, dx.round() as i32, dy.round() as i32);
        })
}

fn drag(id: &str, point: Point) {
    let Some(last) = LAST.replace(Some(point)) else {
        // the press itself: nothing moved yet
        TRAVEL.set(0.0);
        REST.set((0.0, 0.0));
        return;
    };
    let (dx, dy) = (point.x - last.x, point.y - last.y);
    TRAVEL.set(TRAVEL.get() + dx.hypot(dy));

    let (rest_x, rest_y) = REST.get();
    let (x, y) = (dx * SPEED + rest_x, dy * SPEED + rest_y);
    let (whole_x, whole_y) = (x.trunc(), y.trunc());
    REST.set((x - whole_x, y - whole_y));
    KdeConnect::move_pointer(id, whole_x as i32, whole_y as i32);
}

fn release(id: &str) {
    LAST.set(None);
    if TRAVEL.replace(0.0) < TAP_SLOP {
        KdeConnect::click(id, PhoneClick::Left);
    }
}

fn buttons(id: String) -> Row {
    let button = |label: &'static str, click: PhoneClick| {
        let id = id.clone();
        boxed(
            PushButton::new(format!("messages.input.{label}"), label)
                .style(Style::Tonal)
                .on_press(move || KdeConnect::click(&id, click))
                .build(),
        )
    };
    Row::new(vec![
        button("Left", PhoneClick::Left),
        button("Middle", PhoneClick::Middle),
        button("Right", PhoneClick::Right),
    ])
    .gap(sys::space::gap::s())
    .justify(Center)
    .width(Size::Parent)
}

/// A key typed while Phone Input shows, to the phone.
pub fn key(key: Key) {
    let Some(device) = crate::device() else {
        return;
    };
    let id = device.id();
    let held = amane::modifiers();
    // Ctrl and the logo key are this computer's shortcuts
    if held.ctrl || held.logo {
        return;
    }
    let modifiers = PhoneModifiers {
        shift: held.shift,
        ctrl: false,
        alt: held.alt,
    };
    let named = match key {
        Key::Character(c) => {
            KdeConnect::type_text(id, &c.to_string(), modifiers);
            return;
        }
        Key::Space => {
            KdeConnect::type_text(id, " ", modifiers);
            return;
        }
        Key::Enter => PhoneKey::Enter,
        Key::Escape => PhoneKey::Escape,
        Key::Tab => PhoneKey::Tab,
        Key::Backspace => PhoneKey::Backspace,
        Key::Delete => PhoneKey::Delete,
        Key::Up => PhoneKey::Up,
        Key::Down => PhoneKey::Down,
        Key::Left => PhoneKey::Left,
        Key::Right => PhoneKey::Right,
        Key::Home => PhoneKey::Home,
        Key::End => PhoneKey::End,
        Key::PageUp => PhoneKey::PageUp,
        Key::PageDown => PhoneKey::PageDown,
        _ => return,
    };
    KdeConnect::press_key(id, named, modifiers);
}
