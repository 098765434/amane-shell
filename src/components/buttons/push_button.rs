//! Push button — a label, optionally led by an icon.
//!
//! Modelled on Material 3's buttons: filled, tonal and text variants, round
//! or square shape, a leading icon, a hover state layer and a disabled
//! state. It is sized like the shell's controls rather than M3's: control
//! height, and as wide as its content plus the inset unless told otherwise.

use std::rc::Rc;

use amane::{Center, Color, Rectangle, Row, Size, Text, Widget};

use super::{Actions, Shape, Style, interactive, state_fill};
use crate::components::taskbar::{boxed, icon};
use crate::tokens::{Styled, TextStyle, sys};

/// A labelled button. `id` keys its hover state, so it must be unique and
/// stable.
pub struct PushButton {
    id: String,
    label: String,
    icon: Option<char>,
    icon_size: f32,
    style: Style,
    shape: Shape,
    text: TextStyle,
    color: Option<Color>,
    width: Option<f32>,
    height: Size,
    disabled: bool,
    actions: Actions,
}

impl PushButton {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            icon_size: sys::size::icon::s(),
            style: Style::Tonal,
            shape: Shape::Square,
            text: sys::r#type::caption(),
            color: None,
            width: None,
            height: Size::Fixed(sys::size::control::height()),
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

    /// A Material Symbols glyph before the label.
    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// The leading glyph's size; `sys.size.icon.s` by default.
    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = size;
        self
    }

    /// The label's type role; `sys.type.caption` by default.
    pub fn text(mut self, style: TextStyle) -> Self {
        self.text = style;
        self
    }

    /// The label and icon colour. Defaults to `accent.fg` on a filled
    /// button and the panel's `fg` otherwise, so pass the `fg` of any other
    /// surface the button sits on.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// A fixed width; without one the button hugs its content.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
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
        let color = self.color.unwrap_or(match self.style {
            Style::Filled => sys::color::accent::fg(),
            _ => sys::color::surface::panel::fg(),
        });

        let mut content: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(glyph) = self.icon {
            content.push(boxed(icon(glyph, self.icon_size, color)));
        }
        // not elided, which would stretch it and lose the centering;
        // the button clips a label too long for it
        content.push(boxed(Text::new(self.label).style(self.text).color(color)));
        let content = Row::new(content).gap(sys::space::gap::xs()).align(Center);

        // hugging, the content keeps an inset either side
        let inset = sys::space::inset::m();
        let width = self.width.unwrap_or_else(|| match Widget::width(&content) {
            Size::Fixed(width) => width + inset * 2.0,
            Size::Parent => 0.0,
        });

        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(self.height)
            .radius(self.shape.radius())
            .fill(state_fill(&self.id, self.style.fill(), self.disabled))
            .align_child(Center, Center)
            .clip()
            .child(content);

        interactive(base, self.id, self.disabled, self.actions)
    }
}
