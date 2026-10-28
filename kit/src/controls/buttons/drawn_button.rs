//! Drawn button — content of its own, drawn in the button's colour.
//!
//! For a control that's neither a label nor a glyph, such as a workspace's
//! pip in the status bar: it gets the same shapes, hover, highlight and
//! clicks as the other buttons, and draws in whatever colour they settle on.

use amane::{Color, Rectangle};

use super::{ButtonBuilder, Shape, Style};
use crate::tokens::sys;

/// A button drawing its own content. `id` keys its hover state, so it must
/// be unique and stable.
pub type DrawnButton = ButtonBuilder<Drawn>;

/// What only a drawn button has.
pub struct Drawn {
    draw: Option<Box<dyn FnOnce(Color) -> Rectangle>>,
    width: f32,
}

impl DrawnButton {
    /// `draw` builds the content in the colour it's handed.
    pub fn new(id: impl Into<String>, draw: impl FnOnce(Color) -> Rectangle + 'static) -> Self {
        let kind = Drawn {
            draw: Some(Box::new(draw)),
            width: sys::size::control::height(),
        };
        Self::with(id, Style::Plain, Shape::Square, kind)
    }

    /// Control height (square) by default.
    pub fn width(mut self, width: f32) -> Self {
        self.kind.width = width;
        self
    }

    pub fn build(mut self) -> Rectangle {
        let draw = self.kind.draw.take().expect("built once");
        let content = draw(self.fg());
        let width = self.kind.width;

        self.finish(width, content)
    }
}
