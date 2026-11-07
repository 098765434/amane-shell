//! The inbox: the phone's conversations, newest first, each with its
//! contact's photo or initials, name, latest message and when; an unread
//! one in bold with the accent's dot. A search field over them filters by
//! name, number or message, and New Message starts one. Phone Input sits
//! under them.

use amane_kit::avatar::Avatar;

use amane::{
    Center, Color, Column, Radius, Rectangle, Row, ScrollArea, Service, Size, Text, TextInput,
    Widget,
};
use amane_kit::app::{field_box, sidebar_label, text_field};
use amane_kit::buttons::{Actions, IconButton, interactive, state_fill};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::color::Accent;
use amane_kit::control_size::ControlSize;
use amane_kit::icons::ICON_KEYBOARD;
use amane_kit::layout::{boxed, caption, icon, pad, text_height};
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};
use amane_phone::{PhoneDevice, PhoneMessages, SmsState, SmsThread};

use crate::{Navigation, View, read, thread, time};

/// `refresh`, `group` and `edit_square`.
const ICON_REFRESH: char = '\u{e5d5}';
const ICON_GROUP: char = '\u{e7ef}';
const ICON_NEW: char = '\u{f88d}';

/// The search field's id.
pub const SEARCH: &str = "messages.search";

thread_local! {
    // the search field is to take the keys when next built (Ctrl+F)
    static FOCUS_SEARCH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Puts the keys in the search field.
pub fn focus_search() {
    FOCUS_SEARCH.set(true);
    amane::mark_all();
}

const AVATAR: f32 = 36.0;

pub fn view(device: Option<&PhoneDevice>, view: View) -> Column {
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(header(device))];

    let Some(device) = device else {
        return Column::new(parts).width(Size::Parent).height(Size::Parent);
    };

    if device.can_message() {
        parts.push(boxed(search()));
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

/// The phone's name with how many conversations are unread, a button
/// asking it for its messages again (a phone that was asleep sends nothing
/// until asked), and New Message.
fn header(device: Option<&PhoneDevice>) -> Row {
    let name = device.map_or("Messages", PhoneDevice::name).to_string();
    let unread = PhoneMessages::read()
        .threads()
        .iter()
        .filter(|thread| read::unread(thread))
        .count();

    let mut title: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new(name)
            .style(sys::r#type::title())
            .color(sidebar_label())
            .elide(),
    )];
    if unread > 0 {
        title.push(boxed(badge(unread)));
    }

    let button = |id: &'static str, glyph: char, tip: &'static str, press: fn()| {
        boxed(
            IconButton::new(id, glyph)
                .size(sys::size::icon::s())
                .height(ControlSize::Medium.height())
                .width(ControlSize::Medium.height())
                .on_press(press)
                .tooltip("main", tip)
                .build(),
        )
    };

    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(
        Column::new(vec![boxed(
            Row::new(title).gap(sys::space::gap::s()).align(Center),
        )])
        .width(Size::Parent),
    )];
    parts.push(button(
        "messages.refresh",
        ICON_REFRESH,
        "Check for New Messages",
        PhoneMessages::refresh,
    ));
    if device.is_some_and(|device| device.can_message() && device.reachable()) {
        parts.push(button(
            "messages.new",
            ICON_NEW,
            "New Message (Ctrl+N)",
            || Navigation::show(View::New),
        ));
    }
    Row::new(parts).width(Size::Parent).align(Center)
}

/// How many conversations are unread, in the accent's pill.
fn badge(count: usize) -> Rectangle {
    let text = count.to_string();
    let style = sys::r#type::caption();
    let width = (amane_kit::layout::text_width(&text, style) + 12.0).max(20.0);
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(20.0))
        .radius(Radius::Full)
        .fill(sys::color::accent::bg())
        .align_child(Center, Center)
        .child(
            Text::new(text)
                .style(style)
                .weight(600)
                .color(sys::color::accent::fg()),
        )
}

/// The field that filters the conversations.
fn search() -> Rectangle {
    let mut input = text_field(SEARCH, "Search").on_change(|_| amane::mark_all());
    if FOCUS_SEARCH.get() {
        input = input.focused();
        if TextInput::has_focus(SEARCH) {
            FOCUS_SEARCH.set(false);
        }
    }
    field_box(SEARCH, input, Size::Parent)
}

/// Whether `thread` is one the search asks for.
fn matches(thread: &SmsThread, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    let digits: String = query.chars().filter(char::is_ascii_digit).collect();
    thread.title().to_lowercase().contains(&query)
        || thread.latest().body().to_lowercase().contains(&query)
        || (digits.len() >= 3
            && thread.addresses().iter().any(|address| {
                address
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .contains(&digits)
            }))
}

/// The conversations the search shows, in order, for moving through them
/// with the keyboard.
pub fn shown_ids() -> Vec<i64> {
    let query = TextInput::text(SEARCH);
    PhoneMessages::read()
        .threads()
        .iter()
        .filter(|thread| matches(thread, query.trim()))
        .map(SmsThread::id)
        .collect()
}

fn conversations(view: View) -> ScrollArea {
    let messages = PhoneMessages::read();
    let query = TextInput::text(SEARCH);
    let threads: Vec<SmsThread> = messages
        .threads()
        .iter()
        .filter(|thread| matches(thread, query.trim()))
        .cloned()
        .collect();
    let state = messages.state();
    let any = !messages.threads().is_empty();
    drop(messages);

    let shown = match view {
        View::Thread(thread) => thread,
        View::Input | View::New => None,
    };

    let rows: Vec<Box<dyn Widget>> = if threads.is_empty() {
        empty(state, any)
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

/// Why there's no conversation to list, and what to do.
fn empty(state: SmsState, any: bool) -> Vec<Box<dyn Widget>> {
    if any {
        return vec![boxed(caption("No conversation matches."))];
    }
    match state {
        SmsState::NoAnswer => vec![
            boxed(caption(
                "No answer from the phone. Unlock it and open KDE Connect on it, or start a conversation.",
            )),
            boxed(
                PushButton::new("messages.inbox.retry", "Try Again")
                    .style(Style::Tonal)
                    .on_press(PhoneMessages::refresh)
                    .build(),
            ),
        ],
        SmsState::Ready => vec![boxed(caption("No conversations yet."))],
        SmsState::Idle | SmsState::Syncing => vec![boxed(caption("Asking the phone…"))],
    }
}

/// What a message shows as in the list: its words, else what's attached.
fn snippet(latest: &amane_phone::SmsMessage) -> String {
    let attached = latest.attachments();
    let words = match (latest.body().trim(), attached.first()) {
        ("", None) => return String::new(),
        ("", Some(part)) if part.image() => String::from("Photo"),
        ("", Some(part)) if part.video() => String::from("Video"),
        ("", Some(_)) => String::from("Attachment"),
        (body, _) => one_line(body),
    };
    if latest.outgoing() {
        format!("You: {words}")
    } else {
        words
    }
}

fn row(thread: &SmsThread, chosen: bool) -> Rectangle {
    let id = thread.id();
    let key = format!("messages.inbox.{id}");
    let unread = read::unread(thread);

    let latest = thread.latest();
    let snippet = snippet(latest);

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

    let title = if thread.group() {
        let names: Vec<String> = thread.title().split(", ").map(String::from).collect();
        thread::group_title(&names)
    } else {
        thread.title().to_string()
    };
    let name = Text::new(title)
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
                boxed(avatar_for(thread, AVATAR)),
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

    let actions = Actions::press(move || Navigation::show(View::Thread(Some(id))));
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

/// A conversation's picture: the contact's photo, or initials, or for a
/// group the group's mark.
pub fn avatar_for(thread: &SmsThread, size: f32) -> Rectangle {
    if thread.group() {
        return Rectangle::new()
            .width(Size::Fixed(size))
            .height(Size::Fixed(size))
            .radius(Radius::Full)
            .fill(Accent::Gray.color())
            .align_child(Center, Center)
            .child(icon(ICON_GROUP, size * 0.55, Color::WHITE));
    }
    match thread.addresses().first() {
        Some(address) => avatar_of(address, size),
        None => Avatar::new(thread.title(), size).build(),
    }
}

/// The person with this number: their photo, or their initials.
pub fn avatar_of(address: &str, size: f32) -> Rectangle {
    let messages = PhoneMessages::read();
    let photo = messages.photo(address).map(std::path::Path::to_path_buf);
    Avatar::new(messages.name(address), size)
        .photo(photo)
        .build()
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
    let actions = Actions::press(|| Navigation::show(View::Input));
    interactive(base, key, false, actions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_fits_one_line() {
        assert_eq!(one_line("hei\nder   du"), "hei der du");
    }
}
