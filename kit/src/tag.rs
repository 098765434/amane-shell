//! Tag — a short fact as a small capsule label: "OLED", "HDR · 1107 nits".
//!
//! Modelled on Material 3's assist chip, without its action: it only
//! labels. Drawn as a capsule of the secondary fill with a caption in it,
//! or the accent's tint and colour for the fact that matters most.
//! [`tags`] lays several out in rows that wrap, through [`Flow`].

use amane::{Center, Radius, Rectangle, Size, Text};

use crate::color::with_alpha;
use crate::flow::Flow;
use crate::layout::text_width;
use crate::material::{self, FillLevel, Level};
use crate::tokens::{Styled, sys};

pub struct Tag {
    text: String,
    accent: bool,
}

impl Tag {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            accent: false,
        }
    }

    /// In the accent, for the one fact a reader should see first.
    pub fn accent(mut self, accent: bool) -> Self {
        self.accent = accent;
        self
    }

    /// Its width and height, which a layout needs before building it.
    pub fn size(&self) -> (f32, f32) {
        let height = sys::size::control::mini();
        let width = text_width(&self.text, sys::r#type::caption()) + sys::space::inset::s() * 2.0;
        (width.ceil(), height)
    }

    pub fn build(self) -> Rectangle {
        let (width, height) = self.size();
        let (fill, color) = if self.accent {
            (
                with_alpha(sys::color::accent::bg(), 0.16),
                sys::color::accent::bg(),
            )
        } else {
            (
                material::fill_role(FillLevel::Secondary),
                material::label(Level::Secondary),
            )
        };
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .radius(Radius::Full)
            .fill(fill)
            .align_child(Center, Center)
            .child(
                Text::new(self.text)
                    .style(sys::r#type::caption())
                    .color(color),
            )
    }
}

/// Tags in rows `width` wide, wrapping, and how tall they came out.
pub fn tags(width: f32, tags: Vec<Tag>) -> (Rectangle, f32) {
    let gap = sys::space::gap::xs();
    let flow = tags
        .into_iter()
        .fold(Flow::new(width, gap, gap), |flow, tag| {
            let (w, h) = tag.size();
            flow.item(w, h, tag.build())
        });
    let (rectangle, height, _) = flow.build();
    (rectangle, height)
}
