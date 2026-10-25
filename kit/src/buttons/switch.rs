//! Switch — an on/off setting as a track and a knob.
//!
//! Modelled on Material 3's switch (a whole-track hit area, a hover state
//! layer, a disabled state), but drawn like iOS's: a capsule track that
//! fills with the accent as it turns on, and a round `accent.fg` knob that
//! springs from one end to the other. Off, the track is the
//! `state.selected` overlay, so it reads on any surface.
//!
//! Like the other controls it reflects state: a click moves the knob at
//! once, and if the service doesn't follow within [`PENDING_TIMEOUT`] it
//! goes back to what the service says.
//!
//! [`PENDING_TIMEOUT`]: super::PENDING_TIMEOUT

use std::rc::Rc;
use std::time::Instant;

use amane::{Animation, Blend, Center, Color, Radius, Rectangle, Size, Start, request_frame};

use super::{Actions, Pending, interactive, state_fill, with_existing, with_state};
use crate::layout::pad;
use crate::motion;
use crate::tokens::sys;

/// One switch's motion, kept across rebuilds under its id.
struct Motion {
    // 0.0 off … 1.0 on
    knob: Animation,
    pending: Pending<bool>,
}

/// An on/off switch. `id` keys its motion and hover, so it must be unique
/// and stable.
pub struct Switch {
    id: String,
    on: bool,
    disabled: bool,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl Switch {
    pub fn new(id: impl Into<String>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            disabled: false,
            on_toggle: Rc::new(|_| {}),
        }
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
        let width = sys::size::switch::width();
        let height = sys::size::switch::height();
        let inset = sys::space::gap::_2xs();
        let knob = (height - inset * 2.0).max(1.0);

        let (shown, progress) = self.animate();

        let track = Color::blend(
            sys::color::state::selected(),
            sys::color::accent::bg(),
            progress.clamp(0.0, 1.0),
        );
        let knob = Rectangle::new()
            .width(Size::Fixed(knob))
            .height(Size::Fixed(knob))
            .radius(Radius::Full)
            .fill(sys::color::accent::fg())
            .translate(progress * (width - height), 0.0);

        let base = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .radius(Radius::Full)
            .fill(state_fill(&self.id, track, self.disabled))
            .padding(pad(inset, inset))
            .align_child(Start, Center)
            .child(knob);

        let id = self.id.clone();
        let toggle = self.on_toggle;
        let next = !shown;
        let actions = Actions {
            press: Some(Rc::new(move || {
                clicked(&id, next);
                toggle(next);
            })),
            ..Actions::default()
        };

        interactive(base, self.id, self.disabled, actions)
    }

    /// Steps the knob towards what should show: the state, or a pending
    /// click laid over it. Gives that state and the knob's position.
    fn animate(&self) -> (bool, f32) {
        let init = || Motion {
            // at rest on the current state, so opening a panel doesn't animate
            knob: motion::spring(if self.on { 1.0 } else { 0.0 }),
            pending: Pending::NONE,
        };

        with_state(&self.id, init, |motion: &mut Motion| {
            let shown = motion.pending.shown(self.on, Instant::now());
            motion.knob.to(if shown { 1.0 } else { 0.0 });
            // an overshoot past either end stays inside the track
            (shown, motion.knob.value().clamp(0.0, 1.0))
        })
    }
}

/// Shows a click before the service answers it.
fn clicked(id: &str, on: bool) {
    with_existing(id, |motion: &mut Motion| {
        motion.pending.set(on, Instant::now())
    });
    request_frame();
}
