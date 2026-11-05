//! MacRadioButton — a 16 px circle; checked, the accent with a 4.8 px
//! white dot, unchecked `fieldFill` in a 1 px `fieldBorder`.

use std::rc::Rc;

use amane::{Center, Radius, Rectangle, Size};

use super::checkbox::labelled;
use super::theme::{self, px};
use super::{crossfade, interactive, on_press};

pub struct MacRadioButton {
    id: String,
    selected: bool,
    label: Option<String>,
    disabled: bool,
    on_select: Rc<dyn Fn()>,
}

impl MacRadioButton {
    pub fn new(id: impl Into<String>, selected: bool) -> Self {
        Self {
            id: id.into(),
            selected,
            label: None,
            disabled: false,
            on_select: Rc::new(|| {}),
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

    pub fn on_select(mut self, select: impl Fn() + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    pub fn build(self) -> Rectangle {
        let side = px(16.0);
        let fill = crossfade(
            &format!("{}.fill", self.id),
            self.selected,
            theme::field_fill(),
            theme::tint(),
        );
        let mut mark = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(fill)
            .align_child(Center, Center);
        mark = if self.selected {
            let dot = px(4.8);
            mark.child(
                Rectangle::new()
                    .width(Size::Fixed(dot))
                    .height(Size::Fixed(dot))
                    .radius(Radius::Full)
                    .fill(theme::glyph_on_tint()),
            )
        } else {
            mark.border(1.0, theme::field_border())
        };

        let base = labelled(mark, self.label);
        let select = self.on_select;
        interactive(base, self.id, self.disabled, on_press(move || select()))
    }
}
