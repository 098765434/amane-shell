//! Checkbox — a setting that's on or off, in a list of independent ones.
//!
//! Drawn in the macform look: a 16 px box, radius 5.5,
//! `fieldFill` in a hairline while off, filling with the accent around a
//! drawn checkmark when on, with an optional label 3 px after it that takes
//! the click too. Where a setting applies at once, a [`Switch`] reads
//! better; a checkbox suits choices confirmed together, like a dialog's.
//!
//! [`Switch`]: super::Switch

#![allow(missing_docs)]

use std::rc::Rc;

use amane::{Center, Rectangle, Size};

use super::{Actions, interactive};
use crate::macform::symbol::{Symbol, symbol};
use crate::macform::theme::{self, px};
use crate::macform::{crossfade, labelled};

/// A checkbox. `id` keys its hover, so it must be unique and stable.
pub struct Checkbox {
    id: String,
    checked: bool,
    label: Option<String>,
    disabled: bool,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl Checkbox {
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

    pub fn build(self) -> Rectangle {
        let side = px(16.0);
        let fill = crossfade(
            &format!("{}.fill", self.id),
            self.checked,
            theme::field_fill(),
            theme::tint(),
        );
        let mark = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(px(5.5))
            .fill(fill)
            .align_child(Center, Center);
        let mark = if self.checked {
            mark.child(symbol(Symbol::Checkmark, px(11.0), theme::glyph_on_tint()))
        } else {
            mark.border(1.0, theme::field_border())
        };

        let toggle = self.on_toggle;
        let next = !self.checked;
        let actions = Actions {
            press: Some(Rc::new(move || toggle(next))),
            ..Actions::default()
        };

        let base = labelled(mark, self.label);

        interactive(base, self.id, self.disabled, actions)
    }
}
