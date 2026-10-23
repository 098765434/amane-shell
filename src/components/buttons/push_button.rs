//! Push button — a label, optionally led by an icon.
//!
//! Modelled on Material 3's buttons: filled, tonal and text variants, round
//! or square shape, a leading icon, a hover state layer and a disabled
//! state. It is sized like the shell's controls rather than M3's: control
//! height, and as wide as its content plus the inset unless told otherwise.

use amane::Rectangle;

use super::{ButtonBuilder, Shape, Style, label_row};
use crate::components::layout::fixed_width;
use crate::tokens::{TextStyle, sys};

/// A labelled button. `id` keys its hover state, so it must be unique and
/// stable.
pub type PushButton = ButtonBuilder<Label>;

/// What only a push button has.
pub struct Label {
    label: String,
    icon: Option<char>,
    icon_size: f32,
    text: TextStyle,
    width: Option<f32>,
}

impl PushButton {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let kind = Label {
            label: label.into(),
            icon: None,
            icon_size: sys::size::icon::s(),
            text: sys::r#type::caption(),
            width: None,
        };
        Self::with(id, Style::Tonal, Shape::Square, kind)
    }

    /// A Material Symbols glyph before the label.
    pub fn icon(mut self, glyph: char) -> Self {
        self.kind.icon = Some(glyph);
        self
    }

    /// The leading glyph's size; `sys.size.icon.s` by default.
    pub fn icon_size(mut self, size: f32) -> Self {
        self.kind.icon_size = size;
        self
    }

    /// The label's type role; `sys.type.caption` by default.
    pub fn text(mut self, style: TextStyle) -> Self {
        self.kind.text = style;
        self
    }

    /// A fixed width; without one the button hugs its content.
    pub fn width(mut self, width: f32) -> Self {
        self.kind.width = Some(width);
        self
    }

    pub fn build(self) -> Rectangle {
        let kind = &self.kind;
        let content = label_row(
            kind.icon,
            kind.icon_size,
            kind.label.clone(),
            kind.text,
            self.fg(),
        );

        // hugging, the content keeps an inset either side
        let width = kind
            .width
            .unwrap_or_else(|| fixed_width(&content) + sys::space::inset::m() * 2.0);

        self.finish(width, content)
    }
}
