//! MacPushButton — 24 px tall, 16 px either side of its content, radius 6.
//! Prominent: the accent with white text; otherwise `controlFill` under a
//! primary label. An optional Material Symbols glyph leads, 6 px before the
//! text. Hover lays 8% of the content colour over it.

use std::rc::Rc;

use amane::{Center, Rectangle, Row, Size, Text, Widget};

use super::theme::{self, px};
use super::{hover_fill, interactive, on_press};
use crate::buttons::press_scale;
use crate::layout::{boxed, icon, text_width};
use crate::tokens::Styled;

pub struct MacPushButton {
    id: String,
    text: String,
    icon: Option<char>,
    prominent: bool,
    disabled: bool,
    on_press: Rc<dyn Fn()>,
}

impl MacPushButton {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            icon: None,
            prominent: false,
            disabled: false,
            on_press: Rc::new(|| {}),
        }
    }

    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// The accent fill: the default action.
    pub fn prominent(mut self, prominent: bool) -> Self {
        self.prominent = prominent;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.on_press = Rc::new(press);
        self
    }

    pub fn build(self) -> Rectangle {
        let (fill, fg) = if self.prominent {
            (theme::tint(), theme::glyph_on_tint())
        } else {
            (theme::control_fill(), theme::label_primary())
        };
        let style = theme::control_font();
        let glyph = px(13.5);
        let gap = px(6.0);

        let mut width = text_width(&self.text, style);
        let mut parts: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(g) = self.icon {
            parts.push(boxed(icon(g, glyph, fg)));
            width += glyph + gap;
        }
        parts.push(boxed(Text::new(self.text).style(style).color(fg)));

        let base = Rectangle::new()
            .width(Size::Fixed((width + px(32.0)).ceil()))
            .height(Size::Fixed(theme::control_height()))
            .radius(theme::field_radius())
            .fill(hover_fill(&self.id, fill, fg, self.disabled))
            .scale(press_scale(&self.id, 0.97))
            .align_child(Center, Center)
            .child(Row::new(parts).gap(gap).align(Center));

        let press = self.on_press;
        interactive(base, self.id, self.disabled, on_press(move || press()))
    }
}
