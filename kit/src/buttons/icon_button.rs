//! Icon button — a lone glyph.
//!
//! Modelled on Material 3's icon buttons: standard (plain), filled and tonal
//! variants, round or square shape, an optional toggle whose selected state
//! turns the glyph accent, a hover state layer and a disabled state. It is
//! square at control height unless told otherwise; bar items stretch it to
//! the bar's height.

use amane::Rectangle;

use super::{ButtonBuilder, Shape, Style};
use crate::icons::{Glyph, glyph};
use crate::tokens::sys;

/// A glyph button. `id` keys its hover state, so it must be unique and
/// stable.
pub type IconButton = ButtonBuilder<Lone>;

/// What only an icon button has.
pub struct Lone {
    glyph: Glyph,
    size: f32,
    selected: bool,
    width: f32,
}

impl IconButton {
    pub fn new(id: impl Into<String>, glyph: impl Into<Glyph>) -> Self {
        let kind = Lone {
            glyph: glyph.into(),
            size: sys::size::icon::m(),
            selected: false,
            width: sys::size::control::height(),
        };
        Self::with(id, Style::Plain, Shape::Round, kind)
    }

    /// The glyph's size; `sys.size.icon.m` by default.
    pub fn size(mut self, size: f32) -> Self {
        self.kind.size = size;
        self
    }

    /// A toggle that's on: the glyph turns accent, whatever its colour.
    pub fn selected(mut self, selected: bool) -> Self {
        self.kind.selected = selected;
        self
    }

    /// Control height (square) by default.
    pub fn width(mut self, width: f32) -> Self {
        self.kind.width = width;
        self
    }

    pub fn build(self) -> Rectangle {
        let color = match (self.kind.selected, self.style) {
            (true, Style::Filled) => sys::color::accent::fg(),
            (true, _) => sys::color::accent::bg(),
            (false, _) => self.fg(),
        };
        let glyph = glyph(self.kind.glyph, self.kind.size, color);
        let width = self.kind.width;

        self.finish(width, glyph)
    }
}
