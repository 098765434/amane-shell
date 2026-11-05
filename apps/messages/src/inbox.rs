//! The inbox: the phone's conversations, newest first, each with its
//! contact's initials, name, latest message and when; an unread one in
//! bold with the accent's dot. Phone Input sits under them.

use std::rc::Rc;

use amane::{
    Center, Color, Column, PhoneDevice, PhoneMessages, Radius, Rectangle, Row, ScrollArea, Service,
    Size, SmsThread, Text, Widget,
};
use amane_kit::app::sidebar_label;
use amane_kit::buttons::{Actions, IconButton, interactive, state_fill};
use amane_kit::color::Accent;
use amane_kit::control_size::ControlSize;
use amane_kit::icons::ICON_KEYBOARD;
use amane_kit::layout::{boxed, caption, icon, pad, text_height};
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};

use crate::{Navigation, View, time};

/// `refresh` and `person`.
const ICON_REFRESH: char = '\u{e5d5}';
const ICON_PERSON: char = '\u{e7fd}';

const AVATAR: f32 = 36.0;

pub fn view(device: Option<&PhoneDevice>, view: View) -> Column {
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(header(device))];

    let Some(device) = device else {
        return Column::new(parts).width(Size::Parent).height(Size::Parent);
    };

    if device.can_message() {
        parts.push(boxed(conversations(view)));
    } else {
        parts.push(boxed(
            Column::new(vec![boxed(caption(if device.reachable() {
                "Messages aren't shared by this phone. Allow SMS in KDE Connect on it."
            } else {
                "The phone isn't on this network."
            }))])
            .width(Size::Parent)
            .height(Size::Parent),
        ));
    }

    if device.can_remote_input() {
        parts.push(boxed(input_row(view == View::Input)));
    }

    Column::new(parts)
        .gap(sys::space::gap::s())
        .width(Size::Parent)
        .height(Size::Parent)
}

/// The phone's name and a button asking it for its messages again: a
/// phone that was asleep sends nothing until asked.
fn header(device: Option<&PhoneDevice>) -> Row {
    let name = device.map_or("Messages", PhoneDevice::name).to_string();
    let refresh = IconButton::new("messages.refresh", ICON_REFRESH)
        .size(sys::size::icon::s())
        .height(ControlSize::Medium.height())
        .width(ControlSize::Medium.height())
        .on_press(PhoneMessages::refresh)
        .tooltip("main", "Check for New Messages")
        .build();
    Row::new(vec![
        boxed(
            Text::new(name)
                .style(sys::r#type::title())
                .color(sidebar_label())
                .elide(),
        ),
        boxed(refresh),
    ])
    .width(Size::Parent)
    .align(Center)
}

fn conversations(view: View) -> ScrollArea {
    let messages = PhoneMessages::read();
    let threads: Vec<SmsThread> = messages.threads().to_vec();
    drop(messages);

    let shown = match view {
        View::Thread(thread) => thread,
        View::Input => None,
    };

    let rows: Vec<Box<dyn Widget>> = if threads.is_empty() {
        vec![boxed(caption("Waiting for the phone…"))]
    } else {
        threads
            .iter()
            .map(|thread| boxed(row(thread, shown == Some(thread.id()))))
            .collect()
    };

    ScrollArea::new(
        "messages.inbox",
        Column::new(rows)
            .gap(sys::space::gap::_2xs())
            .width(Size::Parent),
    )
    .width(Size::Parent)
    .height(Size::Parent)
}

/// How tall a conversation's row is: its name over one line of message.
fn row_height() -> f32 {
    (text_height(sys::r#type::label()) + text_height(sys::r#type::caption()) + 14.0)
        .max(AVATAR + 12.0)
}

fn row(thread: &SmsThread, chosen: bool) -> Rectangle {
    let id = thread.id();
    let key = format!("messages.inbox.{id}");
    let unread = thread.unread();

    let latest = thread.latest();
    let snippet = match (latest.body().trim(), latest.attachments()) {
        ("", 0) => String::new(),
        ("", _) => String::from("Attachment"),
        (body, _) if latest.outgoing() => format!("You: {}", one_line(body)),
        (body, _) => one_line(body),
    };

    let (on_accent, label) = if chosen {
        (true, sys::color::accent::fg())
    } else {
        (false, sidebar_label())
    };
    let secondary = if on_accent {
        sys::color::accent::fg()
    } else {
        material::label(Level::Secondary)
    };

    let name = Text::new(thread.title().to_string())
        .style(sys::r#type::label())
        .weight(if unread { 700 } else { 500 })
        .color(label)
        .elide();
    let when = Text::new(time::short(latest.date()))
        .style(sys::r#type::caption())
        .color(if unread && !on_accent {
            sys::color::accent::bg()
        } else {
            secondary
        });
    let top = Row::new(vec![
        boxed(Column::new(vec![boxed(name)]).width(Size::Parent)),
        boxed(when),
    ])
    .gap(sys::space::gap::s())
    .width(Size::Parent)
    .align(Center);
    let bottom = Text::new(snippet)
        .style(sys::r#type::caption())
        .color(secondary)
        .elide();

    let fill = if chosen {
        sys::color::accent::bg()
    } else {
        state_fill(&key, Color::TRANSPARENT, false)
    };
    let base = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(row_height()))
        .radius(Radius::Fixed(sys::radius::raised()))
        .fill(fill)
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![
                boxed(unread_dot(unread && !chosen)),
                boxed(avatar(thread.title())),
                boxed(
                    Column::new(vec![boxed(top), boxed(bottom)])
                        .gap(2.0)
                        .width(Size::Parent),
                ),
            ])
            .gap(sys::space::gap::s())
            .width(Size::Parent)
            .align(Center),
        );

    let actions = Actions {
        press: Some(Rc::new(move || Navigation::show(View::Thread(Some(id))))),
        ..Actions::default()
    };
    interactive(base, key, false, actions)
}

/// A message on one line, its line breaks as spaces.
fn one_line(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The accent's dot before an unread conversation, or room for it.
fn unread_dot(shown: bool) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(8.0))
        .height(Size::Fixed(8.0))
        .radius(Radius::Full)
        .fill(if shown {
            sys::color::accent::bg()
        } else {
            Color::TRANSPARENT
        })
}

/// A contact's initials in a circle of a colour kept for their name; a
/// person for a bare number.
pub fn avatar(name: &str) -> Rectangle {
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .filter(|c| c.is_alphabetic())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();

    let mark: Box<dyn Widget> = if initials.is_empty() {
        boxed(icon(ICON_PERSON, 20.0, Color::WHITE))
    } else {
        boxed(
            Text::new(initials)
                .size(14.0)
                .weight(600)
                .color(Color::WHITE),
        )
    };

    Rectangle::new()
        .width(Size::Fixed(AVATAR))
        .height(Size::Fixed(AVATAR))
        .radius(Radius::Full)
        .fill(colour_for(name))
        .align_child(Center, Center)
        .child(Column::new(vec![mark]).align(Center))
}

/// The same colour for the same name, from the accents that take white text.
fn colour_for(name: &str) -> Color {
    const COLOURS: [Accent; 8] = [
        Accent::Blue,
        Accent::Green,
        Accent::Orange,
        Accent::Purple,
        Accent::Pink,
        Accent::Teal,
        Accent::Indigo,
        Accent::Brown,
    ];
    let hash = name.bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    if name.chars().any(char::is_alphabetic) {
        COLOURS[hash as usize % COLOURS.len()].color()
    } else {
        Accent::Gray.color()
    }
}

/// Phone Input, under the conversations.
fn input_row(chosen: bool) -> Rectangle {
    let key = String::from("messages.inbox.input");
    let fill = if chosen {
        sys::color::state::selected()
    } else {
        state_fill(&key, Color::TRANSPARENT, false)
    };
    let base = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(sys::size::row::height()))
        .radius(Radius::Fixed(sys::radius::raised()))
        .fill(fill)
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![
                boxed(icon(ICON_KEYBOARD, sys::size::icon::s(), sidebar_label())),
                boxed(
                    Text::new("Phone Input")
                        .style(sys::r#type::label())
                        .color(sidebar_label()),
                ),
            ])
            .gap(sys::space::gap::s())
            .align(Center),
        );
    let actions = Actions {
        press: Some(Rc::new(|| Navigation::show(View::Input))),
        ..Actions::default()
    };
    interactive(base, key, false, actions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_keeps_its_colour() {
        assert_eq!(colour_for("Ola Nordmann"), colour_for("Ola Nordmann"));
        assert_eq!(colour_for("+4795723015"), Accent::Gray.color());
    }

    #[test]
    fn a_message_fits_one_line() {
        assert_eq!(one_line("hei\nder   du"), "hei der du");
    }
}
