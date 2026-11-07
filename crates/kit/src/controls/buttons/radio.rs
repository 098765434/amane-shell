//! Radio — one choice out of a few, all in sight.
//!
//! Drawn in the form controls' look ([`style`](crate::style)), at the
//! [`control_size`](crate::control_size) being built: a circle, `fieldFill` in a
//! hairline while not chosen, filling with the accent around a white dot
//! when chosen, with an optional label 3 px after it that takes the click
//! too. It is as wide as its mark and label. A group
//! is just radios that share what's chosen: build one per option, each
//! `selected` when the setting equals its value. For many options, or little
//! room, a [`Dropdown`] fits better.
//!
//! [`Dropdown`]: crate::dropdown::Dropdown

use std::rc::Rc;

use amane::Rectangle;

use super::{Actions, interactive};
use crate::controls::parts::{Mark, labelled, mark};

/// A radio button. `id` keys its hover, so it must be unique and stable.
pub struct Radio {
    id: String,
    selected: bool,
    label: Option<String>,
    disabled: bool,
    on_select: Rc<dyn Fn()>,
}

impl Radio {
    /// A radio, chosen or not, with no label and a click that does nothing yet.
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

    /// The ring, and its label if it has one.
    pub fn build(self) -> Rectangle {
        let mark = mark(&self.id, self.selected, Mark::Dot);

        // choosing what's already chosen asks for nothing
        let actions = Actions {
            press: (!self.selected).then_some(self.on_select),
            ..Actions::default()
        };

        let base = labelled(mark, self.label);

        interactive(base, self.id, self.disabled, actions)
    }
}
