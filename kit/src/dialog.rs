//! A question answered with one of its buttons, in macOS 26's alert: a
//! narrow card of thick glass with an optional icon, a bold title, a
//! message, and its buttons side by side or stacked.
//!
//! ```ignore
//! Dialog::new("settings.discard", "Discard changes?")
//!     .message("Your changes will be lost.")
//!     .button("Cancel", || {})
//!     .destructive("Discard", discard)
//!     .open("main");
//! ```
//!
//! The same [`Dialog`] shows in two places: [`Dialog::open`] puts it in a
//! modal window over an app's window, and the shell's alert puts it on a
//! layer surface of its own. Both draw [`Dialog::content`] on their glass.
//!
//! Its buttons sit side by side when there are at most two and their labels
//! fit, and stack otherwise, as macOS's do; [`Dialog::stacked`] and
//! [`Dialog::side_by_side`] choose for it. Return answers with the default
//! button, Escape with the cancel button.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use amane::{
    Center, Color, Column, Image, Key, Padding, Rectangle, Row, Size, Start, Text, Widget, Window,
    close_window, open_window_with,
};

use crate::app::{field_box, frosted, standard_shortcut, text_field, window_bg};
use crate::buttons::{PushButton, Shape, Style};
use crate::layout::{boxed, icon, text_width};
use crate::material::{self, Level, Material};
use crate::tokens::{Styled, dialog, sys};

/// What a button does to the question, which decides its look and place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Backs out: Escape answers with it.
    Cancel,
    /// The answer the dialog expects, in the accent: Return answers with it.
    Default,
    /// An answer that destroys or can't be undone, in red.
    Destructive,
}

impl Role {
    fn style(self) -> Style {
        match self {
            Role::Cancel => Style::Tonal,
            Role::Default => Style::Filled,
            Role::Destructive => Style::Destructive,
        }
    }

    /// Where it goes: side by side the default ends the row, on the right;
    /// stacked it leads, on top, with cancel at the bottom.
    fn rank(self, stacked: bool) -> u8 {
        match (self, stacked) {
            (Role::Cancel, false) | (Role::Default, true) => 0,
            (Role::Destructive, _) => 1,
            (Role::Default, false) | (Role::Cancel, true) => 2,
        }
    }
}

/// How the buttons are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Layout {
    /// Side by side when they fit, else stacked.
    #[default]
    Auto,
    SideBySide,
    Stacked,
}

/// What shows above the title.
#[derive(Clone, Debug)]
pub enum Icon {
    /// An app's icon file.
    App(PathBuf),
    /// A Material Symbols glyph.
    Symbol(char),
}

#[derive(Clone)]
struct Answer {
    label: String,
    role: Role,
    action: Rc<dyn Fn()>,
}

/// A line of text under the message, in its own colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detail {
    text: String,
    error: bool,
}

/// A text field between the message and the buttons, for a dialog that
/// asks for a word: a password, a name.
#[derive(Clone)]
pub struct Field {
    /// The input's id, under which its text is kept: read it with
    /// `TextInput::text(id)` from a button's action.
    pub id: &'static str,
    pub placeholder: String,
    /// Hide what's typed, as a password.
    pub secret: bool,
}

#[derive(Clone)]
pub struct Dialog {
    name: String,
    title: String,
    message: String,
    details: Vec<Detail>,
    field: Option<Field>,
    icon: Option<Icon>,
    answers: Vec<Answer>,
    layout: Layout,
}

thread_local! {
    // each open app dialog, which its window's view reads every frame
    static OPEN: RefCell<HashMap<String, Dialog>> = RefCell::new(HashMap::new());
}

impl Dialog {
    /// `name` keys its window and its buttons, so it must be unique.
    pub fn new(name: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            message: String::new(),
            details: Vec::new(),
            field: None,
            icon: None,
            answers: Vec::new(),
            layout: Layout::Auto,
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// A quieter line under the message, like what a reader is waiting for.
    pub fn note(mut self, text: impl Into<String>) -> Self {
        self.details.push(Detail {
            text: text.into(),
            error: false,
        });
        self
    }

    /// A line under the message in the critical colour: what went wrong.
    pub fn error(mut self, text: impl Into<String>) -> Self {
        self.details.push(Detail {
            text: text.into(),
            error: true,
        });
        self
    }

    /// A text field under the message, which takes the keys while the
    /// dialog shows. Return still answers with the default button, whose
    /// action reads the field's text by its id.
    pub fn field(mut self, field: Field) -> Self {
        self.field = Some(field);
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A button that backs out ("Cancel"): it closes the dialog and runs
    /// `action`.
    pub fn button(self, label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.answer(label, Role::Cancel, action)
    }

    /// The one main answer, in the accent.
    pub fn main_button(self, label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.answer(label, Role::Default, action)
    }

    /// An answer that destroys or can't be undone, in red.
    pub fn destructive(self, label: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.answer(label, Role::Destructive, action)
    }

    fn answer(mut self, label: impl Into<String>, role: Role, action: impl Fn() + 'static) -> Self {
        self.answers.push(Answer {
            label: label.into(),
            role,
            action: Rc::new(action),
        });
        self
    }

    /// Buttons one above the other, whatever fits.
    pub fn stacked(mut self) -> Self {
        self.layout = Layout::Stacked;
        self
    }

    /// Buttons in one row, whatever fits.
    pub fn side_by_side(mut self) -> Self {
        self.layout = Layout::SideBySide;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Opens it in a modal window over the app window with this name.
    pub fn open(self, parent: &str) {
        let name = self.name.clone();
        OPEN.with_borrow_mut(|open| {
            open.insert(name.clone(), self);
        });

        let parent = parent.to_string();
        let key = name.clone();
        open_window_with(name, move || window(&key, &parent));
    }

    /// How wide every dialog is.
    pub fn width() -> f32 {
        dialog::width()
    }

    /// How tall this one is, with its text wrapped to fit.
    pub fn height(&self) -> f32 {
        let mut height = dialog::padding_top() + dialog::padding_x();
        if self.icon.is_some() {
            height += dialog::icon_size() + dialog::gap();
        }
        height += self.text_height();
        if self.field.is_some() {
            height += dialog::gap() + sys::size::control::height();
        }
        if !self.answers.is_empty() {
            height += dialog::gap() + self.buttons_height();
        }
        height
    }

    /// Whether the buttons stack, one above the other.
    pub fn is_stacked(&self) -> bool {
        match self.layout {
            Layout::Stacked => true,
            Layout::SideBySide => false,
            Layout::Auto => {
                let count = self.answers.len();
                if count > 2 {
                    return true;
                }
                let room = side_by_side_width(count) - dialog::button::inset() * 2.0;
                self.answers
                    .iter()
                    .any(|answer| text_width(&answer.label, dialog::button::label()) > room)
            }
        }
    }

    /// The card's content, `width()` by `height()`, on no fill: the glass
    /// is the caller's. A button press runs `close`, then its answer.
    pub fn content(&self, close: Rc<dyn Fn()>) -> Rectangle {
        material::scope(Material::Sheet, || self.build(close))
    }

    /// What a key does: Return answers with the default button, Escape with
    /// the cancel button (or just closes). True when it used the key.
    pub fn key(&self, key: Key, close: &Rc<dyn Fn()>) -> bool {
        let wanted = match key {
            Key::Enter => Role::Default,
            Key::Escape => Role::Cancel,
            _ => return false,
        };
        match self.answers.iter().find(|answer| answer.role == wanted) {
            Some(answer) => {
                close();
                (answer.action)();
            }
            None if wanted == Role::Cancel => close(),
            None => return false,
        }
        true
    }

    fn text_height(&self) -> f32 {
        let width = text_column_width();
        let mut height = title(&self.title, Color::TRANSPARENT).height_in(width);
        if !self.message.is_empty() {
            height +=
                dialog::text::gap() + message(&self.message, Color::TRANSPARENT).height_in(width);
        }
        for detail in &self.details {
            height +=
                dialog::text::gap() + message(&detail.text, Color::TRANSPARENT).height_in(width);
        }
        height + dialog::text::inset_bottom()
    }

    fn buttons_height(&self) -> f32 {
        let count = self.answers.len() as f32;
        if self.is_stacked() {
            dialog::button::height_stacked() * count
                + dialog::button::gap_stacked() * (count - 1.0).max(0.0)
        } else {
            dialog::button::height()
        }
    }

    /// The answers in the order they're shown, with their place in the
    /// list they were given in, which keys their buttons.
    fn ordered(&self) -> Vec<(usize, &Answer)> {
        let stacked = self.is_stacked();
        let mut ordered: Vec<(usize, &Answer)> = self.answers.iter().enumerate().collect();
        ordered.sort_by_key(|(_, answer)| answer.role.rank(stacked));
        ordered
    }

    fn build(&self, close: Rc<dyn Fn()>) -> Rectangle {
        let fg = material::label(Level::Primary);
        let inset = dialog::text::inset();

        let mut rows: Vec<Box<dyn Widget>> = Vec::new();

        if let Some(shown) = &self.icon {
            let side = dialog::icon_size();
            let slot = Rectangle::new()
                .width(Size::Fixed(side))
                .height(Size::Fixed(side));
            let glyph = match shown {
                Icon::App(path) if Image::loaded(path) => slot.fill(Image::contain(path.clone())),
                // an icon still loading leaves its room
                Icon::App(_) => slot.fill(Color::TRANSPARENT),
                Icon::Symbol(glyph) => slot
                    .fill(Color::TRANSPARENT)
                    .align_child(Center, Center)
                    .child(icon(*glyph, side, fg)),
            };
            rows.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(inner_width()))
                    .height(Size::Fixed(side))
                    .fill(Color::TRANSPARENT)
                    .padding(Padding {
                        top: 0.0,
                        right: inset,
                        bottom: 0.0,
                        left: inset,
                    })
                    .align_child(Start, Center)
                    .child(glyph),
            ));
        }

        let mut text: Vec<Box<dyn Widget>> = vec![boxed(title(&self.title, fg))];
        if !self.message.is_empty() {
            text.push(boxed(message(&self.message, fg)));
        }
        for detail in &self.details {
            let color = if detail.error {
                sys::color::status::critical::text()
            } else {
                material::label(Level::Secondary)
            };
            text.push(boxed(message(&detail.text, color)));
        }
        rows.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(inner_width()))
                .height(Size::Fixed(self.text_height()))
                .fill(Color::TRANSPARENT)
                .padding(Padding {
                    top: 0.0,
                    right: inset,
                    bottom: dialog::text::inset_bottom(),
                    left: inset,
                })
                .child(Column::new(text).gap(dialog::text::gap())),
        ));

        if let Some(field) = &self.field {
            let mut input = text_field(field.id, &field.placeholder).focused();
            if field.secret {
                input = input.password();
            }
            // the field takes Return, so it answers with the default button itself
            if let Some(default) = self.answers.iter().find(|a| a.role == Role::Default) {
                let close = Rc::clone(&close);
                let action = Rc::clone(&default.action);
                input = input.on_submit(move |_| {
                    close();
                    action();
                });
            }
            rows.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(inner_width()))
                    .height(Size::Fixed(sys::size::control::height()))
                    .fill(Color::TRANSPARENT)
                    .padding(Padding {
                        top: 0.0,
                        right: inset,
                        bottom: 0.0,
                        left: inset,
                    })
                    .child(field_box(field.id, input, inner_width() - inset * 2.0)),
            ));
        }

        if !self.answers.is_empty() {
            rows.push(self.buttons(close));
        }

        Rectangle::new()
            .width(Size::Fixed(Self::width()))
            .height(Size::Fixed(self.height()))
            .fill(Color::TRANSPARENT)
            .padding(Padding {
                top: dialog::padding_top(),
                right: dialog::padding_x(),
                bottom: dialog::padding_x(),
                left: dialog::padding_x(),
            })
            .child(Column::new(rows).gap(dialog::gap()))
    }

    fn buttons(&self, close: Rc<dyn Fn()>) -> Box<dyn Widget> {
        let stacked = self.is_stacked();
        let (width, height) = if stacked {
            (inner_width(), dialog::button::height_stacked())
        } else {
            (
                side_by_side_width(self.answers.len()),
                dialog::button::height(),
            )
        };

        let buttons: Vec<Box<dyn Widget>> = self
            .ordered()
            .into_iter()
            .map(|(index, answer)| {
                let close = Rc::clone(&close);
                let action = Rc::clone(&answer.action);
                boxed(
                    PushButton::new(format!("{}.button.{index}", self.name), &answer.label)
                        .style(answer.role.style())
                        .shape(Shape::Round)
                        .text(dialog::button::label())
                        .width(width)
                        .height(height)
                        .on_press(move || {
                            close();
                            action();
                        })
                        .build(),
                )
            })
            .collect();

        if stacked {
            boxed(Column::new(buttons).gap(dialog::button::gap_stacked()))
        } else {
            boxed(Row::new(buttons).gap(dialog::button::gap()))
        }
    }
}

/// The width inside the card's padding, which the buttons span.
fn inner_width() -> f32 {
    Dialog::width() - dialog::padding_x() * 2.0
}

/// The width the title and message wrap at.
fn text_column_width() -> f32 {
    inner_width() - dialog::text::inset() * 2.0
}

/// Each button's width when `count` share the row.
fn side_by_side_width(count: usize) -> f32 {
    let count = count.max(1) as f32;
    (inner_width() - dialog::button::gap() * (count - 1.0)) / count
}

fn title(text: &str, color: Color) -> Text {
    Text::new(text).style(dialog::title()).color(color).wrap()
}

fn message(text: &str, color: Color) -> Text {
    Text::new(text).style(dialog::message()).color(color).wrap()
}

/// The app dialog's window: the card on the dialog's glass, where niri
/// draws it (its generated window rule frosts a floating amane window in
/// thick glass), else on the window background.
fn window(name: &str, parent: &str) -> Window {
    let shown = OPEN
        .with_borrow(|open| open.get(name).cloned())
        .unwrap_or_else(|| Dialog::new(name, ""));

    let closing = name.to_string();
    let close: Rc<dyn Fn()> = Rc::new(move || {
        close_window(&closing);
        OPEN.with_borrow_mut(|open| open.remove(&closing));
    });

    let radius = dialog::radius();
    let glass = frosted();
    let card = Rectangle::new()
        .width(Size::Parent)
        .height(Size::Parent)
        .radius(radius)
        .fill(if glass {
            Material::Sheet.fill()
        } else {
            window_bg()
        })
        .child(shown.content(Rc::clone(&close)));
    let card = if glass {
        Material::Sheet.lit(card, radius)
    } else {
        card
    };

    let closed = name.to_string();
    let keys = shown.clone();
    Window::new()
        .vibrant(material::vibrant())
        .title(shown.title())
        .size(Dialog::width(), shown.height())
        .resizable(false)
        .parent(parent)
        .modal(true)
        // closing it from the compositor is the same as no answer
        .on_close(move || {
            OPEN.with_borrow_mut(|open| open.remove(&closed));
            true
        })
        .on_key(move |key| {
            keys.key(key, &close);
        })
        .on_shortcut(standard_shortcut)
        .child(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(dialog: &Dialog) -> Vec<&str> {
        dialog
            .ordered()
            .into_iter()
            .map(|(_, answer)| answer.label.as_str())
            .collect()
    }

    #[test]
    fn two_short_buttons_sit_side_by_side() {
        let two = Dialog::new("t", "Title")
            .button("Cancel", || {})
            .main_button("OK", || {});
        assert!(!two.is_stacked());
        assert_eq!(labels(&two), ["Cancel", "OK"]);
    }

    #[test]
    fn three_buttons_or_a_long_label_stack() {
        let three = Dialog::new("t", "Title")
            .button("Cancel", || {})
            .destructive("Delete", || {})
            .main_button("Save", || {});
        assert!(three.is_stacked());
        assert_eq!(labels(&three), ["Save", "Delete", "Cancel"]);

        let long = Dialog::new("t", "Title")
            .button("Cancel", || {})
            .main_button("Move to Trash and Empty It", || {});
        assert!(long.is_stacked());

        assert!(!three.clone().side_by_side().is_stacked());
        assert!(
            Dialog::new("t", "Title")
                .main_button("OK", || {})
                .stacked()
                .is_stacked()
        );
    }

    #[test]
    fn side_by_side_puts_the_default_on_the_right() {
        let two = Dialog::new("t", "Title")
            .main_button("Discard", || {})
            .button("Cancel", || {});
        assert_eq!(labels(&two), ["Cancel", "Discard"]);
    }

    #[test]
    fn measures_like_the_design() {
        // the design's text: a one-line title over three lines of message
        let text = "A message long enough to wrap onto three lines of the alert, as the \
                    design's own does, with a few more words";
        let two = Dialog::new("t", "Title")
            .message(text)
            .button("Cancel", || {})
            .main_button("OK", || {});
        let three = two.clone().destructive("Delete", || {});

        let lines = message(text, Color::TRANSPARENT).height_in(text_column_width());
        let line = message("Ag", Color::TRANSPARENT).height_in(f32::MAX);
        assert_eq!(
            (lines / line).round(),
            3.0,
            "the sample should wrap to 3 lines"
        );

        assert!((two.height() - 152.0).abs() < 0.5, "{}", two.height());
        assert!((three.height() - 216.0).abs() < 0.5, "{}", three.height());
    }

    #[test]
    fn return_and_escape_answer() {
        let answered = Rc::new(RefCell::new(Vec::new()));
        let (a, b) = (Rc::clone(&answered), Rc::clone(&answered));
        let shown = Dialog::new("t", "Title")
            .button("Cancel", move || a.borrow_mut().push("cancel"))
            .main_button("OK", move || b.borrow_mut().push("ok"));
        let closes = Rc::new(RefCell::new(0));
        let counted = Rc::clone(&closes);
        let close: Rc<dyn Fn()> = Rc::new(move || *counted.borrow_mut() += 1);

        assert!(shown.key(Key::Enter, &close));
        assert!(shown.key(Key::Escape, &close));
        assert!(!shown.key(Key::Tab, &close));
        assert_eq!(*answered.borrow(), ["ok", "cancel"]);
        assert_eq!(*closes.borrow(), 2);

        // with no cancel button, Escape just closes
        let lone = Dialog::new("t", "Title").main_button("OK", || {});
        assert!(lone.key(Key::Escape, &close));
        assert_eq!(*closes.borrow(), 3);
    }
}
