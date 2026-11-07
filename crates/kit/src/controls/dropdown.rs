//! Dropdown — one choice out of a list, behind a button that shows it.
//!
//! Drawn as a pop-up button in the form controls' look: a control-fill
//! button, radius 6, showing the chosen option (its glyph, if it has one,
//! then its text) and a drawn chevron. A click opens the menu of options
//! lined up under it, the chosen one filled with the accent; the pointer,
//! or Up, Down and Enter, picks one, and Escape or a click outside closes
//! it (see [`menu`](crate::menu)).
//!
//! The menu is an xdg-popup over the surface the dropdown is on: a normal
//! window's (found by itself, from `current_window`), or a layer window's
//! namespace given with [`Dropdown::layer`].

use crate::buttons::Actions;
use std::rc::Rc;

use amane::{Center, Padding, Rectangle, Row, Size, Text, Widget};

use crate::buttons::interactive;
use crate::layout::{boxed, icon, text_width};
use crate::look::hover::hover_fill;
use crate::menu::{self, MenuItem};
use crate::style::{self, px};
use crate::symbol::{Symbol, symbol};
use crate::tokens::Styled;

/// A dropdown. `id` keys its hover and its menu, so it must be unique and
/// stable.
pub struct Dropdown {
    id: String,
    items: Vec<MenuItem>,
    selected: Option<usize>,
    width: Option<f32>,
    fallback: String,
    disabled: bool,
    layer: Option<&'static str>,
    on_select: Rc<dyn Fn(usize)>,
}

impl Dropdown {
    /// A dropdown of plain text `options` showing the one at `selected`.
    pub fn new(id: impl Into<String>, options: Vec<String>, selected: usize) -> Self {
        Self::items(
            id,
            options.into_iter().map(MenuItem::new).collect(),
            Some(selected),
        )
    }

    /// Options with glyphs before or after their text; `selected` may be
    /// none, which shows [`fallback`](Self::fallback).
    pub fn items(id: impl Into<String>, items: Vec<MenuItem>, selected: Option<usize>) -> Self {
        Self {
            id: id.into(),
            items,
            selected,
            width: None,
            fallback: String::new(),
            disabled: false,
            layer: None,
            on_select: Rc::new(|_| {}),
        }
    }

    /// As wide as this; by default as wide as the longest option.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Shown while nothing is chosen.
    pub fn fallback(mut self, text: impl Into<String>) -> Self {
        self.fallback = text.into();
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The namespace of the layer window it's on, for a dropdown outside a
    /// normal window.
    pub fn layer(mut self, namespace: &'static str) -> Self {
        self.layer = Some(namespace);
        self
    }

    /// Runs with the index of the option chosen.
    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    /// The button showing the choice; clicking it opens the menu.
    pub fn build(self) -> Rectangle {
        let style = style::control_font();
        let height = style::control_height();
        let glyph = px(12.0);
        let chevron = px(11.0);
        let (lead_icon, lead_text, trail, gap) = (px(8.0), px(12.0), px(8.0), px(6.0));

        let has_glyph = self.items.iter().any(|item| item.icon.is_some());
        let widest = self
            .items
            .iter()
            .map(|item| text_width(&item.label, style))
            .chain(std::iter::once(text_width(&self.fallback, style)))
            .fold(0.0, f32::max);
        let lead = if has_glyph {
            lead_icon + glyph + gap
        } else {
            lead_text
        };
        let width = self
            .width
            .unwrap_or_else(|| (lead + widest + px(4.0) + chevron + trail).ceil());

        let current = self.selected.and_then(|i| self.items.get(i)).cloned();
        let shown: Vec<usize> = (0..self.items.len()).collect();
        menu::register_on(
            &self.id,
            self.items,
            shown,
            self.selected,
            (width, height),
            self.on_select,
            self.layer,
        );

        let fg = style::label_primary();
        let mut parts: Vec<Box<dyn Widget>> = Vec::new();
        let current_glyph = current
            .as_ref()
            .and_then(|item| item.icon.as_ref())
            .and_then(crate::menu::Icon::glyph_char);
        if let Some(g) = current_glyph {
            parts.push(boxed(icon(g, glyph, fg)));
        }
        let text = current.map(|item| item.label).unwrap_or(self.fallback);
        parts.push(boxed(Text::new(text).style(style).color(fg).elide()));

        let label = Rectangle::new()
            .width(Size::Parent)
            .height(height)
            .align_child(amane::Start, Center)
            .child(Row::new(parts).gap(gap).align(Center).width(Size::Parent));

        let content = Row::new(vec![
            boxed(label),
            boxed(symbol(Symbol::ChevronDown, chevron, fg)),
        ])
        .gap(px(4.0))
        .align(Center)
        .width(Size::Parent);

        let base = Rectangle::new()
            .width(width)
            .height(height)
            .radius(style::field_radius())
            .fill(hover_fill(
                &self.id,
                style::control_fill(),
                fg,
                self.disabled,
            ))
            .padding(Padding {
                left: if current_glyph.is_some() {
                    lead_icon
                } else {
                    lead_text
                },
                right: trail,
                top: 0.0,
                bottom: 0.0,
            })
            .align_child(amane::Start, Center)
            .child(content);

        let id = self.id.clone();
        let base = interactive(
            base,
            self.id.clone(),
            self.disabled,
            Actions::press(move || menu::toggle(&id)),
        );
        menu::track(&self.id, base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdowns_build_with_and_without_a_choice() {
        let options = vec![String::from("One"), String::from("Two")];
        let _ = Dropdown::new("test.dropdown", options, 1).build();
        let items = vec![
            MenuItem::new("Name").icon('\u{e5ca}'),
            MenuItem::new("Size"),
        ];
        let _ = Dropdown::items("test.dropdown.items", items, None)
            .fallback("Choose")
            .layer("test.layer")
            .disabled(true)
            .build();
    }
}
