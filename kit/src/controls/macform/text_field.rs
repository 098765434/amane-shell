//! MacTextField — a 24 px field, radius 6: `fieldFill` in a 1 px
//! `fieldBorder`, the text 8 px in. A field that fails its check on Enter
//! turns its edge red (#ff6961) until the next edit.
//!
//! The caret and selection take the accent, where the original took the
//! wallpaper's colours; focused, the edge takes the accent too (the
//! original showed no focus at all).

use std::rc::Rc;

use amane::{Center, Rectangle, Size, TextInput};

use super::theme::{self, px};
use crate::buttons::with_state;
use crate::color::with_alpha;

/// A check run on the text.
type Check = Rc<dyn Fn(&str) -> bool>;

pub struct MacTextField {
    id: &'static str,
    placeholder: String,
    width: Size,
    error: Option<bool>,
    password: bool,
    focused: bool,
    validate: Option<Check>,
    on_change: Rc<dyn Fn(String)>,
    on_submit: Rc<dyn Fn(String)>,
}

/// Whether field `id` failed its check, until it's edited.
fn failed(id: &str) -> bool {
    with_state(
        &format!("{id}.failed"),
        || false,
        |failed: &mut bool| *failed,
    )
}

fn set_failed(id: &str, value: bool) {
    with_state(
        &format!("{id}.failed"),
        || false,
        |failed: &mut bool| *failed = value,
    );
}

impl MacTextField {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            placeholder: String::new(),
            width: Size::Parent,
            error: None,
            password: false,
            focused: false,
            validate: None,
            on_change: Rc::new(|_| {}),
            on_submit: Rc::new(|_| {}),
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn width(mut self, width: impl Into<Size>) -> Self {
        self.width = width.into();
        self
    }

    /// Shows the red edge regardless of any check.
    pub fn error(mut self, error: bool) -> Self {
        self.error = Some(error);
        self
    }

    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    /// Takes the keyboard as soon as it's shown.
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    /// Checked on Enter; a field that fails is marked until it's edited.
    /// An empty field passes.
    pub fn validate(mut self, check: impl Fn(&str) -> bool + 'static) -> Self {
        self.validate = Some(Rc::new(check));
        self
    }

    pub fn on_change(mut self, change: impl Fn(String) + 'static) -> Self {
        self.on_change = Rc::new(change);
        self
    }

    /// Runs on Enter, with the text, if it passes the check.
    pub fn on_submit(mut self, submit: impl Fn(String) + 'static) -> Self {
        self.on_submit = Rc::new(submit);
        self
    }

    pub fn build(self) -> Rectangle {
        let id = self.id;
        let error = self.error.unwrap_or_else(|| failed(id));
        let edge = if error {
            theme::error()
        } else if TextInput::has_focus(id) {
            theme::tint()
        } else {
            theme::field_border()
        };

        let change = self.on_change;
        let submit = self.on_submit;
        let check = self.validate;
        let mut input = TextInput::new(id)
            .placeholder(self.placeholder)
            .size(theme::control_font().size)
            .color(theme::label_primary())
            .selection_color(with_alpha(theme::tint(), 0.4))
            .on_change(move |text| {
                set_failed(id, false);
                change(text);
            })
            .on_submit(move |text| {
                let ok = text.is_empty() || check.as_ref().is_none_or(|check| check(&text));
                set_failed(id, !ok);
                amane::mark_all();
                if ok {
                    submit(text);
                }
            });
        if self.password {
            input = input.password();
        }
        if self.focused {
            input = input.focused();
        }

        Rectangle::new()
            .width(self.width)
            .height(Size::Fixed(theme::control_height()))
            .radius(theme::field_radius())
            .fill(theme::field_fill())
            .border(1.0, edge)
            .padding(amane::Padding {
                left: px(8.0),
                right: px(4.0),
                top: 0.0,
                bottom: 0.0,
            })
            .align_child(amane::Start, Center)
            .clip()
            .child(input)
    }
}
