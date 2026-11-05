//! One conversation: its messages as bubbles, the other side's grey on the
//! left and this side's in the accent on the right, a day's name over each
//! day and the time under each run; a reply field under them.
//!
//! The phone sends a conversation's history when asked, newest first, and
//! a message sent from here shows at once, dimmed with "Sending…", until
//! the phone sends it back as sent.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use amane::{
    Center, Column, End, PhoneDevice, PhoneMessages, Radius, Rectangle, Row, SMS_PAGE, ScrollArea,
    Service, Size, SmsKind, SmsMessage, Start, Text, TextInput, Widget,
};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::color::with_alpha;
use amane_kit::layout::{boxed, caption, fixed_width, text_height, text_width};
use amane_kit::macform::theme as mac;
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};

use crate::{content_width, inbox, time, title};

const SCROLL: &str = "messages.thread";
const COMPOSE: &str = "messages.compose";

/// A bubble's insets and corners.
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 7.0;
const BUBBLE_RADIUS: f32 = 16.0;

/// Messages of one side further apart than this get their own time.
const RUN_GAP_MS: i64 = 10 * 60 * 1000;

thread_local! {
    // how many messages of each thread were asked for
    static ASKED: RefCell<HashMap<i64, i32>> = RefCell::new(HashMap::new());

    // the thread shown and how many messages it had, to follow new ones down
    static SEEN: Cell<(i64, usize)> = const { Cell::new((0, 0)) };
}

pub fn view(device: &PhoneDevice, thread: Option<i64>) -> Column {
    let Some(id) = thread else {
        return Column::new(vec![boxed(caption(if device.can_message() {
            "Choose a conversation."
        } else {
            "No messages to show."
        }))])
        .width(Size::Parent)
        .height(Size::Parent)
        .align(Center);
    };

    // the newest page of history, once per conversation
    let asked = ASKED.with_borrow(|asked| asked.get(&id).copied());
    if asked.is_none() {
        ask(id, SMS_PAGE);
    }

    let messages = PhoneMessages::read();
    let Some(conversation) = messages.thread(id).cloned() else {
        return Column::new(vec![boxed(caption("This conversation is gone."))]).width(Size::Parent);
    };
    let history: Vec<SmsMessage> = messages.messages(id).to_vec();
    let loading = messages.loading(id);
    let numbers: Vec<String> = conversation.addresses().to_vec();
    drop(messages);

    // opening a conversation, or a message arriving, shows the newest
    if SEEN.get() != (id, history.len()) {
        SEEN.set((id, history.len()));
        ScrollArea::scroll_to_end(SCROLL);
    }

    let header = Row::new(vec![
        boxed(inbox::avatar(conversation.title())),
        boxed(
            Column::new(vec![
                boxed(title(conversation.title().to_string())),
                boxed(caption(numbers.join(", "))),
            ])
            .gap(2.0)
            .width(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::m())
    .width(Size::Parent)
    .align(Center);

    let mut items: Vec<Box<dyn Widget>> = Vec::new();
    let asked = asked.unwrap_or(SMS_PAGE);
    if loading {
        items.push(boxed(centred(caption("Loading messages…"))));
    } else if history.len() as i32 >= asked {
        items.push(boxed(centred(
            PushButton::new("messages.thread.earlier", "Load Earlier Messages")
                .style(Style::Tonal)
                .on_press(move || ask(id, asked + SMS_PAGE))
                .build(),
        )));
    }
    items.extend(bubbles(&history));

    let body = ScrollArea::new(
        SCROLL,
        Column::new(items)
            .gap(sys::space::gap::_2xs())
            .width(Size::Parent),
    )
    .width(Size::Parent)
    .height(Size::Parent);

    Column::new(vec![boxed(header), boxed(body), compose(device, id)])
        .gap(sys::space::gap::l())
        .width(Size::Parent)
        .height(Size::Parent)
}

fn ask(thread: i64, count: i32) {
    ASKED.with_borrow_mut(|asked| asked.insert(thread, count));
    PhoneMessages::load_thread(thread, count);
}

/// The history as bubbles, with each day's name over it and the time under
/// the last of each run.
fn bubbles(history: &[SmsMessage]) -> Vec<Box<dyn Widget>> {
    let mut items: Vec<Box<dyn Widget>> = Vec::new();
    for (index, message) in history.iter().enumerate() {
        let previous = index.checked_sub(1).map(|i| &history[i]);
        let next = history.get(index + 1);

        if previous.is_none_or(|previous| time::new_day(previous.date(), message.date())) {
            items.push(boxed(centred(caption(time::day(message.date())))));
        }

        items.push(boxed(bubble(message)));

        let ends_run = next.is_none_or(|next| {
            next.outgoing() != message.outgoing()
                || next.date() - message.date() > RUN_GAP_MS
                || time::new_day(message.date(), next.date())
        });
        if ends_run || message.pending() || message.kind() == SmsKind::Failed {
            items.push(boxed(stamp(message)));
        }
    }
    items
}

/// `widget` in the middle of the conversation's width.
fn centred(widget: impl Widget + 'static) -> Row {
    Row::new(vec![boxed(widget)])
        .width(Size::Parent)
        .justify(Center)
}

fn bubble(message: &SmsMessage) -> Rectangle {
    let style = sys::r#type::label();
    let body = match (message.body(), message.attachments()) {
        ("", 0) => String::new(),
        ("", n) => format!("{n} attachment{}", if n == 1 { "" } else { "s" }),
        (body, _) => body.to_string(),
    };

    let widest = (content_width() * 0.7).max(120.0);
    let width = (text_width(&body, style) + 2.0 * PAD_X + 2.0).min(widest);
    let text = Text::new(body)
        .style(style)
        .color(if message.outgoing() {
            sys::color::accent::fg()
        } else {
            mac::label_primary()
        })
        .wrap();
    let height = text.height_in(width - 2.0 * PAD_X) + 2.0 * PAD_Y;

    let fill = match message.kind() {
        SmsKind::Failed => sys::color::status::critical::bg(),
        _ if message.pending() => with_alpha(sys::color::accent::bg(), 0.55),
        _ if message.outgoing() => sys::color::accent::bg(),
        _ => mac::control_fill(),
    };

    let bubble = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(Radius::Fixed(BUBBLE_RADIUS))
        .fill(fill)
        .padding(amane_kit::layout::pad(PAD_X, PAD_Y))
        .child(text);

    side(message.outgoing(), height).child(bubble)
}

/// A row as wide as the conversation, holding its child on this side's edge.
fn side(outgoing: bool, height: f32) -> Rectangle {
    let row = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(height));
    if outgoing {
        row.align_child(End, Center)
    } else {
        row.align_child(Start, Center)
    }
}

/// Under a run: when it was sent, or that it's still sending or failed.
fn stamp(message: &SmsMessage) -> Rectangle {
    let (text, color) = match message.kind() {
        SmsKind::Failed => (
            String::from("Not Delivered"),
            sys::color::status::critical::text(),
        ),
        _ if message.pending() => (String::from("Sending…"), material::label(Level::Secondary)),
        _ => (
            time::clock(message.date()),
            material::label(Level::Tertiary),
        ),
    };
    let height = text_height(sys::r#type::caption()) + 4.0;
    side(message.outgoing(), height)
        .padding(amane_kit::layout::pad(PAD_X / 2.0, 0.0))
        .child(Text::new(text).style(sys::r#type::caption()).color(color))
}

/// The reply field and Send; a phone away from the network can't send.
fn compose(device: &PhoneDevice, thread: i64) -> Box<dyn Widget> {
    if !device.reachable() {
        return boxed(caption(
            "The phone isn't on this network, so replies can't be sent.",
        ));
    }

    let send = move || {
        let text = TextInput::text(COMPOSE);
        if !text.trim().is_empty() {
            PhoneMessages::reply(thread, &text);
            TextInput::set_text(COMPOSE, "");
        }
    };

    let button = PushButton::new("messages.send", "Send")
        .style(Style::Filled)
        .on_press(send)
        .build();
    let field_w = (content_width() - fixed_width(&button) - sys::space::gap::s()).max(1.0);
    let field = field_box(
        COMPOSE,
        text_field(COMPOSE, "Text Message")
            .focused()
            .on_submit(move |_| send()),
        Size::Fixed(field_w),
    );

    boxed(
        Row::new(vec![boxed(field), boxed(button)])
            .gap(sys::space::gap::s())
            .width(Size::Parent)
            .align(Center),
    )
}
