//! MacLeadingIcon — the coloured tile before a settings row: 20 px (28
//! large), radius 6, an accent fill and a white Material Symbols glyph.

use amane::{Center, Rectangle, Size};

use super::theme::{self, Accent, px};
use crate::layout::icon;

pub struct MacLeadingIcon {
    glyph: char,
    accent: Accent,
    large: bool,
}

impl MacLeadingIcon {
    pub fn new(glyph: char) -> Self {
        Self {
            glyph,
            accent: Accent::Blue,
            large: false,
        }
    }

    pub fn accent(mut self, accent: Accent) -> Self {
        self.accent = accent;
        self
    }

    pub fn large(mut self, large: bool) -> Self {
        self.large = large;
        self
    }

    pub fn build(self) -> Rectangle {
        let (side, glyph) = if self.large {
            (px(28.0), px(20.0))
        } else {
            (px(20.0), px(15.0))
        };
        Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(theme::field_radius())
            .fill(self.accent.color())
            .align_child(Center, Center)
            .child(icon(self.glyph, glyph, amane::Color::WHITE))
    }
}
