//! Radio — one choice out of a few, all in sight.
//!
//! Modelled on Material 3's radio button: a ring that takes the accent and a
//! dot when chosen, with an optional label that takes the click too. A group
//! is just radios that share what's chosen: build one per option, each
//! `selected` when the setting equals its value. For many options, or little
//! room, a [`Dropdown`] fits better.
//!
//! [`Dropdown`]: crate::dropdown::Dropdown

use std::rc::Rc;

use amane::{Center, Color, Radius, Rectangle, Size};

use super::checkbox::row;
use super::{Actions, interactive};
use crate::tokens::sys;

/// A radio button. `id` keys its hover, so it must be unique and stable.
pub struct Radio {
    id: String,
    selected: bool,
    label: Option<String>,
    disabled: bool,
    on_select: Rc<dyn Fn()>,
}

impl Radio {
    pub fn new(id: impl Into<String>, selected: bool) -> Self {
        Self {
            id: id.into(),
            selected,
            label: None,
            disabled: false,
            on_select: Rc::new(|| {}),
        }
    }

    /// Text beside the ring, which takes the click too.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Runs when it's clicked while not chosen.
    pub fn on_select(mut self, select: impl Fn() + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    pub fn build(self) -> Rectangle {
        // TODO(token): `sys.size.radio` once Penpot has it
        let side = sys::size::icon::s();
        let ring = sys::border::hairline() * 1.5;

        let (color, dot) = if self.selected {
            (sys::color::accent::bg(), side / 2.0)
        } else {
            (sys::color::content::secondary(), 0.0)
        };

        let mark = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(Color::TRANSPARENT)
            .border(ring, color)
            .align_child(Center, Center)
            .child(
                Rectangle::new()
                    .width(Size::Fixed(dot))
                    .height(Size::Fixed(dot))
                    .radius(Radius::Full)
                    .fill(color),
            );

        // choosing what's already chosen asks for nothing
        let actions = Actions {
            press: (!self.selected).then_some(self.on_select),
            ..Actions::default()
        };

        let base = row(&self.id, mark, self.label, self.disabled);

        interactive(base, self.id, self.disabled, actions)
    }
}
