//! MacLeadingIcon — the coloured tile before a settings row, a list item
//! or a sidebar page: 20 px (28 large), radius 6, an accent fill and a
//! white Material Symbols glyph.

use amane::{Center, Color, Rectangle, Size};

use super::theme::{self, Accent, px};
use crate::layout::icon;

pub struct MacLeadingIcon {
    glyph: char,
    fill: Color,
    side: Option<f32>,
    large: bool,
}

impl MacLeadingIcon {
    pub fn new(glyph: char) -> Self {
        Self {
            glyph,
            fill: Accent::Blue.color(),
            side: None,
            large: false,
        }
    }

    pub fn accent(mut self, accent: Accent) -> Self {
        self.fill = accent.color();
        self
    }

    /// Any colour in place of an [`Accent`], such as the system accent.
    pub fn color(mut self, fill: Color) -> Self {
        self.fill = fill;
        self
    }

    pub fn large(mut self, large: bool) -> Self {
        self.large = large;
        self
    }

    /// A side of its own, in place of 20 or 28.
    pub fn side(mut self, side: f32) -> Self {
        self.side = Some(side);
        self
    }

    pub fn build(self) -> Rectangle {
        let side = self
            .side
            .unwrap_or(if self.large { px(28.0) } else { px(20.0) });
        let radius = theme::field_radius() * (side / px(20.0)).clamp(1.0, 1.4);
        Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(radius)
            .fill(self.fill)
            .align_child(Center, Center)
            .child(icon(self.glyph, (side * 0.72).round(), Color::WHITE))
    }
}
