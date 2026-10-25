//! Card — a surface that holds one subject's content.
//!
//! Modelled on Material 3's cards: filled and outlined variants on a
//! surface role, content stacked inside an inset. Drawn in the shell's
//! macOS look: `sys.radius.popover` corners, and frosted when the
//! compositor can blur what's below (a departure from M3, which has no
//! translucent material), its edge lit like [`glass`](super::glass) when
//! asked. M3's elevated card is left out: there's no `sys.shadow` role yet.
//!
//! A card records where the pointer is over it ([`layout::cursor`]), so a
//! right-click on its content can anchor a menu.

use amane::{Color, Column, Parent, Rectangle, Widget};

use crate::glass;
use crate::layout::{pad, track_cursor};
use crate::tokens::sys;

/// The surface role a card is drawn on: one `bg` / `fg` / `stroke` set, so
/// what's on it takes its colours from the same pair (manual §1.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// `sys.color.surface.panel.*`: the bar and its panels.
    Panel,
    /// `sys.color.surface.popover.*`: menus and the notification center.
    Popover,
    /// `sys.color.surface.notification.*`: banners.
    Notification,
}

impl Surface {
    pub fn bg(self) -> Color {
        match self {
            Self::Panel => sys::color::surface::panel::bg(),
            Self::Popover => sys::color::surface::popover::bg(),
            Self::Notification => sys::color::surface::notification::bg(),
        }
    }

    pub fn fg(self) -> Color {
        match self {
            Self::Panel => sys::color::surface::panel::fg(),
            Self::Popover => sys::color::surface::popover::fg(),
            Self::Notification => sys::color::surface::notification::fg(),
        }
    }

    /// Text that reads secondary on this surface.
    pub fn fg_secondary(self) -> Color {
        match self {
            // the panel has no secondary of its own yet
            Self::Panel => sys::color::content::secondary(),
            Self::Popover => sys::color::surface::popover::fg_secondary(),
            Self::Notification => sys::color::surface::notification::fg_secondary(),
        }
    }

    /// The edge an outlined card draws; the panel has none yet.
    pub fn stroke(self) -> Option<Color> {
        match self {
            Self::Panel => None,
            Self::Popover => Some(sys::color::surface::popover::stroke()),
            Self::Notification => Some(sys::color::surface::notification::stroke()),
        }
    }
}

/// M3's card variants that the shell draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// The surface's fill alone.
    Filled,
    /// The fill and a hairline in the surface's `stroke`.
    Outlined,
}

/// A card filling its parent, its content stacked `gap` apart.
pub struct Card {
    content: Vec<Box<dyn Widget>>,
    surface: Surface,
    variant: Variant,
    blur: bool,
    glass: bool,
    gap: f32,
    padding: f32,
}

impl Card {
    pub fn new(content: Vec<Box<dyn Widget>>) -> Self {
        Self {
            content,
            surface: Surface::Popover,
            variant: Variant::Filled,
            blur: false,
            glass: false,
            gap: 0.0,
            padding: sys::space::inset::l(),
        }
    }

    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
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
    /// translucent window.
    pub fn glass(mut self, glass: bool) -> Self {
        self.glass = glass;
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
            .radius(sys::radius::popover())
            .fill(self.surface.bg())
            .padding(pad(self.padding, self.padding));

        if self.variant == Variant::Outlined
            && let Some(stroke) = self.surface.stroke()
        {
            card = card.border(sys::border::hairline(), stroke);
        }
        if self.blur {
            card = card.blur(sys::effect::backdrop_blur());
        }
        if self.glass {
            card = glass::apply(card, sys::radius::popover());
        }

        track_cursor(card).child(Column::new(self.content).gap(self.gap).width(Parent))
    }
}
