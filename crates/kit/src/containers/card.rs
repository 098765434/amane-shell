//! Card — a surface that holds one subject's content.
//!
//! Filled and outlined, on a [`Material`], with its content stacked inside
//! an inset; what's on it takes the vibrant labels and fills
//! ([`material::label`]). It has `sys.radius.popover` corners, is frosted
//! when the compositor can blur what's below, and has its edge lit like
//! [`glass`](crate::glass) when asked. There's no raised card: there's no
//! `sys.shadow` role yet.
//!
//! A card records where the pointer is over it ([`layout::cursor`](crate::layout::cursor)), so a
//! right-click on its content can anchor a menu.

use amane::{Color, Column, Parent, Rectangle, Widget};

use crate::layout::{pad, track_cursor};
use crate::material::{self, Material};
use crate::tokens::sys;

/// How a card sets itself apart from what's behind it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// The material's fill alone.
    Filled,
    /// The fill and a hairline in the material's separator.
    Outlined,
}

/// A card filling its parent, its content stacked `gap` apart.
pub struct Card {
    content: Vec<Box<dyn Widget>>,
    material: Material,
    variant: Variant,
    blur: bool,
    glass: bool,
    fill: Option<Color>,
    stroke: Option<Color>,
    radius: f32,
    gap: f32,
    padding: f32,
}

impl Card {
    /// A filled card on `Material::Popover` holding `content`, inset by `sys.space.inset.l`.
    pub fn new(content: Vec<Box<dyn Widget>>) -> Self {
        Self {
            content,
            material: Material::Popover,
            variant: Variant::Filled,
            blur: false,
            glass: false,
            fill: None,
            stroke: None,
            radius: sys::radius::popover(),
            gap: 0.0,
            padding: sys::space::inset::l(),
        }
    }

    /// What the card is made of; `Material::Popover` by default.
    pub fn material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// Filled or outlined; filled by default.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// Frost what's below with `sys.effect.backdrop-blur`.
    pub fn blur(mut self, blur: bool) -> Self {
        self.blur = blur;
        self
    }

    /// Light the edge like glass, for a card that is its surface's whole
    /// translucent window: the shader's edge, unless the compositor draws
    /// the material behind it ([`Material::lit`]), and then no hairline
    /// either.
    pub fn glass(mut self, glass: bool) -> Self {
        self.glass = glass;
        self
    }

    /// The fill in place of the material's tint.
    pub fn fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    /// An outlined card's hairline in place of the material's separator.
    pub fn stroke(mut self, stroke: Color) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// The corner radius in logical pixels; `sys.radius.popover` by default.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// The space between content items; none by default.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// The inset on every side; `sys.space.inset.l` by default.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// The card, filling its parent.
    pub fn build(self) -> Rectangle {
        let mut card = Rectangle::new()
            .width(Parent)
            .height(Parent)
            .radius(self.radius)
            .fill(self.fill.unwrap_or_else(|| self.material.tint()))
            .padding(pad(self.padding, self.padding));

        if self.variant == Variant::Outlined {
            // glass draws no hairline where the compositor's material is its edge
            let stroke = self.stroke.unwrap_or_else(material::separator);
            let stroke = if self.glass {
                self.material.stroke(stroke)
            } else {
                stroke
            };
            card = card.border(sys::border::hairline(), stroke);
        }
        if self.blur {
            card = card.blur(sys::effect::backdrop_blur());
        }
        if self.glass {
            card = self.material.lit(card, self.radius);
        }

        track_cursor(card).child(Column::new(self.content).gap(self.gap).width(Parent))
    }
}
