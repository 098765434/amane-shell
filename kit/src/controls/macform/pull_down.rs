//! MacPullDown — the selector: a 110×24 `controlFill` button showing the
//! active item (its glyph, if any, then its text) and a down chevron, which
//! opens the [menu](super::menu) of items lined up under it.

use std::rc::Rc;

use amane::{Center, Rectangle, Row, Size, Text, Widget};

use super::menu::{self, MenuItem};
use super::symbol::{Symbol, symbol};
use super::theme::{self, px};
use super::{hover_fill, interactive, on_press};
use crate::layout::{boxed, icon, pad};
use crate::tokens::Styled;

pub struct MacPullDown {
    id: String,
    items: Vec<MenuItem>,
    active: Option<usize>,
    width: f32,
    fallback: String,
    disabled: bool,
    on_select: Rc<dyn Fn(usize)>,
}

impl MacPullDown {
    pub fn new(id: impl Into<String>, items: Vec<MenuItem>, active: Option<usize>) -> Self {
        Self {
            id: id.into(),
            items,
            active,
            width: px(110.0),
            fallback: String::new(),
            disabled: false,
            on_select: Rc::new(|_| {}),
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shown while no item is active.
    pub fn fallback(mut self, text: impl Into<String>) -> Self {
        self.fallback = text.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    pub fn build(self) -> Rectangle {
        let height = theme::control_height();
        let shown: Vec<usize> = (0..self.items.len()).collect();
        let current = self.active.and_then(|i| self.items.get(i)).cloned();
        menu::register(
            &self.id,
            self.items,
            shown,
            self.active,
            (self.width, height),
            self.on_select,
        );

        let fg = theme::label_primary();
        let mut parts: Vec<Box<dyn Widget>> = Vec::new();
        let glyph = current.as_ref().and_then(|item| item.icon);
        if let Some(g) = glyph {
            parts.push(boxed(icon(g, px(12.0), fg)));
        }
        let text = current.map(|item| item.text).unwrap_or(self.fallback);
        parts.push(boxed(
            Text::new(text)
                .style(theme::control_font())
                .color(fg)
                .elide(),
        ));
        let lead = if glyph.is_some() { px(8.0) } else { px(12.0) };
        let label = Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(height))
            .align_child(amane::Start, Center)
            .child(
                Row::new(parts)
                    .gap(px(6.0))
                    .align(Center)
                    .width(Size::Parent),
            );

        let content = Row::new(vec![
            boxed(label),
            boxed(symbol(Symbol::ChevronDown, px(11.0), fg)),
        ])
        .gap(px(4.0))
        .align(Center)
        .width(Size::Parent);

        let base = Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(height))
            .radius(theme::field_radius())
            .fill(hover_fill(
                &self.id,
                theme::control_fill(),
                fg,
                self.disabled,
            ))
            .padding(amane::Padding {
                left: lead,
                right: px(8.0),
                ..pad(0.0, 0.0)
            })
            .align_child(amane::Start, Center)
            .child(content);

        let id = self.id.clone();
        let base = interactive(
            base,
            self.id.clone(),
            self.disabled,
            on_press(move || menu::toggle(&id)),
        );
        menu::track(&self.id, base)
    }
}
