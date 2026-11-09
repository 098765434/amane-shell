//! Search field — a capsule holding a magnifier, the query and, while
//! there's one, a button that clears it. The text, the caret and the
//! selection are drawn here rather than by a focused `TextInput`, so every
//! key reaches the surface's `on_key` and a [`Field`] does the editing
//! (`Field::handle`). The caret shows while the keys go to the field.
//!
//! Drawn in the shell's look: a `surface.raised` capsule a control
//! tall, the query in `sys.type.label`, the placeholder secondary, the
//! selection washed `state.selected`, the caret in the accent.

use amane::{Center, Color, Radius, Rectangle, Row, Start, Text, Widget};

use crate::buttons::{Actions, interactive};
use crate::icons::{ICON_CLOSE, ICON_SEARCH};
use crate::layout::{boxed, frame, icon, pad, slot, text_width};
use crate::material::{self, FillLevel, Level};
use crate::text_edit::Field;
use crate::tokens::{Styled, TextStyle, sys};

/// A search capsule. `id` keys the clear button's hover, so it must be
/// unique and stable.
pub struct SearchField<'a> {
    id: String,
    field: &'a Field,
    focused: bool,
    placeholder: String,
    width: f32,
    style: TextStyle,
    clear: Option<Box<dyn Fn()>>,
}

impl<'a> SearchField<'a> {
    /// A field showing `field`'s text, its caret drawn while `focused`.
    pub fn new(id: impl Into<String>, field: &'a Field, focused: bool) -> Self {
        Self {
            id: id.into(),
            field,
            focused,
            placeholder: String::from("Search"),
            width: sys::size::popover::width(),
            style: sys::r#type::label(),
            clear: None,
        }
    }

    /// What the empty field says.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// How wide the capsule is; a popover's width unless set.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// What the clear button does; without one there's no button.
    pub fn on_clear(mut self, clear: impl Fn() + 'static) -> Self {
        self.clear = Some(Box::new(clear));
        self
    }

    /// How tall the capsule is: a control.
    pub fn height() -> f32 {
        sys::size::control::height()
    }

    /// The capsule: a search glyph, the text or placeholder, and the clear button.
    pub fn build(self) -> Rectangle {
        let height = Self::height();
        let inset = sys::space::inset::m();
        let gap = sys::space::gap::s();
        let glyph = sys::size::icon::s();
        let typed = !self.field.text().is_empty();
        let clear_w = if typed && self.clear.is_some() {
            glyph + gap
        } else {
            0.0
        };
        let text_w = (self.width - inset * 2.0 - glyph - gap - clear_w).max(1.0);

        let mut parts: Vec<Box<dyn Widget>> = vec![
            boxed(icon(ICON_SEARCH, glyph, material::label(Level::Secondary))),
            boxed(query(
                self.field,
                self.focused,
                &self.placeholder,
                self.style,
                text_w,
                height,
            )),
        ];
        if let Some(clear) = self.clear.filter(|_| typed) {
            let button = slot(glyph).radius(Radius::Full).child(icon(
                ICON_CLOSE,
                glyph * 0.8,
                material::label(Level::Secondary),
            ));
            parts.push(boxed(interactive(
                button,
                format!("{}.clear", self.id),
                false,
                Actions::press(clear),
            )));
        }

        Rectangle::new()
            .width(self.width)
            .height(height)
            .radius(Radius::Full)
            .fill(material::fill_role(FillLevel::Secondary))
            .padding(pad(inset, 0.0))
            .align_child(Start, Center)
            .clip()
            .child(Row::new(parts).gap(gap).align(Center))
    }
}

/// The query as runs of text with the caret between them and the
/// selection washed behind its run, or `placeholder` while it's empty.
fn query(
    field: &Field,
    focused: bool,
    placeholder: &str,
    style: TextStyle,
    width: f32,
    height: f32,
) -> Rectangle {
    let fg = material::label(Level::Primary);
    let caret_h = style.size * 1.25;
    let run = |text: String| Text::new(text).style(style).color(fg);
    let caret = || {
        Rectangle::new()
            .width(sys::border::caret())
            .height(caret_h)
            .radius(sys::border::caret() / 2.0)
            .fill(if focused {
                sys::color::accent::bg()
            } else {
                Color::TRANSPARENT
            })
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if field.text().is_empty() {
        parts.push(boxed(caret()));
        parts.push(boxed(
            Text::new(placeholder)
                .style(style)
                .color(material::label(Level::Secondary)),
        ));
    } else {
        let len = field.len();
        let caret_at = field.caret();
        let (from, to) = field.selection();
        if from > 0 {
            parts.push(boxed(run(field.slice(0, from))));
        }
        if caret_at <= from {
            parts.push(boxed(caret()));
        }
        if to > from {
            let selected = field.slice(from, to);
            parts.push(boxed(
                Rectangle::new()
                    .width(text_width(&selected, style) + sys::space::gap::_2xs())
                    .height(caret_h)
                    .radius(sys::radius::raised() / 2.0)
                    .fill(sys::color::state::selected())
                    .align_child(Center, Center)
                    .child(run(selected)),
            ));
            if caret_at >= to {
                parts.push(boxed(caret()));
            }
        }
        if to < len {
            parts.push(boxed(run(field.slice(to, len))));
        }
    }

    frame(width, height)
        .clip()
        .align_child(Start, Center)
        .child(Row::new(parts).gap(sys::space::gap::_2xs()).align(Center))
}
