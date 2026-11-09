//! A surface opening and closing: a spring to 1 as it opens, a little past
//! it and back, and a quicker ease to 0 as it closes. A change of mind turns
//! it around at the speed it had. The alert, the launcher and Quick Settings
//! open this way.

use amane::{Animation, Blend, BlurShape};

use crate::{glass_region, motion};

/// How far open a surface has to be to be drawn at all.
const SHOWING: f32 = 0.001;

/// Whether a surface heading `open`, `shown` far open, is drawn at all:
/// while it opens, and until it has closed.
pub fn showing(open: bool, shown: f32) -> bool {
    open || shown > SHOWING
}

/// How a surface swelling open is drawn this frame: growing from a little
/// smaller about its centre, fading in, its frost following.
///
/// A compositor that can't fade frost only shows or hides it, so there the
/// surface arrives whole while it opens, rather than fading in over frost
/// at full strength, and its frost goes at once as it closes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swell {
    /// How big it's drawn, from `from_scale` closed to 1 open.
    pub scale: f32,
    /// How strongly it's drawn.
    pub opacity: f32,
    /// Whether its frost shows.
    pub frost: bool,
}

impl Swell {
    /// A surface heading `open`, `shown` far open (a little past 1 on the
    /// spring), that grows from `from_scale`.
    pub fn new(open: bool, shown: f32, from_scale: f32) -> Self {
        let fades = amane::blur_fades();
        Self {
            scale: f32::blend(motion::scale(from_scale), 1.0, shown),
            opacity: if open && !fades {
                1.0
            } else {
                shown.clamp(0.0, 1.0)
            },
            frost: open || fades,
        }
    }

    /// `shapes`, the surface's frost at its full size, scaled about
    /// `centre` and faded with it; none while its frost doesn't show.
    pub fn frost(&self, shapes: Vec<BlurShape>, centre: (f32, f32)) -> Vec<BlurShape> {
        self.frost_moved(shapes, centre, (0.0, 0.0))
    }

    /// As [`frost`](Self::frost), moved by `offset` after scaling.
    pub fn frost_moved(
        &self,
        shapes: Vec<BlurShape>,
        centre: (f32, f32),
        offset: (f32, f32),
    ) -> Vec<BlurShape> {
        if !self.frost {
            return Vec::new();
        }
        glass_region::faded(
            glass_region::transformed(shapes, self.scale, centre, offset),
            self.opacity,
        )
    }
}

/// How far a surface has opened, 0 closed to 1 open (a little past 1 on the spring).
pub struct Reveal {
    shown: Animation,
    open: bool,
}

impl Reveal {
    /// Closed.
    pub fn new() -> Self {
        Self {
            shown: motion::pop(0.0),
            open: false,
        }
    }

    /// Steps towards open or closed and gives how far it is.
    pub fn step(&mut self, open: bool) -> f32 {
        self.step_then(open, |_| {})
    }

    /// As [`step`](Self::step), calling `turned` with how far it was when it turns towards
    /// `open`, before it starts moving that way.
    pub fn step_then(&mut self, open: bool, turned: impl FnOnce(f32)) -> f32 {
        if self.open != open {
            self.open = open;
            turned(self.shown.value());
            self.shown.retime(if open {
                motion::duration::pop()
            } else {
                motion::duration::close()
            });
            self.shown.to(if open { 1.0 } else { 0.0 });
        }
        self.shown.value()
    }

    /// How far it is, without stepping.
    pub fn value(&self) -> f32 {
        self.shown.value()
    }
}

impl Default for Reveal {
    fn default() -> Self {
        Self::new()
    }
}
