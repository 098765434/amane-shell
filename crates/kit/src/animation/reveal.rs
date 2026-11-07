//! A surface opening and closing: a spring to 1 as it opens, a little past
//! it and back, and a quicker ease to 0 as it closes. A change of mind turns
//! it around at the speed it had. The alert, the launcher and Quick Settings
//! open this way.

use amane::Animation;

use crate::motion;

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
            self.shown
                .retime(if open { motion::POP } else { motion::CLOSE });
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
