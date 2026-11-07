//! The field a message is written in, under a conversation or a new
//! message: many lines (Enter sends, Shift+Enter starts a new line), a
//! button that attaches files, and the files attached as chips over it.
//! Each conversation keeps its own half-written text and files while
//! another is shown.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use amane::{
    Center, Color, Column, FileDialog, Radius, Rectangle, Row, ScrollArea, Size, Text, TextInput,
    Widget,
};
use amane_kit::app::text_field;
use amane_kit::buttons::{Actions, IconButton, PushButton, Style, interactive, state_fill};
use amane_kit::color::with_alpha;
use amane_kit::control_size::ControlSize;
use amane_kit::layout::{boxed, caption, fixed_width, icon, pad, text_width};
use amane_kit::material::{self, Level};
use amane_kit::style;
use amane_kit::tokens::{Styled, sys};

/// The field's id, which keeps its text.
pub const FIELD: &str = "messages.compose";

/// `attach_file`, `close` and `description`.
const ICON_ATTACH: char = '\u{e226}';
const ICON_CLOSE: char = '\u{e5cd}';
const ICON_FILE: char = '\u{e873}';
const ICON_IMAGE: char = '\u{e3f4}';

/// How many lines the field grows to before it scrolls.
const MOST_LINES: usize = 5;

/// Bigger than this, a carrier may refuse the message (MMS limits).
const BIG: u64 = 1024 * 1024;

/// Where a message goes: a conversation, or the new one being written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Draft {
    Thread(i64),
    New,
}

thread_local! {
    // the half-written text of each draft not shown
    static TEXTS: RefCell<HashMap<Draft, String>> = RefCell::new(HashMap::new());

    // the files attached to each draft
    static FILES: RefCell<HashMap<Draft, Vec<PathBuf>>> = RefCell::new(HashMap::new());

    // the draft the field holds now
    static SHOWN: Cell<Option<Draft>> = const { Cell::new(None) };

    // the field is to take the keys when next built, until it has them
    static WANTS_FOCUS: Cell<bool> = const { Cell::new(true) };
}

/// The field holds `draft`'s text: the one it held is put away first.
fn show(draft: Draft) {
    let before = SHOWN.replace(Some(draft));
    if before == Some(draft) {
        return;
    }
    if let Some(before) = before {
        let text = TextInput::text(FIELD);
        TEXTS.with_borrow_mut(|texts| texts.insert(before, text));
    }
    let text = TEXTS.with_borrow_mut(|texts| texts.remove(&draft).unwrap_or_default());
    TextInput::set_text(FIELD, &text);
    WANTS_FOCUS.set(true);
}

/// Asks for the field to take the keys.
pub fn focus() {
    WANTS_FOCUS.set(true);
}

pub fn files(draft: Draft) -> Vec<PathBuf> {
    FILES.with_borrow(|files| files.get(&draft).cloned().unwrap_or_default())
}

/// Attaches `paths` to `draft`.
pub fn attach(draft: Draft, paths: Vec<PathBuf>) {
    FILES.with_borrow_mut(|files| {
        let attached = files.entry(draft).or_default();
        for path in paths {
            if path.is_file() && !attached.contains(&path) {
                attached.push(path);
            }
        }
    });
    amane::mark_all();
}

fn detach(draft: Draft, path: &PathBuf) {
    FILES.with_borrow_mut(|files| {
        if let Some(attached) = files.get_mut(&draft) {
            attached.retain(|old| old != path);
        }
    });
    amane::mark_all();
}

/// Empties `draft` once it's sent.
fn clear(draft: Draft) {
    FILES.with_borrow_mut(|files| files.remove(&draft));
    TEXTS.with_borrow_mut(|texts| texts.remove(&draft));
    if SHOWN.get() == Some(draft) {
        TextInput::set_text(FIELD, "");
    }
}

/// The field for `draft`, `width` across, with `send` taking the text and
/// files when Enter or Send is pressed; `send` says whether it went.
pub fn view(
    draft: Draft,
    placeholder: &str,
    width: f32,
    send: impl Fn(&str, &[PathBuf]) -> bool + 'static,
) -> Column {
    show(draft);

    let send = Rc::new(move || {
        let text = TextInput::text(FIELD);
        let attached = files(draft);
        if (text.trim().is_empty() && attached.is_empty()) || !send(text.trim(), &attached) {
            return;
        }
        clear(draft);
    });

    let attach_button = IconButton::new("messages.compose.attach", ICON_ATTACH)
        .size(sys::size::icon::s())
        .height(ControlSize::Large.height())
        .width(ControlSize::Large.height())
        .on_press(move || {
            FileDialog::open("Attach to the Message")
                .multiple(true)
                .show(move |paths| attach(draft, paths));
        })
        .tooltip("main", "Attach Files")
        .build();

    let pressed = send.clone();
    let send_button = PushButton::new("messages.send", "Send")
        .style(Style::Filled)
        .on_press(move || pressed())
        .build();

    let field_width = (width
        - fixed_width(&attach_button)
        - fixed_width(&send_button)
        - 2.0 * sys::space::gap::s())
    .max(1.0);

    let focused = WANTS_FOCUS.get();
    if TextInput::has_focus(FIELD) {
        WANTS_FOCUS.set(false);
    }

    let submitted = send.clone();
    let mut input = text_field(FIELD, placeholder)
        .multiline(true)
        .on_submit(move |_| submitted());
    if focused {
        input = input.focused();
    }

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    let attached = files(draft);
    if !attached.is_empty() {
        parts.push(boxed(chips(draft, &attached, width)));
    }
    parts.push(boxed(
        Row::new(vec![
            boxed(attach_button),
            boxed(frame(input, field_width)),
            boxed(send_button),
        ])
        .gap(sys::space::gap::s())
        .width(Size::Parent)
        .align(amane::End),
    ));

    Column::new(parts)
        .gap(sys::space::gap::s())
        .width(Size::Parent)
}

/// The field's rounded box, as tall as its lines up to `MOST_LINES`, then
/// scrolling; ringed in the accent while it has the keys.
fn frame(input: TextInput, width: f32) -> Rectangle {
    let lines = TextInput::text(FIELD).matches('\n').count() + 1;
    let line = match Widget::height(&input) {
        Size::Fixed(height) => height / lines as f32,
        Size::Parent => 18.0,
    };
    let inset = ((ControlSize::Large.height() - line) / 2.0).max(4.0);
    let shown = lines.min(MOST_LINES) as f32 * line;

    let inner: Box<dyn Widget> = if lines > MOST_LINES {
        boxed(
            ScrollArea::new("messages.compose.scroll", input)
                .width(Size::Parent)
                .height(Size::Fixed(shown)),
        )
    } else {
        boxed(input)
    };

    let edge = if TextInput::has_focus(FIELD) {
        style::tint()
    } else {
        style::field_border()
    };

    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(shown + 2.0 * inset))
        .radius(Radius::Fixed((line / 2.0 + inset).min(18.0)))
        .fill(style::field_fill())
        .border(1.0, edge)
        .padding(pad(12.0, inset))
        .align_child(amane::Start, Center)
        .clip()
        .child(Column::new(vec![inner]).width(Size::Parent))
}

/// The files attached, each with its name and a button to take it off.
fn chips(draft: Draft, attached: &[PathBuf], width: f32) -> Row {
    let mut chips: Vec<Box<dyn Widget>> = Vec::new();
    let mut used = 0.0;
    for path in attached {
        let chip = chip(draft, path);
        used += fixed_width(&chip) + sys::space::gap::s();
        if used > width && !chips.is_empty() {
            chips.push(boxed(caption(format!(
                "and {} more",
                attached.len() - chips.len()
            ))));
            break;
        }
        chips.push(boxed(chip));
    }
    Row::new(chips)
        .gap(sys::space::gap::s())
        .width(Size::Parent)
        .align(Center)
}

fn chip(draft: Draft, path: &PathBuf) -> Rectangle {
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let name = if name.chars().count() > 28 {
        format!("{}…", name.chars().take(27).collect::<String>())
    } else {
        name
    };
    let big = std::fs::metadata(path).is_ok_and(|meta| meta.len() > BIG);
    let image = matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "heic")
    );

    let style = sys::r#type::caption();
    let label = if big {
        format!("{name} · may be too big to send")
    } else {
        name
    };
    let label_width = text_width(&label, style);
    let key = format!("messages.compose.detach.{}", path.display());

    let removed = path.clone();
    let remove = interactive(
        Rectangle::new()
            .width(Size::Fixed(18.0))
            .height(Size::Fixed(18.0))
            .radius(Radius::Full)
            .fill(state_fill(&key, Color::TRANSPARENT, false))
            .align_child(Center, Center)
            .child(icon(ICON_CLOSE, 14.0, material::label(Level::Secondary))),
        key,
        false,
        Actions {
            press: Some(Rc::new(move || detach(draft, &removed))),
            ..Actions::default()
        },
    );

    let width = 8.0 + 16.0 + 6.0 + label_width + 4.0 + 18.0 + 6.0;
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(28.0))
        .radius(Radius::Full)
        .fill(with_alpha(style::label_primary(), 0.07))
        .padding(pad(8.0, 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![
                boxed(icon(
                    if image { ICON_IMAGE } else { ICON_FILE },
                    16.0,
                    material::label(Level::Secondary),
                )),
                boxed(Text::new(label).style(style).color(if big {
                    sys::color::status::critical::text()
                } else {
                    style::label_primary()
                })),
                boxed(remove),
            ])
            .gap(6.0)
            .align(Center),
        )
}
