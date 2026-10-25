//! Card — a surface that holds one subject's content.
//!
//! Modelled on Material 3's cards: filled and outlined variants on a
//! [`Material`], content stacked inside an inset; what's on it takes the
//! vibrant labels and fills ([`material::label`]). Drawn in the shell's macOS look: `sys.radius.popover` corners,
//! and frosted when the compositor can blur what's below (a departure from
//! M3, which has no translucent material), its edge lit like
//! [`glass`](super::glass) when asked. M3's elevated card is left out:
//! there's no `sys.shadow` role yet.
//!
//! A card records where the pointer is over it ([`layout::cursor`]), so a
//! right-click on its content can anchor a menu.

use amane::{Color, Column, Parent, Rectangle, Widget};

use crate::layout::{pad, track_cursor};
use crate::material::{self, Material};
use crate::tokens::sys;

/// M3's card variants that the shell draws.
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
    radius: f32,
    gap: f32,
    padding: f32,
}

impl Card {
    pub fn new(content: Vec<Box<dyn Widget>>) -> Self {
        Self {
            content,
            material: Material::Popover,
            variant: Variant::Filled,
            blur: false,
            glass: false,
            fill: None,
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

    /// The corners; `sys.radius.popover` by default.
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

    pub fn build(self) -> Rectangle {
        let mut card = Rectangle::new()
            .width(Parent)
            .height(Parent)
            .radius(self.radius)
            .fill(self.fill.unwrap_or_else(|| self.material.tint()))
            .padding(pad(self.padding, self.padding));

        if self.variant == Variant::Outlined {
            // glass draws no hairline where the compositor's material is its edge
            let stroke = if self.glass {
                self.material.stroke(material::separator())
            } else {
                material::separator()
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
