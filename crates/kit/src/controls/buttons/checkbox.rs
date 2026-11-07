//! Checkbox — a setting that's on or off, in a list of independent ones.
//!
//! Drawn in the form controls' look ([`style`](crate::style)), at the
//! [`control_size`](crate::control_size) being built: a rounded box,
//! `fieldFill` in a hairline while off, filling with the accent around a
//! drawn checkmark when on, with an optional label 3 px after it that takes
//! the click too. Where a setting applies at once, a [`Switch`] reads
//! better; a checkbox suits choices confirmed together, like a dialog's.
//!
//! [`Switch`]: super::Switch

use std::rc::Rc;

use amane::Rectangle;

use super::{Actions, interactive};
use crate::controls::parts::{Mark, labelled, mark};

/// A checkbox. `id` keys its hover, so it must be unique and stable.
pub struct Checkbox {
    id: String,
    checked: bool,
    label: Option<String>,
    disabled: bool,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl Checkbox {
    /// A checkbox, checked or not, with no label and a click that does nothing yet.
    pub fn new(id: impl Into<String>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            disabled: false,
            on_toggle: Rc::new(|_| {}),
        }
    }

    /// Text beside the box, which takes the click too.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What a click asks for: the other state.
    pub fn on_toggle(mut self, toggle: impl Fn(bool) + 'static) -> Self {
        self.on_toggle = Rc::new(toggle);
        self
    }

    /// The box, and its label if it has one.
    pub fn build(self) -> Rectangle {
        let mark = mark(&self.id, self.checked, Mark::Check);

        let toggle = self.on_toggle;
        let next = !self.checked;
        let actions = Actions::press(move || toggle(next));

        let base = labelled(mark, self.label);

        interactive(base, self.id, self.disabled, actions)
    }
}
