//! The dock's motion, all in one place: every animation it runs is an
//! [`Anim`] in [`DOCK`], each with its own switch, and the magnification
//! and tooltip can go altogether. `AMANE_DOCK_ANIMATIONS=off` turns every
//! animation off at once ([`motion`]); the dock then jumps where it would
//! have moved, and still magnifies and hides as configured.
//!
//! The view never asks whether an animation is on: it makes each one with
//! [`anim`], which gives a motion that arrives at once when it's off.

use std::sync::OnceLock;
use std::time::Duration;

use amane::{Animation, Blend, Easing};

use crate::theme;

// TODO(token): these become `dock.motion.*` tokens once Penpot has them
// (manual §1.1; docs/penpot-todo.md).
/// The dock's motion as shipped.
pub const DOCK: DockMotion = DockMotion {
    enabled: true,
    reveal: Anim::spring(380, 0.08),
    hide_delay: Duration::from_millis(400),
    follow_overview: true,
    resize: Anim::spring(450, 0.1),
    item: Anim::spring(450, 0.15),
    magnify: Some(Magnify {
        scales: [1.5, 1.2, 1.1],
        lifts: [10.0, 6.0, 0.0],
        smooth: true,
        hover: Anim::ease(120),
    }),
    tooltip: Some(Anim::ease(120)),
    spring_load: Some(Duration::from_millis(700)),
};

/// Every animation the dock runs, and whether it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockMotion {
    /// `false` jumps every animation below (they still happen, at once).
    pub enabled: bool,
    /// Sliding up out of the bottom edge and back down, the frost fading
    /// with it.
    pub reveal: Anim,
    /// How long the dock stays out once the pointer has left it.
    pub hide_delay: Duration,
    /// Whether the dock rises with niri's overview, in step with its zoom.
    pub follow_overview: bool,
    /// The card widening and narrowing as windows come and go.
    pub resize: Anim,
    /// A window's icon growing in as it opens, and its neighbours making
    /// room or closing up.
    pub item: Anim,
    /// The icons swelling under the pointer; `None` keeps them still.
    pub magnify: Option<Magnify>,
    /// The app's name over the icon under the pointer; `None` shows none.
    pub tooltip: Option<Anim>,
    /// How long files held over an icon wait before its window comes into
    /// view, so they can be let go in it; `None` waits for the drop.
    pub spring_load: Option<Duration>,
}

/// One animation: how long, how bouncy, and whether it runs at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anim {
    pub on: bool,
    pub duration: Duration,
    pub easing: Easing,
}

impl Anim {
    /// A spring settling in `millis` with `bounce` (0 arrives without
    /// overshooting).
    pub const fn spring(millis: u64, bounce: f32) -> Self {
        Self {
            on: true,
            duration: Duration::from_millis(millis),
            easing: Easing::Spring { bounce },
        }
    }

    /// An ease-out over `millis`.
    pub const fn ease(millis: u64) -> Self {
        Self {
            on: true,
            duration: Duration::from_millis(millis),
            easing: Easing::Out,
        }
    }

    /// The same, switched off: for turning one animation off in [`DOCK`].
    #[allow(dead_code)]
    pub const fn off(self) -> Self {
        Self { on: false, ..self }
    }
}

/// The macOS magnification: the icon under the pointer and its neighbours
/// swell and rise, the dock spreading to make room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Magnify {
    /// How big the icon under the pointer is, then one and two along.
    pub scales: [f32; 3],
    /// How far each of those rises, in logical pixels.
    pub lifts: [f32; 3],
    /// Follow the pointer across an icon smoothly, rather than stepping
    /// from one to the next as the reference CSS does.
    pub smooth: bool,
    /// The swell coming on as the pointer arrives, and going as it leaves.
    pub hover: Anim,
}

impl Magnify {
    /// The largest scale and lift, for the room above the dock.
    pub fn most(&self) -> (f32, f32) {
        let scale = self.scales.iter().copied().fold(1.0, f32::max);
        let lift = self.lifts.iter().copied().fold(0.0, f32::max);
        (scale, lift)
    }
}

/// The dock's motion: [`DOCK`], every animation off when
/// `AMANE_DOCK_ANIMATIONS=off` (or `0`, `false`, `no`).
pub fn motion() -> &'static DockMotion {
    static MOTION: OnceLock<DockMotion> = OnceLock::new();
    MOTION.get_or_init(|| {
        let var = std::env::var("AMANE_DOCK_ANIMATIONS").ok();
        configured(DOCK, var.as_deref())
    })
}

/// `motion` with every animation off when `var` says so.
fn configured(motion: DockMotion, var: Option<&str>) -> DockMotion {
    let off = var.is_some_and(|var| {
        matches!(
            var.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    });
    DockMotion {
        enabled: motion.enabled && !off,
        ..motion
    }
}

/// An animation resting at `value` that runs as `anim` says, or arrives at
/// once when it or the dock's motion is off, or motion is reduced.
pub fn anim<T: Blend>(anim: &Anim, value: T) -> Animation<T> {
    let runs = anim.on && motion().enabled && !theme::reduced_motion();
    Animation::new(value)
        .duration(if runs { anim.duration } else { Duration::ZERO })
        .easing(anim.easing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variable_turns_every_animation_off() {
        assert!(configured(DOCK, None).enabled);
        assert!(configured(DOCK, Some("on")).enabled);
        assert!(!configured(DOCK, Some("off")).enabled);
        assert!(!configured(DOCK, Some(" OFF ")).enabled);
        assert!(!configured(DOCK, Some("0")).enabled);
    }

    #[test]
    fn an_animation_that_is_off_arrives_at_once() {
        let mut off = anim(&DOCK.item.off(), 0.0);
        off.to(1.0);
        assert_eq!(off.value(), 1.0);

        let mut on = anim(&DOCK.item, 0.0);
        on.to(1.0);
        if motion().enabled {
            assert!(on.value() < 1.0, "a running one takes its time");
        }
    }

    #[test]
    fn the_most_magnification_makes_room_above() {
        let magnify = DOCK.magnify.expect("magnifies as shipped");
        assert_eq!(magnify.most(), (1.5, 10.0));
    }
}
