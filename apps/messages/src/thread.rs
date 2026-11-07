//! One conversation: its messages as bubbles, the other side's grey on the
//! left and this side's in the accent on the right, a day's name over each
//! day and the time under each run; in a group, who wrote each run. Photos
//! and files show as their own bubbles, links as a chip under theirs. A
//! right click on a message copies it, or the code or link in it. The
//! field to reply in is under them.
//!
//! The phone sends a conversation's history when asked, newest first, and
//! a message sent from here shows at once, dimmed with "Sending…", until
//! the phone sends it back as sent, or "Not Delivered" with Retry when it
//! doesn't in time.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use amane::{
    Button, Center, Clipboard, Color, Column, End, Image, Radius, Rectangle, Row, ScrollArea,
    Service, Shadow, Size, Stack, Start, Text, Widget,
};
use amane_kit::buttons::{Actions, PushButton, Style, interactive, state_fill};
use amane_kit::color::with_alpha;
use amane_kit::layout::{boxed, caption, pad, text_height, text_width};
use amane_kit::material::{self, Level};
use amane_kit::menu::{self, MenuItem};
use amane_kit::style;
use amane_kit::tokens::{Styled, sys};
use amane_phone::{
    PhoneDevice, PhoneMessages, SMS_PAGE, SmsAttachment, SmsKind, SmsMessage, SmsThread,
};

use crate::bubble::{self, Look};
use crate::compose::{self, Draft};
use crate::{content_width, inbox, links, phone, read, time, title};

const SCROLL: &str = "messages.thread";

/// How far a run's time sits in from its bubbles' edge.
const STAMP_INSET: f32 = 6.0;

/// Messages of one side further apart than this get their own time.
const RUN_GAP_MS: i64 = 10 * 60 * 1000;

/// The small avatar beside a group's received runs.
const SMALL_AVATAR: f32 = 28.0;

/// How big a photo is drawn at most.
const PHOTO_WIDTH: f32 = 240.0;
const PHOTO_HEIGHT: f32 = 320.0;

/// `link`, `play_arrow`, `description` and `arrow_downward`.
const ICON_LINK: char = '\u{e157}';
const ICON_PLAY: char = '\u{e037}';
const ICON_FILE: char = '\u{e873}';
const ICON_DOWN: char = '\u{e5db}';

thread_local! {
    // how many messages of each thread were asked for
    static ASKED: RefCell<HashMap<i64, i32>> = RefCell::new(HashMap::new());

    // the thread shown and its newest message, to follow new ones down
    static SEEN: Cell<(i64, i64)> = const { Cell::new((0, 0)) };

    // a message came while the conversation was scrolled up
    static NEW_BELOW: Cell<bool> = const { Cell::new(false) };
}

pub fn view(device: &PhoneDevice, thread: Option<i64>) -> Column {
    let Some(id) = thread else {
        return Column::new(vec![boxed(caption(if device.can_message() {
            "Choose a conversation, or start a new one."
        } else {
            "No messages to show."
        }))])
        .width(Size::Parent)
        .height(Size::Parent)
        .align(Center);
    };

    // the newest page of history, once per conversation
    let asked = ASKED.with_borrow(|asked| asked.get(&id).copied());
    if asked.is_none() && id >= 0 {
        ask(id, SMS_PAGE);
    }

    let messages = PhoneMessages::read();
    let Some(conversation) = messages.thread(id).cloned() else {
        return Column::new(vec![boxed(caption("This conversation is gone."))]).width(Size::Parent);
    };
    let history: Vec<SmsMessage> = messages.messages(id).to_vec();
    let loading = messages.loading(id);
    drop(messages);

    read::mark(&conversation);
    follow(id, &history);

    let mut items: Vec<Box<dyn Widget>> = Vec::new();
    let asked = asked.unwrap_or(SMS_PAGE);
    if loading {
        items.push(boxed(centred(caption("Loading messages…"))));
    } else if id >= 0 && history.len() as i32 >= asked {
        items.push(boxed(centred(
            PushButton::new("messages.thread.earlier", "Load Earlier Messages")
                .style(Style::Tonal)
                .on_press(move || {
                    // they come in above: what's being read stays put
                    ScrollArea::keep_bottom(SCROLL);
                    ask(id, asked + SMS_PAGE);
                })
                .build(),
        )));
    }
    items.extend(bubbles(&history, conversation.group()));

    let list = ScrollArea::new(
        SCROLL,
        Column::new(items)
            .gap(sys::space::gap::_2xs())
            .width(Size::Parent),
    )
    .width(Size::Parent)
    .height(Size::Parent);

    let body: Box<dyn Widget> = if NEW_BELOW.get() {
        boxed(Stack::new(vec![boxed(list), boxed(new_below())]))
    } else {
        boxed(list)
    };

    let width = content_width();
    let reply: Box<dyn Widget> = if device.reachable() {
        boxed(compose::view(
            Draft::Thread(id),
            "Text Message",
            width,
            move |text, files| {
                PhoneMessages::reply(id, text, files);
                ScrollArea::scroll_to_end(SCROLL);
                true
            },
        ))
    } else {
        boxed(caption(
            "The phone isn't on this network, so replies can't be sent.",
        ))
    };

    let dropped = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .on_drop(move |paths: Vec<PathBuf>| compose::attach(Draft::Thread(id), paths))
        .child(
            Column::new(vec![boxed(header(device, &conversation)), body, reply])
                .gap(sys::space::gap::l())
                .width(Size::Parent)
                .height(Size::Parent),
        );

    Column::new(vec![boxed(dropped)])
        .width(Size::Parent)
        .height(Size::Parent)
}

fn ask(thread: i64, count: i32) {
    ASKED.with_borrow_mut(|asked| asked.insert(thread, count));
    PhoneMessages::load_thread(thread, count);
}

/// Opening a conversation shows its newest message; a new one is followed
/// down only while the newest is in view, else a pill says it's there.
fn follow(id: i64, history: &[SmsMessage]) {
    let newest = history.last().map_or(0, SmsMessage::uid);
    let (shown, seen) = SEEN.get();
    if shown != id {
        SEEN.set((id, newest));
        NEW_BELOW.set(false);
        ScrollArea::scroll_to_end(SCROLL);
        compose::focus();
        return;
    }
    if newest != seen {
        SEEN.set((id, newest));
        let mine = history.last().is_some_and(SmsMessage::local);
        if mine || ScrollArea::at_end(SCROLL) {
            ScrollArea::scroll_to_end(SCROLL);
        } else {
            NEW_BELOW.set(true);
        }
    }
    if NEW_BELOW.get() && ScrollArea::at_end(SCROLL) {
        NEW_BELOW.set(false);
    }
}

/// The pill over the bottom of the conversation when a message came in
/// below what's shown.
fn new_below() -> Rectangle {
    let key = String::from("messages.thread.new_below");
    let label = "New Message";
    let style = sys::r#type::caption();
    let width = text_width(label, style) + 16.0 + 6.0 + 24.0;
    let pill = interactive(
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(28.0))
            .radius(Radius::Full)
            .fill(state_fill(&key, sys::color::accent::bg(), true))
            .shadow(
                Shadow::drop(with_alpha(Color::BLACK, 0.2))
                    .blur(8.0)
                    .offset(0.0, 2.0),
            )
            .align_child(Center, Center)
            .child(
                Row::new(vec![
                    boxed(amane_kit::layout::icon(
                        ICON_DOWN,
                        16.0,
                        sys::color::accent::fg(),
                    )),
                    boxed(
                        Text::new(label)
                            .style(style)
                            .color(sys::color::accent::fg()),
                    ),
                ])
                .gap(6.0)
                .align(Center),
            ),
        key,
        false,
        Actions {
            press: Some(Rc::new(|| {
                NEW_BELOW.set(false);
                ScrollArea::scroll_to_end(SCROLL);
            })),
            ..Actions::default()
        },
    );
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .padding(pad(0.0, 12.0))
        .align_child(Center, End)
        .child(pill)
}

/// The conversation's people: their avatar, names and numbers, and the
/// phone's own messaging app when its screen can be shown.
fn header(device: &PhoneDevice, conversation: &SmsThread) -> Row {
    let messages = PhoneMessages::read();
    let names: Vec<String> = conversation
        .addresses()
        .iter()
        .map(|address| messages.name(address).to_string())
        .collect();
    drop(messages);

    let (heading, under) = if conversation.group() {
        (group_title(&names), names.join(", "))
    } else {
        let number = conversation.addresses().join(", ");
        let under = if names.first() == Some(&number) {
            String::new()
        } else {
            number
        };
        (conversation.title().to_string(), under)
    };

    let mut text: Vec<Box<dyn Widget>> = vec![boxed(title(heading))];
    if !under.is_empty() {
        text.push(boxed(caption(under)));
    }

    let mut parts: Vec<Box<dyn Widget>> = vec![
        boxed(inbox::avatar_for(conversation, 36.0)),
        boxed(Column::new(text).gap(2.0).width(Size::Parent)),
    ];

    if phone::screen_available(device)
        && let Some((package, name)) = phone::messaging_app(device)
    {
        let device = device.clone();
        parts.push(boxed(
            PushButton::new("messages.thread.on_phone", "Show on Phone")
                .style(Style::Tonal)
                .on_press(move || phone::open_app(&device, &package, &name))
                .build(),
        ));
    }

    Row::new(parts)
        .gap(sys::space::gap::m())
        .width(Size::Parent)
        .align(Center)
}

/// A group's name from its people's: "Ola, Kari and 2 others".
pub fn group_title(names: &[String]) -> String {
    let first = |name: &String| name.split_whitespace().next().unwrap_or(name).to_string();
    match names {
        [] => String::from("Group"),
        [one] => one.clone(),
        [a, b] => format!("{} and {}", first(a), first(b)),
        [a, b, c] => format!("{}, {} and {}", first(a), first(b), first(c)),
        [a, b, rest @ ..] => format!("{}, {} and {} others", first(a), first(b), rest.len()),
    }
}

/// The history as bubbles, with each day's name over it, the time under
/// the last of each run, and in a group who wrote each received run.
fn bubbles(history: &[SmsMessage], group: bool) -> Vec<Box<dyn Widget>> {
    let messages = PhoneMessages::read();
    let mut items: Vec<Box<dyn Widget>> = Vec::new();
    for (index, message) in history.iter().enumerate() {
        let previous = index.checked_sub(1).map(|i| &history[i]);
        let next = history.get(index + 1);

        let day = previous.is_none_or(|previous| time::new_day(previous.date(), message.date()));
        if day {
            items.push(boxed(centred(caption(time::day(message.date())))));
        }

        let same_run = |a: &SmsMessage, b: &SmsMessage| {
            a.outgoing() == b.outgoing()
                && a.sender() == b.sender()
                && (b.date() - a.date()).abs() <= RUN_GAP_MS
                && !time::new_day(a.date(), b.date())
        };
        let starts_run = day || previous.is_none_or(|previous| !same_run(previous, message));
        let ends_run = next.is_none_or(|next| !same_run(message, next));

        // who wrote it, over a group's received run
        let sender = (group && !message.outgoing())
            .then(|| message.sender())
            .flatten();
        if starts_run && let Some(sender) = sender {
            items.push(boxed(
                Rectangle::new()
                    .width(Size::Parent)
                    .height(Size::Fixed(text_height(sys::r#type::caption()) + 2.0))
                    .padding(pad(SMALL_AVATAR + sys::space::gap::s() + 12.0, 0.0))
                    .align_child(Start, Center)
                    .child(
                        Text::new(messages.name(sender).to_string())
                            .style(sys::r#type::caption())
                            .color(material::label(Level::Secondary)),
                    ),
            ));
        }

        let avatar = sender.map(|sender| ends_run.then(|| sender.to_string()));

        let parts = parts(message, ends_run);
        let last = parts.len().saturating_sub(1);
        for (at, part) in parts.into_iter().enumerate() {
            // the avatar beside the last of a run, room for it beside the rest
            let beside = avatar
                .as_ref()
                .map(|shown| shown.as_ref().filter(|_| at == last));
            items.push(boxed(side(message.outgoing(), beside, part)));
        }

        for link in links::links(message.body()).into_iter().take(3) {
            let beside = avatar.as_ref().map(|_| None);
            items.push(boxed(side(message.outgoing(), beside, link_chip(&link))));
        }

        if ends_run || message.pending() || message.kind() == SmsKind::Failed {
            items.push(boxed(stamp(message, avatar.is_some())));
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

/// What a message shows, top to bottom: its photos and files, then its
/// words in a bubble, the last with the tail when it ends a run. Each
/// takes a right click.
fn parts(message: &SmsMessage, tail: bool) -> Vec<Rectangle> {
    let mut parts: Vec<Rectangle> = message.attachments().iter().map(attachment).collect();

    let body = message.body().trim();
    if links::emoji_only(body) {
        parts.push(bubble::emoji(body));
    } else if !body.is_empty() {
        let look = match message.kind() {
            SmsKind::Failed => Look::Failed,
            _ if message.pending() => Look::Pending,
            _ if message.outgoing() => Look::Sent,
            _ => Look::Received,
        };
        let widest = (content_width() * 0.7).max(120.0);
        parts.push(bubble::bubble(message.uid(), body, look, tail, widest));
    }

    let menu = Rc::new(message.clone());
    parts
        .into_iter()
        .map(|part| {
            let message = menu.clone();
            part.on_click(move |button| {
                if button == Button::Right {
                    context_menu(&message);
                }
            })
        })
        .collect()
}

/// What a right click on a message offers.
fn context_menu(message: &SmsMessage) {
    #[derive(Clone)]
    enum Pick {
        Copy(String),
        Open(String),
        Retry,
        Delete,
    }

    let mut items = Vec::new();
    let mut picks = Vec::new();
    let body = message.body().trim().to_string();
    if !body.is_empty() {
        items.push(MenuItem::new("Copy").icon('\u{e14d}'));
        picks.push(Pick::Copy(body.clone()));
    }
    if let Some(code) = links::code(&body) {
        items.push(MenuItem::new(format!("Copy Code {code}")).icon('\u{e73c}'));
        picks.push(Pick::Copy(code));
    }
    for link in links::links(&body).into_iter().take(3) {
        items.push(MenuItem::new(format!("Open {}", links::shown(&link))).icon(ICON_LINK));
        picks.push(Pick::Open(links::url(&link)));
        items.push(MenuItem::new("Copy Link").icon('\u{e14d}'));
        picks.push(Pick::Copy(links::url(&link)));
    }
    for part in message.attachments() {
        items.push(MenuItem::new(attachment_label(part)).icon(ICON_FILE));
        picks.push(Pick::Open(part.unique().to_string()));
    }
    if message.kind() == SmsKind::Failed {
        items.push(MenuItem::new("Send Again").icon('\u{e5d5}'));
        picks.push(Pick::Retry);
        if message.local() {
            items.push(MenuItem::new("Delete").icon('\u{e872}'));
            picks.push(Pick::Delete);
        }
    }
    if items.is_empty() {
        return;
    }

    let message = message.clone();
    menu::context_menu("messages.thread.menu", items, move |index| {
        match picks.get(index).cloned() {
            Some(Pick::Copy(text)) => Clipboard::copy(text),
            Some(Pick::Open(target)) => {
                if let Some(part) = message
                    .attachments()
                    .iter()
                    .find(|part| part.unique() == target)
                {
                    PhoneMessages::download(part);
                } else {
                    amane_kit::launch::open(&target);
                }
            }
            Some(Pick::Retry) => retry(&message),
            Some(Pick::Delete) => PhoneMessages::discard(message.uid()),
            None => {}
        }
    });
}

/// Sends a failed message again: one sent from here as it was, one the
/// phone says failed as a new reply.
fn retry(message: &SmsMessage) {
    if message.local() {
        PhoneMessages::retry(message.uid());
    } else {
        PhoneMessages::reply(message.thread(), message.body(), &[]);
    }
}

/// A row as wide as the conversation, holding `child` on its side's edge;
/// in a group, a received one has its sender's avatar beside it, or room
/// for it (`beside` is `Some(None)`).
fn side(outgoing: bool, beside: Option<Option<&String>>, child: Rectangle) -> Rectangle {
    let height = match child.height() {
        Size::Fixed(height) => height,
        Size::Parent => 0.0,
    };
    let row = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(height));
    if outgoing {
        return row.align_child(End, Center).child(child);
    }
    let Some(avatar) = beside else {
        return row.align_child(Start, Center).child(child);
    };
    let avatar: Box<dyn Widget> = match avatar {
        Some(address) => boxed(inbox::avatar_of(address, SMALL_AVATAR)),
        None => boxed(
            Rectangle::new()
                .width(Size::Fixed(SMALL_AVATAR))
                .height(Size::Fixed(1.0)),
        ),
    };
    row.align_child(Start, End).child(
        Row::new(vec![avatar, boxed(child)])
            .gap(sys::space::gap::s())
            .align(End),
    )
}

/// A photo or video as a picture of its own, a file as a chip; a click
/// opens it, fetching it from the phone first.
fn attachment(part: &SmsAttachment) -> Rectangle {
    let key = format!("messages.attachment.{}", part.unique());
    let fetching = PhoneMessages::read().downloading(part.unique());
    let opened = part.clone();
    let actions = Actions {
        press: Some(Rc::new(move || PhoneMessages::download(&opened))),
        ..Actions::default()
    };

    let picture = part
        .thumbnail()
        .filter(|_| part.image() || part.video())
        .map(|path| path.to_path_buf());

    let Some(path) = picture else {
        return interactive(file_chip(part, &key, fetching), key, false, actions);
    };

    let (width, height) = match Image::size(&path) {
        Some((w, h)) if w > 0 && h > 0 => {
            let scale = (PHOTO_WIDTH / w as f32).min(PHOTO_HEIGHT / h as f32);
            (w as f32 * scale, h as f32 * scale)
        }
        _ => (PHOTO_WIDTH, PHOTO_WIDTH * 0.75),
    };

    let mut over: Vec<Box<dyn Widget>> = Vec::new();
    if part.video() || fetching {
        let mark: Box<dyn Widget> = if fetching {
            boxed(
                Text::new("Opening…")
                    .style(sys::r#type::caption())
                    .color(Color::WHITE),
            )
        } else {
            boxed(amane_kit::layout::icon(ICON_PLAY, 28.0, Color::WHITE))
        };
        over.push(boxed(
            Rectangle::new()
                .width(Size::Parent)
                .height(Size::Parent)
                .align_child(Center, Center)
                .child(
                    Rectangle::new()
                        .width(Size::Fixed(if fetching { 84.0 } else { 48.0 }))
                        .height(Size::Fixed(if fetching { 28.0 } else { 48.0 }))
                        .radius(Radius::Full)
                        .fill(with_alpha(Color::BLACK, 0.45))
                        .align_child(Center, Center)
                        .child(Column::new(vec![mark]).align(Center)),
                ),
        ));
    }

    let image = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(Radius::Fixed(16.0))
        .fill(Image::cover(path))
        .clip()
        .child(Stack::new(over));
    interactive(image, key, false, actions)
}

/// What a file is, in a word or two.
fn attachment_label(part: &SmsAttachment) -> String {
    let kind = match part.mime() {
        mime if mime.starts_with("image/") => "Photo",
        mime if mime.starts_with("video/") => "Video",
        mime if mime.starts_with("audio/") => "Audio",
        "text/vcard" | "text/x-vcard" => "Contact Card",
        "application/pdf" => "PDF",
        _ => "File",
    };
    format!("Open {kind}")
}

fn file_chip(part: &SmsAttachment, key: &str, fetching: bool) -> Rectangle {
    let label = if fetching {
        String::from("Opening…")
    } else {
        attachment_label(part)
            .trim_start_matches("Open ")
            .to_string()
    };
    let style = sys::r#type::label();
    let width = 12.0 + 20.0 + 8.0 + text_width(&label, style) + 14.0;
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(40.0))
        .radius(Radius::Fixed(20.0))
        .fill(state_fill(key, style::bubble_received(), false))
        .padding(pad(12.0, 0.0))
        .align_child(Start, Center)
        .child(
            Row::new(vec![
                boxed(amane_kit::layout::icon(
                    ICON_FILE,
                    20.0,
                    style::label_primary(),
                )),
                boxed(Text::new(label).style(style).color(style::label_primary())),
            ])
            .gap(8.0)
            .align(Center),
        )
}

/// A link under its bubble, opened by a click.
fn link_chip(link: &str) -> Rectangle {
    let key = format!("messages.link.{link}");
    let shown = links::shown(link);
    let shown = if shown.chars().count() > 48 {
        format!("{}…", shown.chars().take(47).collect::<String>())
    } else {
        shown
    };
    let style = sys::r#type::caption();
    let width = 10.0 + 16.0 + 6.0 + text_width(&shown, style) + 12.0;
    let url = links::url(link);
    interactive(
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(26.0))
            .radius(Radius::Full)
            .fill(state_fill(
                &key,
                with_alpha(style::label_primary(), 0.06),
                false,
            ))
            .padding(pad(10.0, 0.0))
            .align_child(Start, Center)
            .child(
                Row::new(vec![
                    boxed(amane_kit::layout::icon(
                        ICON_LINK,
                        16.0,
                        sys::color::accent::bg(),
                    )),
                    boxed(
                        Text::new(shown)
                            .style(style)
                            .color(sys::color::accent::bg()),
                    ),
                ])
                .gap(6.0)
                .align(Center),
            ),
        key,
        false,
        Actions {
            press: Some(Rc::new(move || {
                amane_kit::launch::open(&url);
            })),
            ..Actions::default()
        },
    )
}

/// Under a run: when it was sent, or that it's still sending, or failed
/// with a button to send it again.
fn stamp(message: &SmsMessage, beside_avatar: bool) -> Rectangle {
    let height = text_height(sys::r#type::caption()) + 4.0;
    let inset = if beside_avatar {
        SMALL_AVATAR + sys::space::gap::s() + STAMP_INSET
    } else {
        STAMP_INSET
    };
    let row = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(height))
        .padding(pad(inset, 0.0));
    let row = if message.outgoing() {
        row.align_child(End, Center)
    } else {
        row.align_child(Start, Center)
    };

    if message.kind() == SmsKind::Failed {
        let key = format!("messages.retry.{}", message.uid());
        let label = "Not Delivered · Send Again";
        let style = sys::r#type::caption();
        let failed = message.clone();
        return row.child(interactive(
            Rectangle::new()
                .width(Size::Fixed(text_width(label, style) + 8.0))
                .height(Size::Fixed(height))
                .radius(Radius::Fixed(4.0))
                .fill(state_fill(&key, Color::TRANSPARENT, false))
                .align_child(Center, Center)
                .child(
                    Text::new(label)
                        .style(style)
                        .color(sys::color::status::critical::text()),
                ),
            key,
            false,
            Actions {
                press: Some(Rc::new(move || retry(&failed))),
                ..Actions::default()
            },
        ));
    }

    let (text, color) = if message.pending() {
        (String::from("Sending…"), material::label(Level::Secondary))
    } else {
        (
            time::clock(message.date()),
            material::label(Level::Tertiary),
        )
    };
    row.child(Text::new(text).style(sys::r#type::caption()).color(color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_is_named_for_its_people() {
        let names = |list: &[&str]| list.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(
            group_title(&names(&["Ola Nordmann", "Kari"])),
            "Ola and Kari"
        );
        assert_eq!(
            group_title(&names(&["Ola Nordmann", "Kari", "+4795723015", "Per"])),
            "Ola, Kari and 2 others"
        );
    }
}
