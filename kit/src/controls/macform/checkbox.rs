//! MacCheckbox — a 16 px box, radius 5.5, with a label 3 px after it.
//!
//! Checked: the accent with a white checkmark. Unchecked: `fieldFill` in a
//! 1 px `fieldBorder`. The fill crossfades over 300 ms; the whole control,
//! label included, takes the click.

use std::rc::Rc;

use amane::{Center, Rectangle, Row, Size, Text, Widget};

use super::symbol::{Symbol, symbol};
use super::theme::{self, px};
use super::{crossfade, interactive, on_press};
use crate::layout::{boxed, text_width};
use crate::tokens::Styled;

pub struct MacCheckbox {
    id: String,
    checked: bool,
    label: Option<String>,
    disabled: bool,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl MacCheckbox {
    pub fn new(id: impl Into<String>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            disabled: false,
            on_toggle: Rc::new(|_| {}),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

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
        let mut mark = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(px(5.5))
            .fill(fill)
            .align_child(Center, Center);
        mark = if self.checked {
            mark.child(symbol(Symbol::Checkmark, px(11.0), theme::glyph_on_tint()))
        } else {
            mark.border(1.0, theme::field_border())
        };

        let base = labelled(mark, self.label);
        let toggle = self.on_toggle;
        let next = !self.checked;
        interactive(base, self.id, self.disabled, on_press(move || toggle(next)))
    }
}

/// A mark and its optional label, 3 px apart, the shared shape of the
/// checkbox and the radio button.
pub(super) fn labelled(mark: Rectangle, label: Option<String>) -> Rectangle {
    let gap = px(3.0);
    let mut width = px(16.0);
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(mark)];
    if let Some(label) = label {
        width += gap + text_width(&label, theme::control_font()).ceil();
        parts.push(boxed(
            Text::new(label)
                .style(theme::control_font())
                .color(theme::label_primary()),
        ));
    }
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(theme::control_height()))
        .align_child(amane::Start, Center)
        .child(Row::new(parts).gap(gap).align(Center))
}
