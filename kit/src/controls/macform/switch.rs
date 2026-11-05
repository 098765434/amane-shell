//! MacSwitch — a 36×16 capsule track and a 21×13 white knob.
//!
//! Off, the track is `fieldFill`; on, the accent, crossfading over 300 ms
//! while the knob springs across (the original's overshooting 350 ms
//! curve). Unlike the original it lights on hover and dims when disabled.

use std::rc::Rc;

use amane::{Animation, Center, Radius, Rectangle, Size, Start};

use super::theme::{self, px};
use super::{crossfade, hover_fill, interactive, on_press};
use crate::buttons::with_state;
use crate::layout::pad;
use crate::motion;

pub struct MacSwitch {
    id: String,
    on: bool,
    disabled: bool,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl MacSwitch {
    pub fn new(id: impl Into<String>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            disabled: false,
            on_toggle: Rc::new(|_| {}),
        }
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
        let (track_w, track_h) = (px(36.0), px(16.0));
        let inset = px(1.5);
        let knob_w = px(21.0);
        let knob_h = track_h - inset * 2.0;

        let target = if self.on { 1.0 } else { 0.0 };
        let progress = with_state(
            &format!("{}.knob", self.id),
            || motion::spring(target),
            |knob: &mut Animation| {
                knob.to(target);
                knob.value().clamp(0.0, 1.0)
            },
        );

        let track = crossfade(
            &format!("{}.track", self.id),
            self.on,
            theme::field_fill(),
            theme::tint(),
        );

        let knob = Rectangle::new()
            .width(Size::Fixed(knob_w))
            .height(Size::Fixed(knob_h))
            .radius(Radius::Full)
            .fill(theme::knob())
            .translate(progress * (track_w - knob_w - inset * 2.0), 0.0);

        let base = Rectangle::new()
            .width(Size::Fixed(track_w))
            .height(Size::Fixed(track_h))
            .radius(Radius::Full)
            .fill(hover_fill(
                &self.id,
                track,
                theme::label_primary(),
                self.disabled,
            ))
            .padding(pad(inset, inset))
            .align_child(Start, Center)
            .child(knob);

        let toggle = self.on_toggle;
        let next = !self.on;
        interactive(base, self.id, self.disabled, on_press(move || toggle(next)))
    }
}
