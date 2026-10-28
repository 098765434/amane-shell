//! Edge reveal — a surface that hides at a screen edge and comes out when
//! the pointer touches the edge, as the macOS dock does with hiding on.
//!
//! While hidden, the surface takes the pointer only along a thin strip at
//! its edge ([`EdgeReveal::input_region`]); the pointer on that strip, or on
//! what it revealed, keeps it out, and once the pointer has been gone for
//! a delay it goes back in. Something else may hold it partly or wholly
//! out too (the dock follows niri's overview this way), and the reveal
//! shows by whichever is further out.
//!
//! It knows nothing of what it reveals: the view reads [`progress`]
//! (0 hidden … 1 out) and moves its content by it.
//!
//! [`progress`]: EdgeReveal::progress

use std::time::{Duration, Instant};

use amane::{Animation, InputArea};

use crate::motion;

crate::marker! {
    /// A reveal's delay ran out: the surfaces that read a reveal's
    /// progress draw again.
    Reveals
}

pub struct EdgeReveal {
    // 0 hidden … 1 out, on the motion it was made with
    motion: Animation,
    // the pointer is on the strip, or on what came out
    hovered: bool,
    // something keeps it out whatever the pointer does (the dock's menu)
    kept: bool,
    // when the pointer left, while the delay runs
    left: Option<Instant>,
    delay: Duration,
    // it hides at the top edge, not the bottom
    top: bool,
}

impl EdgeReveal {
    /// Hidden, coming out on `motion` and going back in `delay` after the
    /// pointer leaves.
    pub fn new(motion: Animation, delay: Duration) -> Self {
        Self {
            motion,
            hovered: false,
            kept: false,
            left: None,
            delay,
            top: false,
        }
    }

    /// Hides at the top edge instead of the bottom, as a bar along the top
    /// does: the strip that takes the pointer is along the top.
    pub fn at_top(mut self, top: bool) -> Self {
        self.top = top;
        self
    }

    pub fn set_top(&mut self, top: bool) {
        self.top = top;
    }

    /// The pointer came onto the surface (`true`) or left it.
    pub fn hover(&mut self, over: bool) {
        if over == self.hovered {
            return;
        }
        self.hovered = over;
        if over {
            self.left = None;
        } else {
            self.left = Some(Instant::now());
            // nothing else changes when the delay runs out
            motion::redraw_after::<Reveals>(self.delay + Duration::from_millis(10));
        }
    }

    /// Keeps it out (`true`) whether or not the pointer is on it, as the
    /// dock stays up while its menu is open; let go, it goes back in the
    /// delay after, unless the pointer is on it.
    pub fn keep_out(&mut self, kept: bool) {
        if kept == self.kept {
            return;
        }
        self.kept = kept;
        if !kept && !self.hovered {
            self.left = Some(Instant::now());
            motion::redraw_after::<Reveals>(self.delay + Duration::from_millis(10));
        }
    }

    /// Whether it's out or coming out: kept out, the pointer on the surface,
    /// or gone for less than the delay.
    pub fn wanted(&self) -> bool {
        self.kept || self.hovered || self.left.is_some_and(|left| left.elapsed() < self.delay)
    }

    /// Steps towards out or in and gives how far out it is, at least
    /// `hold` (0 … 1). Call it once a frame.
    pub fn progress(&mut self, hold: f32) -> f32 {
        Reveals::watch();
        if !self.wanted() {
            self.left = None;
        }
        self.motion.to(if self.wanted() { 1.0 } else { 0.0 });
        self.motion.value().max(hold.clamp(0.0, 1.0))
    }

    /// Where the surface, `width` by `height`, takes the pointer: the
    /// `strip` along its edge (the bottom, or the top [`at_top`]) while
    /// hidden, and `shown` too while any of it is out.
    ///
    /// [`at_top`]: EdgeReveal::at_top
    pub fn input_region(
        &self,
        width: f32,
        height: f32,
        strip: f32,
        progress: f32,
        shown: InputArea,
    ) -> Vec<InputArea> {
        let edge = InputArea {
            x: 0,
            y: if self.top {
                0
            } else {
                (height - strip).max(0.0) as i32
            },
            width: width.max(1.0) as i32,
            height: strip.max(1.0) as i32,
        };
        if progress > 0.001 || self.hovered || self.kept {
            vec![edge, shown]
        } else {
            vec![edge]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reveal(delay: u64) -> EdgeReveal {
        // no motion: progress follows at once
        EdgeReveal::new(
            Animation::new(0.0).duration(Duration::ZERO),
            Duration::from_millis(delay),
        )
    }

    fn area() -> InputArea {
        InputArea {
            x: 10,
            y: 0,
            width: 20,
            height: 30,
        }
    }

    #[test]
    fn it_comes_out_on_the_edge_and_goes_back_after_the_delay() {
        let mut edge = reveal(30);
        assert_eq!(edge.progress(0.0), 0.0);

        edge.hover(true);
        assert_eq!(edge.progress(0.0), 1.0);

        edge.hover(false);
        assert_eq!(edge.progress(0.0), 1.0, "it waits out the delay");

        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(edge.progress(0.0), 0.0);
    }

    #[test]
    fn coming_back_in_time_keeps_it_out() {
        let mut edge = reveal(30);
        edge.hover(true);
        edge.hover(false);
        edge.hover(true);
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(edge.progress(0.0), 1.0);
    }

    #[test]
    fn a_hold_shows_it_without_the_pointer() {
        let mut edge = reveal(30);
        assert_eq!(edge.progress(0.4), 0.4);
        assert_eq!(edge.progress(1.0), 1.0);
    }

    #[test]
    fn kept_out_it_stays_without_the_pointer_and_goes_the_delay_after() {
        let mut edge = reveal(30);
        edge.hover(true);
        edge.keep_out(true);
        edge.hover(false);
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(edge.progress(0.0), 1.0, "the pointer left long ago");

        edge.keep_out(false);
        assert_eq!(edge.progress(0.0), 1.0, "it waits out the delay");
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(edge.progress(0.0), 0.0);
    }

    #[test]
    fn hidden_it_takes_the_pointer_only_along_the_edge() {
        let mut edge = reveal(30);
        let hidden = edge.input_region(100.0, 50.0, 2.0, 0.0, area());
        assert_eq!(hidden.len(), 1);
        assert_eq!(
            (hidden[0].y, hidden[0].height, hidden[0].width),
            (48, 2, 100)
        );

        edge.hover(true);
        let out = edge.progress(0.0);
        assert_eq!(edge.input_region(100.0, 50.0, 2.0, out, area()).len(), 2);
    }

    #[test]
    fn at_the_top_the_strip_is_along_the_top() {
        let edge = reveal(30).at_top(true);
        let hidden = edge.input_region(100.0, 50.0, 2.0, 0.0, area());
        assert_eq!((hidden[0].y, hidden[0].height), (0, 2));
    }
}
