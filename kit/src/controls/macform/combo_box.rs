//! MacComboBox — a [text field](super::MacTextField) with a 20 px
//! `controlFill` chevron button inside its right edge (radius 4.5, 2 px
//! in), which opens the [menu](super::menu) of suggestions: the items
//! containing what's typed, or all of them while the text is one of them
//! or empty. Choosing one puts it in the field.
//!
//! The original's menu was the old shell's Material one; this uses the
//! pull-down's.

use std::rc::Rc;

use amane::{Center, Rectangle, Row, Size, Stack, TextInput};

use super::menu::{self, MenuItem};
use super::symbol::{Symbol, symbol};
use super::text_field::MacTextField;
use super::theme::{self, px};
use super::{hover_fill, interactive, on_press};
use crate::layout::boxed;

pub struct MacComboBox {
    id: &'static str,
    items: Vec<MenuItem>,
    width: f32,
    placeholder: String,
    on_change: Rc<dyn Fn(String)>,
}

impl MacComboBox {
    pub fn new(id: &'static str, items: Vec<MenuItem>) -> Self {
        Self {
            id,
            items,
            width: px(110.0),
            placeholder: String::new(),
            on_change: Rc::new(|_| {}),
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Runs with the text whenever it changes, typed or chosen.
    pub fn on_change(mut self, change: impl Fn(String) + 'static) -> Self {
        self.on_change = Rc::new(change);
        self
    }

    pub fn build(self) -> Rectangle {
        let id = self.id;
        let height = theme::control_height();
        let typed = TextInput::text(id);
        let shown = suggestions(&self.items, &typed);
        let active = self.items.iter().position(|item| item.text == typed);

        let texts: Vec<String> = self.items.iter().map(|item| item.text.clone()).collect();
        let chosen = self.on_change.clone();
        let menu_id = format!("{id}.menu");
        menu::register(
            &menu_id,
            self.items,
            shown,
            active,
            (self.width, height),
            Rc::new(move |index| {
                if let Some(text) = texts.get(index) {
                    TextInput::set_text(id, text);
                    chosen(text.clone());
                }
            }),
        );

        let field = MacTextField::new(id)
            .placeholder(self.placeholder)
            .width(Size::Fixed(self.width))
            .on_change({
                let change = self.on_change;
                move |text| change(text)
            })
            .build();

        let fg = theme::label_primary();
        let button_id = format!("{id}.chevron");
        let button = Rectangle::new()
            .width(Size::Fixed(px(20.0)))
            .height(Size::Fixed(height - px(4.0)))
            .radius(px(4.5))
            .fill(hover_fill(&button_id, theme::control_fill(), fg, false))
            .align_child(Center, Center)
            .child(symbol(Symbol::ChevronDown, px(11.0), fg));
        let opens = menu_id.clone();
        let button = interactive(
            button,
            button_id,
            false,
            on_press(move || menu::toggle(&opens)),
        );

        let overlay = Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(height))
            .padding(amane::Padding {
                right: px(2.0),
                ..crate::layout::pad(0.0, 0.0)
            })
            .align_child(amane::End, Center)
            .child(Row::new(vec![boxed(button)]));

        let base = Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(height))
            .child(Stack::new(vec![boxed(field), boxed(overlay)]));
        menu::track(&menu_id, base)
    }
}

/// The items to suggest for `typed`, as indices.
fn suggestions(items: &[MenuItem], typed: &str) -> Vec<usize> {
    let needle = typed.trim().to_lowercase();
    let exact = items.iter().any(|item| item.text == typed);
    let all = || (0..items.len()).collect();
    if needle.is_empty() || exact {
        return all();
    }
    let found: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.text.to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect();
    if found.is_empty() { all() } else { found }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_narrow_to_what_is_typed() {
        let items: Vec<MenuItem> = ["Inter", "Atkinson", "Iosevka"].map(MenuItem::from).into();
        assert_eq!(suggestions(&items, ""), vec![0, 1, 2]);
        assert_eq!(suggestions(&items, "in"), vec![0, 1]);
        assert_eq!(suggestions(&items, "Inter"), vec![0, 1, 2]);
        assert_eq!(suggestions(&items, "zzz"), vec![0, 1, 2]);
    }
}
