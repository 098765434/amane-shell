//! Checkbox — a setting that's on or off, in a list of independent ones.
//!
//! Modelled on Material 3's checkbox: a small square that fills with the
//! accent and shows a check when on, with an optional label beside it that
//! takes the click too. Where a setting applies at once, a [`Switch`] reads
//! better; a checkbox suits choices confirmed together, like a dialog's.
//!
//! [`Switch`]: super::Switch

use std::rc::Rc;

use amane::{Center, Color, Radius, Rectangle, Row, Size, Text, children};

use super::{Actions, interactive, state_fill};
use crate::control_size;
use crate::icons::ICON_CHECK;
use crate::layout::{icon, pad};
use crate::material::{self, Level};
use crate::tokens::{Styled, sys};

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
        let (side, radius) = control_size::current().checkbox();

        let mark = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Fixed(radius))
            .align_child(Center, Center);

        let mark = if self.checked {
            mark.fill(sys::color::accent::bg()).child(icon(
                ICON_CHECK,
                side * 0.85,
                sys::color::accent::fg(),
            ))
        } else {
            mark.fill(Color::TRANSPARENT).border(
                sys::border::hairline() * 1.5,
                material::label(Level::Secondary),
            )
        };

        let toggle = self.on_toggle;
        let next = !self.checked;
        let actions = Actions {
            press: Some(Rc::new(move || toggle(next))),
            ..Actions::default()
        };

        let base = row(&self.id, mark, self.label, self.disabled);

        interactive(base, self.id, self.disabled, actions)
    }
}

/// The mark and its label in a hover-lit row, shared with the radio.
pub(super) fn row(id: &str, mark: Rectangle, label: Option<String>, disabled: bool) -> Rectangle {
    let gap = sys::space::gap::s();

    let content = match label {
        Some(label) => Row::new(children![
            mark,
            Text::new(label)
                .style(sys::r#type::label())
                .color(material::label(Level::Primary)),
        ])
        .gap(gap)
        .align(Center),
        None => Row::new(children![mark]),
    };

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(sys::size::control::height()))
        .radius(Radius::Fixed(sys::radius::raised()))
        .fill(state_fill(id, Color::TRANSPARENT, disabled))
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(amane::Start, Center)
        .child(content)
}
