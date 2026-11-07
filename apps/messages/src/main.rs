//! Messages: the phone's text messages and its remote input, through KDE
//! Connect, in the shell's look; KDE's own `kdeconnect-sms` never shows.
//!
//! The sidebar is the inbox, newest conversation first, with Phone Input
//! under it; the content is the open conversation with a reply field, or
//! the trackpad and keyboard that drive the phone.
//!
//! `amane-messages --device <id> --view messages|input|new` opens on a
//! phone and a view (Quick Settings' Phone page does), and `--thread-of
//! <name or number>` on that person's conversation (a notification's Open
//! in Messages does); launched again while it runs, it switches to them.
//! Without a device it picks the first connected phone that has messages.
//!
//! Ctrl+N starts a new message, Ctrl+F searches, Alt+Up and Alt+Down (or
//! Ctrl+Tab) move through the conversations, Escape leaves the search or
//! the new message.

mod bubble;
mod compose;
mod inbox;
mod input;
mod links;
mod new_message;
mod phone;
mod read;
mod thread;
mod time;

use amane::{Column, Key, Row, Service, Size, Text, TextInput, Widget, Window};
use amane_kit::app::{app, app_window, sidebar_panel, standard_shortcut};
use amane_kit::args::flag;
use amane_kit::layout::{boxed, caption};
use amane_kit::style;
use amane_kit::tokens::{Styled, sys};
use amane_phone::{KdeConnect, PhoneDevice, PhoneMessages};

/// How wide the inbox is.
pub const SIDEBAR_WIDTH: f32 = 280.0;

/// What the content shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// A conversation, by its thread id; none before one is picked.
    Thread(Option<i64>),
    /// The trackpad and keyboard.
    Input,
    /// A message to someone new.
    New,
}

pub struct Navigation {
    /// The phone, by its KDE Connect id: the one asked for, else the first
    /// connected one with messages.
    device: Option<String>,
    pub view: View,
    /// A conversation asked for by its person's name or number, opened
    /// once the phone has sent it.
    wanted: Option<String>,
}

impl Service for Navigation {
    fn new() -> Self {
        Self {
            device: None,
            view: View::Thread(None),
            wanted: None,
        }
    }

    // only clicks and launches change it
    fn listen() {}
}

impl Navigation {
    pub fn show(view: View) {
        let before = Navigation::read().view;
        if view != View::New && before == View::New {
            new_message::reset();
        }
        if view == View::New && before != View::New {
            new_message::focus();
        }
        Navigation::write().view = view;
    }
}

/// The conversation with `wanted` (its title, or a number in it), once
/// it's there.
fn find_thread(wanted: &str) -> Option<i64> {
    let messages = PhoneMessages::read();
    let digits: String = wanted.chars().filter(char::is_ascii_digit).collect();
    let threads = messages.threads();
    threads
        .iter()
        .find(|thread| thread.title() == wanted)
        .or_else(|| {
            threads
                .iter()
                .find(|thread| thread.title().eq_ignore_ascii_case(wanted))
        })
        .or_else(|| {
            (digits.len() >= 7).then(|| {
                threads.iter().find(|thread| {
                    thread.addresses().iter().any(|address| {
                        let own: String = address.chars().filter(char::is_ascii_digit).collect();
                        own.ends_with(&digits) || digits.ends_with(&own)
                    })
                })
            })?
        })
        .map(amane_phone::SmsThread::id)
}

/// Moves `step` conversations down the list (up when negative).
fn step_thread(step: isize) {
    let ids = inbox::shown_ids();
    if ids.is_empty() {
        return;
    }
    let now = match Navigation::read().view {
        View::Thread(Some(id)) => ids.iter().position(|shown| *shown == id),
        _ => None,
    };
    let next = match now {
        Some(at) => (at as isize + step).clamp(0, ids.len() as isize - 1) as usize,
        None if step < 0 => ids.len() - 1,
        None => 0,
    };
    Navigation::show(View::Thread(Some(ids[next])));
}

/// The window's shortcuts, after the ones every window has.
fn shortcut(key: Key) -> bool {
    if standard_shortcut(key) {
        return true;
    }
    let held = amane::modifiers();
    match key {
        Key::Character('n') if held.ctrl => {
            Navigation::show(View::New);
            true
        }
        Key::Character('f') if held.ctrl => {
            if matches!(Navigation::read().view, View::Input) {
                Navigation::show(View::Thread(None));
            }
            inbox::focus_search();
            true
        }
        Key::Tab if held.ctrl => {
            step_thread(if held.shift { -1 } else { 1 });
            true
        }
        Key::Up if held.alt => {
            step_thread(-1);
            true
        }
        Key::Down if held.alt => {
            step_thread(1);
            true
        }
        _ => false,
    }
}

/// The phone shown: the one asked for while it's paired, else the first
/// connected one that has messages.
pub fn device() -> Option<PhoneDevice> {
    let asked = Navigation::read().device.clone();
    let kdeconnect = KdeConnect::read();
    asked
        .and_then(|id| kdeconnect.device(&id).filter(|d| d.paired()).cloned())
        .or_else(|| {
            kdeconnect
                .devices()
                .iter()
                .find(|device| device.connected() && device.can_message())
                .cloned()
        })
}

/// Takes `--device` and `--view` from a launch.
fn follow(args: &[String]) {
    let mut nav = Navigation::write();
    if let Some(device) = flag(args, "--device") {
        nav.device = Some(device);
    }
    match flag(args, "--view").as_deref() {
        Some("input") => nav.view = View::Input,
        Some("new") => {
            if nav.view != View::New {
                new_message::focus();
            }
            nav.view = View::New;
        }
        Some("messages") if matches!(nav.view, View::Input | View::New) => {
            nav.view = View::Thread(None);
        }
        _ => {}
    }
    if let Some(wanted) = flag(args, "--thread-of") {
        nav.wanted = Some(wanted);
    }
}

fn main() {
    amane_log::init(amane_kit::app_ids::MESSAGES);
    let args: Vec<String> = std::env::args().skip(1).collect();
    follow(&args);
    app(amane_kit::app_ids::MESSAGES)
        .normal_window("main", main_window)
        .on_activate(follow)
        .run();
}

fn main_window() -> Window {
    let device = device();
    // the phone's messages are read once it's known, and again for another phone
    if let Some(device) = &device
        && device.can_message()
        && PhoneMessages::read().device() != Some(device.id())
    {
        PhoneMessages::open(device.id());
    }

    // a conversation asked for by name opens once the phone has sent it
    let wanted = Navigation::read().wanted.clone();
    if let Some(wanted) = wanted
        && let Some(thread) = find_thread(&wanted)
    {
        let mut nav = Navigation::write();
        nav.wanted = None;
        nav.view = View::Thread(Some(thread));
    }

    // a conversation started here follows the phone's thread it became
    if let View::Thread(Some(id)) = Navigation::read().view
        && id < 0
        && let Some(moved) = PhoneMessages::read().moved(id)
    {
        Navigation::write().view = View::Thread(Some(moved));
    }

    let view = Navigation::read().view;

    let content: Box<dyn Widget> = match (&device, view) {
        (None, _) => boxed(no_phone()),
        (Some(device), View::Input) => boxed(input::view(device)),
        (Some(_), View::New) => boxed(new_message::view()),
        (Some(device), View::Thread(thread)) => boxed(thread::view(device, thread)),
    };

    let window_title = match view {
        View::Thread(Some(id)) => PhoneMessages::read().thread(id).map_or_else(
            || String::from("Messages"),
            |thread| thread.title().to_string(),
        ),
        View::New => String::from("New Message"),
        View::Input => String::from("Phone Input"),
        View::Thread(None) => String::from("Messages"),
    };

    let layout = Row::new(vec![
        boxed(
            Column::new(vec![boxed(sidebar_panel(inbox::view(
                device.as_ref(),
                view,
            )))])
            .width(Size::Fixed(SIDEBAR_WIDTH))
            .height(Size::Parent),
        ),
        boxed(
            Column::new(vec![content])
                .width(Size::Parent)
                .height(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::xl())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(layout)
        .title(&window_title)
        .size(900.0, 660.0)
        .min_size(640.0, 420.0)
        .on_shortcut(shortcut)
        .on_key(|key: Key| {
            let view = Navigation::read().view;
            if view == View::Input {
                input::key(key);
                return;
            }
            if key == Key::Escape {
                if !TextInput::text(inbox::SEARCH).is_empty() {
                    TextInput::set_text(inbox::SEARCH, "");
                    amane::mark_all();
                } else if view == View::New {
                    Navigation::show(View::Thread(None));
                }
            }
        })
}

/// How wide the content is: the window less its insets, the inbox and the
/// gap between.
pub fn content_width() -> f32 {
    let (width, _) = amane::window_size();
    (width - 2.0 * sys::space::inset::l() - SIDEBAR_WIDTH - sys::space::gap::xl()).max(1.0)
}

/// A view's title, in the page title's type.
pub fn title(text: impl Into<String>) -> Text {
    Text::new(text)
        .style(style::page_title_font())
        .color(style::label_primary())
        .elide()
}

/// No phone to show: why, and what to do.
fn no_phone() -> Column {
    let running = KdeConnect::read().running();
    let note = if running {
        "Pair a phone in Quick Settings → Phone, and open KDE Connect on it."
    } else {
        "KDE Connect isn't running. Turn it on in Quick Settings → Phone."
    };
    Column::new(vec![
        boxed(title("No Phone Connected")),
        boxed(caption(note)),
    ])
    .gap(sys::space::gap::s())
    .width(Size::Parent)
}
