//! Icon button — a lone glyph.
//!
//! Modelled on Material 3's icon buttons: standard (plain), filled and tonal
//! variants, round or square shape, an optional toggle whose selected state
//! turns the glyph accent, a hover state layer and a disabled state. It is
//! square at control height unless told otherwise; bar items stretch it to
//! the bar's height.

use std::rc::Rc;

use amane::{Center, Color, Rectangle, Size};

use super::{Actions, Shape, Style, interactive, state_fill};
use crate::components::taskbar::icon;
use crate::tokens::sys;

/// A glyph button. `id` keys its hover state, so it must be unique and
/// stable.
pub struct IconButton {
    id: String,
    glyph: char,
    style: Style,
    shape: Shape,
    size: f32,
    color: Option<Color>,
    selected: bool,
    width: f32,
    height: Size,
    disabled: bool,
    actions: Actions,
}

impl IconButton {
    pub fn new(id: impl Into<String>, glyph: char) -> Self {
        let side = sys::size::control::height();
        Self {
            id: id.into(),
            glyph,
            style: Style::Plain,
            shape: Shape::Round,
            size: sys::size::icon::m(),
            color: None,
            selected: false,
            width: side,
            height: Size::Fixed(side),
            disabled: false,
            actions: Actions::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// The glyph's size; `sys.size.icon.m` by default.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// The glyph's colour. Defaults to `accent.fg` on a filled button and
    /// the panel's `fg` otherwise, so pass the `fg` of any other surface
    /// the button sits on.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// A toggle that's on: the glyph turns accent, whatever its colour.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: impl Into<Size>) -> Self {
        self.height = height.into();
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What a left click does.
    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.actions.press = Some(Rc::new(press));
        self
    }

    /// What a right click does, such as opening a menu.
    pub fn on_secondary(mut self, secondary: impl Fn() + 'static) -> Self {
        self.actions.secondary = Some(Rc::new(secondary));
        self
    }

    pub fn build(self) -> Rectangle {
        let color = match (self.selected, self.style) {
            (true, Style::Filled) => sys::color::accent::fg(),
            (true, _) => sys::color::accent::bg(),
            (false, Style::Filled) => self.color.unwrap_or(sys::color::accent::fg()),
            (false, _) => self.color.unwrap_or(sys::color::surface::panel::fg()),
        };

        let base = Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(self.height)
            .radius(self.shape.radius())
            .fill(state_fill(&self.id, self.style.fill(), self.disabled))
            .align_child(Center, Center)
            .child(icon(self.glyph, self.size, color));

        interactive(base, self.id, self.disabled, self.actions)
    }
}
