//! PopUpButton — an 18 px `controlFill` circle with the up-and-down
//! chevrons. The original only reported the click; given items, this one
//! opens the [menu](super::menu) under itself.

#![allow(missing_docs)]

use std::rc::Rc;

use amane::{Center, Radius, Rectangle, Size};

use crate::controls::parts::{interactive, on_press};
use crate::look::hover::hover_fill;
use crate::menu::{self, MenuItem};
use crate::style::{self, px};
use crate::symbol::{Symbol, symbol};

pub struct PopUpButton {
    id: String,
    items: Vec<MenuItem>,
    active: Option<usize>,
    disabled: bool,
    on_select: Rc<dyn Fn(usize)>,
    on_press: Option<Rc<dyn Fn()>>,
}

impl PopUpButton {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            active: None,
            disabled: false,
            on_select: Rc::new(|_| {}),
            on_press: None,
        }
    }

    /// The menu it opens, and the item checked in it.
    pub fn items(mut self, items: Vec<MenuItem>, active: Option<usize>) -> Self {
        self.items = items;
        self.active = active;
        self
    }

    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    /// What a click does when it has no items.
    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(press));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn build(self) -> Rectangle {
        let side = px(18.0);
        let fg = style::label_primary();
        let has_menu = !self.items.is_empty();
        if has_menu {
            let shown = (0..self.items.len()).collect();
            menu::register(
                &self.id,
                self.items,
                shown,
                self.active,
                (side, side),
                self.on_select,
            );
        }

        let base = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(hover_fill(
                &self.id,
                style::control_fill(),
                fg,
                self.disabled,
            ))
            .align_child(Center, Center)
            .child(symbol(Symbol::ChevronUpDown, px(12.0), fg));

        let id = self.id.clone();
        let press = self.on_press;
        let base = interactive(
            base,
            self.id.clone(),
            self.disabled,
            on_press(move || {
                if has_menu {
                    menu::toggle(&id);
                } else if let Some(press) = &press {
                    press();
                }
            }),
        );
        if has_menu {
            menu::track(&self.id, base)
        } else {
            base
        }
    }
}
