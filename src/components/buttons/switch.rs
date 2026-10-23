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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use amane::{
    Animation, Blend, Center, Color, Easing, Radius, Rectangle, Size, Start, request_frame,
};

use super::{Actions, PENDING_TIMEOUT, SLIDE, SLIDE_BOUNCE, interactive, state_fill};
use crate::components::taskbar::pad;
use crate::tokens::sys;

/// One switch's motion, kept across rebuilds under its id.
struct Motion {
    // 0.0 off … 1.0 on
    knob: Animation,
    // what a click asked for, until the service agrees or it times out
    pending: Option<(bool, Instant)>,
}

thread_local! {
    static MOTION: RefCell<HashMap<&'static str, Motion>> = RefCell::new(HashMap::new());
}

/// An on/off switch. `id` keys its motion and hover, so it must be unique
/// and stable.
pub struct Switch {
    id: &'static str,
    on: bool,
    disabled: bool,
    on_toggle: Rc<dyn Fn(bool)>,
}

impl Switch {
    pub fn new(id: &'static str, on: bool) -> Self {
        Self {
            id,
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
            .fill(state_fill(self.id, track, self.disabled))
            .padding(pad(inset, inset))
            .align_child(Start, Center)
            .child(knob);

        let id = self.id;
        let toggle = self.on_toggle;
        let next = !shown;
        let actions = Actions {
            press: Some(Rc::new(move || {
                clicked(id, next);
                toggle(next);
            })),
            secondary: None,
        };

        interactive(base, id.to_string(), self.disabled, actions)
    }

    /// Steps the knob towards what should show: the state, or a pending
    /// click laid over it. Gives that state and the knob's position.
    fn animate(&self) -> (bool, f32) {
        let now = Instant::now();

        MOTION.with_borrow_mut(|motions| {
            let motion = motions.entry(self.id).or_insert_with(|| Motion {
                // at rest on the current state, so opening a panel doesn't animate
                knob: Animation::new(if self.on { 1.0 } else { 0.0 })
                    .duration(SLIDE)
                    .easing(Easing::Spring {
                        bounce: SLIDE_BOUNCE,
                    }),
                pending: None,
            });

            if let Some((on, at)) = motion.pending
                && (self.on == on || now - at > PENDING_TIMEOUT)
            {
                motion.pending = None;
            }
            let shown = motion.pending.map_or(self.on, |(on, _)| on);

            motion.knob.to(if shown { 1.0 } else { 0.0 });
            // an overshoot past either end stays inside the track
            (shown, motion.knob.value().clamp(0.0, 1.0))
        })
    }
}

/// Shows a click before the service answers it.
fn clicked(id: &'static str, on: bool) {
    MOTION.with_borrow_mut(|motions| {
        if let Some(motion) = motions.get_mut(id) {
            motion.pending = Some((on, Instant::now()));
        }
    });

    request_frame();
}
