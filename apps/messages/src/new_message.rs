//! A new message: whom it's to, picked from the phone's contacts as you
//! type (a name or part of a number) or typed as a number, then the
//! message. Someone you already have a conversation with opens that one
//! instead; a new conversation shows at once and becomes the phone's when
//! it sends the message back.

use std::cell::RefCell;

use amane::{
    Center, Color, Column, Radius, Rectangle, Row, Service, Size, Text, TextInput, Widget,
};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{Actions, interactive, state_fill};
use amane_kit::color::with_alpha;
use amane_kit::layout::{boxed, caption, icon, pad, text_width};
use amane_kit::material::{self, Level};
use amane_kit::style;
use amane_kit::tokens::{Styled, sys};
use amane_phone::PhoneMessages;

use crate::compose::{self, Draft};
use crate::{Navigation, View, content_width, inbox, title};

/// The field the people are typed in.
pub const TO: &str = "messages.new.to";

/// `close`.
const ICON_CLOSE: char = '\u{e5cd}';

/// How many contacts are suggested at most.
const SUGGESTIONS: usize = 6;

thread_local! {
    // the people picked, by name and number
    static PICKED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };

    // the To field is to take the keys when next built, until it has them
    static WANTS_FOCUS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Asks for the To field to take the keys, as a new message opens.
pub fn focus() {
    WANTS_FOCUS.set(true);
}

fn picked() -> Vec<(String, String)> {
    PICKED.with_borrow(Clone::clone)
}

/// Forgets whom the message was to, when it's sent or left.
pub fn reset() {
    PICKED.with_borrow_mut(Vec::clear);
    TextInput::set_text(TO, "");
}

/// Adds a person; one you already have a conversation with alone opens it.
fn pick(name: String, number: String) {
    TextInput::set_text(TO, "");
    let first = PICKED.with_borrow(Vec::is_empty);
    if first && let Some(thread) = PhoneMessages::read().thread_with(std::slice::from_ref(&number))
    {
        reset();
        Navigation::show(View::Thread(Some(thread)));
        return;
    }
    PICKED.with_borrow_mut(|picked| {
        if !picked
            .iter()
            .any(|(_, old)| amane_phone::digits(old) == amane_phone::digits(&number))
        {
            picked.push((name, number));
        }
    });
    compose::focus();
    amane::mark_all();
}

fn unpick(number: &str) {
    PICKED.with_borrow_mut(|picked| picked.retain(|(_, old)| old != number));
    amane::mark_all();
}

/// Whether `text` could be a phone number: digits, with the usual marks.
fn number_like(text: &str) -> bool {
    let text = text.trim();
    amane_phone::digits(text).len() >= 3
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')'))
}

/// The contacts whose name or number holds `query`, best first: names
/// that start with it, then the rest.
fn suggestions(query: &str) -> Vec<(String, String)> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let wanted_digits = amane_phone::digits(&query);
    let contacts = PhoneMessages::read().contacts();
    let mut found: Vec<(u8, (String, String))> = contacts
        .into_iter()
        .filter_map(|(name, number)| {
            let lower = name.to_lowercase();
            let rank = if lower.starts_with(&query)
                || lower
                    .split_whitespace()
                    .any(|word| word.starts_with(&query))
            {
                0
            } else if lower.contains(&query) {
                1
            } else if wanted_digits.len() >= 3
                && amane_phone::digits(&number).contains(&wanted_digits)
            {
                2
            } else {
                return None;
            };
            Some((rank, (name, number)))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.0.cmp(&b.1.0)));
    found.dedup_by(|a, b| a.1 == b.1);
    found
        .into_iter()
        .take(SUGGESTIONS)
        .map(|(_, contact)| contact)
        .collect()
}

pub fn view() -> Column {
    let typed = TextInput::text(TO);
    let found = suggestions(&typed);

    let first = found.first().cloned();
    let typed_now = typed.clone();
    let mut to_field = text_field(TO, "Name or number").on_submit(move |_| {
        if let Some((name, number)) = first.clone() {
            pick(name, number);
        } else if number_like(&typed_now) {
            pick(typed_now.trim().to_string(), typed_now.trim().to_string());
        }
    });
    if WANTS_FOCUS.get() {
        to_field = to_field.focused();
        if TextInput::has_focus(TO) {
            WANTS_FOCUS.set(false);
        }
    }
    let on_change = to_field.on_change(|_| amane::mark_all());

    let mut to_row: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new("To:")
            .style(sys::r#type::label())
            .color(material::label(Level::Secondary)),
    )];
    for (name, number) in picked() {
        to_row.push(boxed(token(&name, &number)));
    }
    to_row.push(boxed(field_box(TO, on_change, Size::Parent)));

    let mut parts: Vec<Box<dyn Widget>> = vec![
        boxed(title("New Message")),
        boxed(
            Row::new(to_row)
                .gap(sys::space::gap::s())
                .width(Size::Parent)
                .align(Center),
        ),
    ];

    let mut list: Vec<Box<dyn Widget>> = found
        .into_iter()
        .map(|(name, number)| boxed(suggestion(name, number)))
        .collect();
    if list.is_empty() && number_like(&typed) {
        let number = typed.trim().to_string();
        list.push(boxed(suggestion(number.clone(), number)));
    } else if list.is_empty() && !typed.trim().is_empty() {
        list.push(boxed(caption(
            "No contact by that name. Type a number instead.",
        )));
    }
    parts.push(boxed(
        Column::new(list)
            .gap(sys::space::gap::_2xs())
            .width(Size::Parent)
            .height(Size::Parent),
    ));

    let people = picked();
    if people.is_empty() {
        parts.push(boxed(caption(
            "Pick whom to write to, then write the message.",
        )));
    } else {
        parts.push(boxed(compose::view(
            Draft::New,
            "Text Message",
            content_width(),
            move |text, files| {
                let numbers: Vec<String> =
                    people.iter().map(|(_, number)| number.clone()).collect();
                let Some(thread) = PhoneMessages::new_conversation(&numbers, text, files) else {
                    return false;
                };
                reset();
                Navigation::show(View::Thread(Some(thread)));
                true
            },
        )));
    }

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
        .height(Size::Parent)
}

/// One person picked, with a button to take them off.
fn token(name: &str, number: &str) -> Rectangle {
    let style = sys::r#type::label();
    let key = format!("messages.new.unpick.{number}");
    let removed = number.to_string();
    let remove = interactive(
        Rectangle::new()
            .width(Size::Fixed(18.0))
            .height(Size::Fixed(18.0))
            .radius(Radius::Full)
            .fill(state_fill(&key, Color::TRANSPARENT, false))
            .align_child(Center, Center)
            .child(icon(ICON_CLOSE, 14.0, sys::color::accent::fg())),
        key,
        false,
        Actions::press(move || unpick(&removed)),
    );
    Rectangle::new()
        .width(10.0 + text_width(name, style) + 4.0 + 18.0 + 6.0)
        .height(Size::Fixed(26.0))
        .radius(Radius::Full)
        .fill(sys::color::accent::bg())
        .padding(pad(10.0, 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![
                boxed(
                    Text::new(name.to_string())
                        .style(style)
                        .color(sys::color::accent::fg()),
                ),
                boxed(remove),
            ])
            .gap(4.0)
            .align(Center),
        )
}

/// A contact to pick: their photo or initials, name and number.
fn suggestion(name: String, number: String) -> Rectangle {
    let key = format!("messages.new.pick.{number}");
    let under = if name == number {
        String::new()
    } else {
        number.clone()
    };
    let mut text: Vec<Box<dyn Widget>> = vec![boxed(
        Text::new(name.clone())
            .style(sys::r#type::label())
            .color(style::label_primary())
            .elide(),
    )];
    if !under.is_empty() {
        text.push(boxed(caption(under)));
    }
    let picked_name = name;
    let picked_number = number.clone();
    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(44.0))
            .radius(Radius::Fixed(sys::radius::raised()))
            .fill(state_fill(&key, with_alpha(Color::WHITE, 0.0), false))
            .padding(pad(sys::space::inset::s(), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Row::new(vec![
                    boxed(inbox::avatar_of(&number, 32.0)),
                    boxed(Column::new(text).gap(1.0).width(Size::Parent)),
                ])
                .gap(sys::space::gap::s())
                .width(Size::Parent)
                .align(Center),
            ),
        key,
        false,
        Actions::press(move || pick(picked_name.clone(), picked_number.clone())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_digits_and_the_usual_marks() {
        assert!(number_like("+47 957 23 015"));
        assert!(number_like("(957) 23-015"));
        assert!(!number_like("Ola"));
        assert!(!number_like("12"));
    }
}
