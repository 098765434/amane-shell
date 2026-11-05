//! Messages: the phone's text messages and its remote input, through KDE
//! Connect, in the shell's look; KDE's own `kdeconnect-sms` never shows.
//!
//! The sidebar is the inbox, newest conversation first, with Phone Input
//! under it; the content is the open conversation with a reply field, or
//! the trackpad and keyboard that drive the phone.
//!
//! `amane-messages --device <id> --view messages|input` opens on a phone
//! and a view (Quick Settings' Phone page does); launched again while it
//! runs, it switches to them. Without a device it picks the first connected
//! phone that has messages.

mod bubble;
mod inbox;
mod input;
mod thread;
mod time;

use amane::{
    Column, KdeConnect, Key, PhoneDevice, PhoneMessages, Row, Service, Size, Text, Widget, Window,
};
use amane_kit::app::{app, app_window, sidebar_panel, standard_shortcut};
use amane_kit::layout::{boxed, caption};
use amane_kit::macform::theme as mac;
use amane_kit::tokens::{Styled, sys};

/// How wide the inbox is.
pub const SIDEBAR_WIDTH: f32 = 280.0;

/// What the content shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// A conversation, by its thread id; none before one is picked.
    Thread(Option<i64>),
    /// The trackpad and keyboard.
    Input,
}

pub struct Navigation {
    /// The phone, by its KDE Connect id: the one asked for, else the first
    /// connected one with messages.
    device: Option<String>,
    pub view: View,
}

impl Service for Navigation {
    fn new() -> Self {
        Self {
            device: None,
            view: View::Thread(None),
        }
    }

    // only clicks and launches change it
    fn listen() {}
}

impl Navigation {
    pub fn show(view: View) {
        Navigation::write().view = view;
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

fn flag(args: &[String], name: &str) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
        if arg == name {
            return args.next().cloned();
        }
    }
    None
}

/// Takes `--device` and `--view` from a launch.
fn follow(args: &[String]) {
    let mut nav = Navigation::write();
    if let Some(device) = flag(args, "--device") {
        nav.device = Some(device);
    }
    match flag(args, "--view").as_deref() {
        Some("input") => nav.view = View::Input,
        Some("messages") if nav.view == View::Input => nav.view = View::Thread(None),
        _ => {}
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    follow(&args);
    app("amane-messages")
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

    let view = Navigation::read().view;

    let content: Box<dyn Widget> = match (&device, view) {
        (None, _) => boxed(no_phone()),
        (Some(device), View::Input) => boxed(input::view(device)),
        (Some(device), View::Thread(thread)) => boxed(thread::view(device, thread)),
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
        .title("Messages")
        .size(860.0, 640.0)
        .min_size(640.0, 420.0)
        .on_shortcut(standard_shortcut)
        .on_key(|key: Key| {
            if Navigation::read().view == View::Input {
                input::key(key);
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
        .style(mac::page_title_font())
        .color(mac::label_primary())
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
