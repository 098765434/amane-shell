//! How big the kit's standard controls are: the five sizes of the macOS 27
//! kit, Mini to XL. A switch, checkbox, radio or slider knob reads its size
//! from the innermost [`scope`], Medium outside any. Each size's values are
//! `sys.size.*.<size>` and `sys.radius.*.<size>` tokens, density-scaled
//! like the rest of the sizes.
//!
//! Layout heights (rows, headers, the hit area of a thin slider) stay on
//! `sys.size.control.height`: a size changes the control drawn, not the
//! room the panels leave for it.

use std::cell::Cell;

use amane::Radius;

use crate::tokens::sys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    Mini,
    Small,
    Medium,
    Large,
    Xl,
}

thread_local! {
    static CURRENT: Cell<ControlSize> = const { Cell::new(ControlSize::Medium) };
}

/// Builds `f` with its controls at `size`.
pub fn scope<T>(size: ControlSize, f: impl FnOnce() -> T) -> T {
    let before = CURRENT.replace(size);
    let out = f();
    CURRENT.set(before);
    out
}

/// The size controls being built take.
pub fn current() -> ControlSize {
    CURRENT.get()
}

/// A switch's track and the pill-shaped knob inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchSize {
    pub width: f32,
    pub height: f32,
    pub knob: (f32, f32),
}

impl ControlSize {
    pub const ALL: [ControlSize; 5] = [
        Self::Mini,
        Self::Small,
        Self::Medium,
        Self::Large,
        Self::Xl,
    ];

    /// A control's height: a button, a switch's track.
    pub fn height(self) -> f32 {
        match self {
            Self::Mini => sys::size::control::mini(),
            Self::Small => sys::size::control::small(),
            Self::Medium => sys::size::control::medium(),
            Self::Large => sys::size::control::large(),
            Self::Xl => sys::size::control::xl(),
        }
    }

    /// A control's corners.
    pub fn radius(self) -> f32 {
        match self {
            Self::Mini => sys::radius::control::mini(),
            Self::Small => sys::radius::control::small(),
            Self::Medium => sys::radius::control::medium(),
            Self::Large => sys::radius::control::large(),
            Self::Xl => sys::radius::control::xl(),
        }
    }

    /// A button's corners: capsules from Large up, as in macOS 26.
    pub fn button_radius(self) -> Radius {
        match self {
            Self::Large | Self::Xl => Radius::Full,
            _ => Radius::Fixed(self.radius()),
        }
    }

    pub fn switch(self) -> SwitchSize {
        use sys::size::switch;

        let (width, knob) = match self {
            Self::Mini => (
                switch::mini::width(),
                (switch::mini::knob_width(), switch::mini::knob_height()),
            ),
            Self::Small => (
                switch::small::width(),
                (switch::small::knob_width(), switch::small::knob_height()),
            ),
            Self::Medium => (
                switch::medium::width(),
                (switch::medium::knob_width(), switch::medium::knob_height()),
            ),
            Self::Large => (
                switch::large::width(),
                (switch::large::knob_width(), switch::large::knob_height()),
            ),
            Self::Xl => (
                switch::xl::width(),
                (switch::xl::knob_width(), switch::xl::knob_height()),
            ),
        };

        SwitchSize {
            width,
            height: self.height(),
            knob,
        }
    }

    /// A checkbox's or radio's box, and the checkbox's corners.
    pub fn checkbox(self) -> (f32, f32) {
        match self {
            Self::Mini => (sys::size::checkbox::mini(), sys::radius::checkbox::mini()),
            Self::Small => (sys::size::checkbox::small(), sys::radius::checkbox::small()),
            Self::Medium => (
                sys::size::checkbox::medium(),
                sys::radius::checkbox::medium(),
            ),
            Self::Large => (sys::size::checkbox::large(), sys::radius::checkbox::large()),
            Self::Xl => (sys::size::checkbox::xl(), sys::radius::checkbox::xl()),
        }
    }

    /// The dot inside a chosen radio.
    pub fn radio_dot(self) -> f32 {
        match self {
            Self::Mini => sys::size::radio::mini::dot(),
            Self::Small => sys::size::radio::small::dot(),
            Self::Medium => sys::size::radio::medium::dot(),
            Self::Large => sys::size::radio::large::dot(),
            Self::Xl => sys::size::radio::xl::dot(),
        }
    }

    /// A thin slider's pill-shaped knob.
    pub fn slider_knob(self) -> (f32, f32) {
        use sys::size::slider;

        match self {
            Self::Mini => (slider::mini::knob_width(), slider::mini::knob_height()),
            Self::Small => (slider::small::knob_width(), slider::small::knob_height()),
            Self::Medium => (slider::medium::knob_width(), slider::medium::knob_height()),
            Self::Large => (slider::large::knob_width(), slider::large::knob_height()),
            Self::Xl => (slider::xl::knob_width(), slider::xl::knob_height()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_grow() {
        for pair in ControlSize::ALL.windows(2) {
            let (small, big) = (pair[0], pair[1]);
            assert!(small.height() < big.height(), "{small:?} → {big:?}");
            assert!(small.switch().width < big.switch().width);
            assert!(small.checkbox().0 <= big.checkbox().0);
        }
    }

    #[test]
    fn knobs_fit_their_tracks() {
        for size in ControlSize::ALL {
            let switch = size.switch();
            assert!(switch.knob.0 < switch.width, "{size:?}");
            assert!(switch.knob.1 < switch.height, "{size:?}");
        }
    }

    #[test]
    fn medium_outside_any_scope() {
        assert_eq!(current(), ControlSize::Medium);
        assert_eq!(scope(ControlSize::Large, current), ControlSize::Large);
        assert_eq!(current(), ControlSize::Medium);
    }

    #[test]
    fn large_buttons_are_capsules() {
        assert_eq!(ControlSize::Large.button_radius(), Radius::Full);
        assert_eq!(
            ControlSize::Medium.button_radius(),
            Radius::Fixed(ControlSize::Medium.radius())
        );
    }
}
