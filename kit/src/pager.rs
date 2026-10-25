//! Turning pages with the fingers: two fingers on a touchpad slide to the
//! next page, following them one to one and settling on a page when they
//! lift; a flick turns the page however far it got, and a wheel steps a
//! page a notch. The dock pages through workspaces with it, up and down;
//! the launcher through its categories, sideways.
//!
//! Where it is is a page offset: 1.5 is half way from the second page to
//! the third. Only how far it is moves; what the pages are is the view's
//! business.

use std::time::{Duration, Instant};

use amane::{Animation, Easing, Scroll, ScrollSource};

use crate::kinetic::{Axis, Tracker, banded};

/// How a pager moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagerMotion {
    /// How long settling on a page takes (zero arrives at once), and on
    /// what curve.
    pub duration: Duration,
    pub easing: Easing,
    /// How fast a swipe (pages a second) turns the page however far it got.
    pub flick: f32,
    /// How far past the first and last page a swipe pulls, as a share of
    /// the fingers' travel; `None` stops at the ends.
    pub rubber_band: Option<f32>,
}

impl PagerMotion {
    fn anim(&self, value: f32) -> Animation {
        Animation::new(value)
            .duration(self.duration)
            .easing(self.easing)
    }
}

pub struct Pager {
    axis: Axis,
    // where it's settling, in pages
    motion: Animation,
    // the page it's heading for, or that the swipe set out from
    page: usize,
    swipe: Option<Swipe>,
    // a wheel's part-notches, until they add up to one
    notches: f32,
}

/// Fingers on the touchpad.
struct Swipe {
    // in pages, past the ends too
    offset: f32,
    // in pages a second
    tracker: Tracker,
}

impl Pager {
    /// Resting on the first page, turning along `axis`.
    pub fn new(axis: Axis, motion: &PagerMotion) -> Self {
        Self {
            axis,
            motion: motion.anim(0.0),
            page: 0,
            swipe: None,
            notches: 0.0,
        }
    }

    /// The page it's on or heading for.
    pub fn page(&self) -> usize {
        self.page
    }

    /// Whether fingers are on it.
    pub fn swiping(&self) -> bool {
        self.swipe.is_some()
    }

    /// Where it is now, in pages: under the fingers (pulled past an end by
    /// the rubber band), or settling.
    pub fn offset(&self, count: usize, motion: &PagerMotion) -> f32 {
        match &self.swipe {
            Some(swipe) => banded(swipe.offset, last(count), motion.rubber_band),
            None => self.motion.value(),
        }
    }

    /// Puts it on `page` at once: the dock coming out on the workspace
    /// that's showing.
    pub fn jump(&mut self, page: usize, motion: &PagerMotion) {
        self.swipe = None;
        self.notches = 0.0;
        self.page = page;
        self.motion = motion.anim(page as f32);
    }

    /// Settles on `page`, from wherever it is.
    pub fn go(&mut self, page: usize, count: usize, motion: &PagerMotion) {
        let page = page.min(count.saturating_sub(1));
        let from = self.offset(count, motion);
        self.swipe = None;
        self.page = page;
        self.motion = motion.anim(from);
        self.motion.to(page as f32);
    }

    /// A scroll along its axis, `page_px` the distance that turns a page.
    pub fn scroll(&mut self, scroll: Scroll, page_px: f32, count: usize, motion: &PagerMotion) {
        if count == 0 {
            return;
        }
        match scroll.source {
            ScrollSource::Finger | ScrollSource::Continuous => self.follow(
                self.axis.pixels(&scroll),
                scroll.stop,
                page_px,
                count,
                motion,
            ),
            ScrollSource::Wheel | ScrollSource::Tilt => {
                self.step(self.axis.lines(&scroll), count, motion)
            }
        }
    }

    /// Fingers moving `moved` pixels along its axis, or lifting if `stop`.
    pub fn follow(
        &mut self,
        moved: f32,
        stop: bool,
        page_px: f32,
        count: usize,
        motion: &PagerMotion,
    ) {
        let now = Instant::now();
        let moved = moved / page_px.max(1.0);
        let start = self.motion.value();
        let swipe = self.swipe.get_or_insert_with(|| Swipe {
            offset: start,
            tracker: Tracker::new(now),
        });
        swipe.offset += moved;
        swipe.tracker.feed(moved, now);

        if stop {
            self.release(count, motion);
        }
    }

    /// The fingers lifted: a flick turns the page however far it got, else
    /// it settles on the nearest one; never more than a page from where the
    /// swipe began.
    fn release(&mut self, count: usize, motion: &PagerMotion) {
        let Some(swipe) = &self.swipe else {
            return;
        };
        let from = self.page as f32;
        let velocity = swipe.tracker.velocity();
        let target = if velocity.abs() >= motion.flick {
            from + velocity.signum()
        } else {
            swipe.offset.round()
        };
        let target = target.clamp(from - 1.0, from + 1.0);
        self.go(target.clamp(0.0, last(count)) as usize, count, motion);
    }

    /// A swipe whose end never came ends now. Call it once a frame.
    pub fn settle_stale(&mut self, count: usize, motion: &PagerMotion) {
        if self
            .swipe
            .as_ref()
            .is_some_and(|swipe| swipe.tracker.stale(Instant::now()))
        {
            self.release(count, motion);
        }
    }

    /// A wheel: a page a notch, part-notches adding up.
    pub fn step(&mut self, lines: f32, count: usize, motion: &PagerMotion) {
        self.notches += lines;
        if self.notches.abs() < 1.0 {
            return;
        }
        let way = self.notches.signum();
        self.notches = 0.0;
        let page = (self.page as f32 + way).clamp(0.0, last(count));
        self.go(page as usize, count, motion);
    }
}

/// The last page's offset.
fn last(count: usize) -> f32 {
    count.saturating_sub(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinetic::STALE;

    /// Paging that arrives at once.
    fn still() -> PagerMotion {
        PagerMotion {
            duration: Duration::ZERO,
            easing: Easing::Out,
            flick: 1.2,
            rubber_band: Some(0.3),
        }
    }

    fn vertical(motion: &PagerMotion) -> Pager {
        Pager::new(Axis::Vertical, motion)
    }

    fn finger(pixels: f32) -> Scroll {
        let per_line = Scroll {
            y: 1.0,
            ..Scroll::default()
        }
        .pixels()
        .1;
        Scroll {
            y: pixels / per_line,
            source: ScrollSource::Finger,
            ..Scroll::default()
        }
    }

    fn lift() -> Scroll {
        Scroll {
            source: ScrollSource::Finger,
            stop: true,
            ..Scroll::default()
        }
    }

    fn wheel(lines: f32) -> Scroll {
        Scroll {
            y: lines,
            ..Scroll::default()
        }
    }

    #[test]
    fn the_icons_follow_the_fingers() {
        let motion = still();
        let mut pager = vertical(&motion);
        pager.scroll(finger(30.0), 60.0, 3, &motion);
        assert!(pager.swiping());
        assert!((pager.offset(3, &motion) - 0.5).abs() < 0.001);
    }

    #[test]
    fn a_slow_swipe_settles_on_the_nearest_page() {
        let motion = PagerMotion {
            flick: f32::INFINITY,
            ..still()
        };
        let mut pager = vertical(&motion);
        pager.scroll(finger(20.0), 60.0, 3, &motion);
        pager.scroll(lift(), 60.0, 3, &motion);
        assert_eq!(pager.page(), 0);

        pager.scroll(finger(40.0), 60.0, 3, &motion);
        pager.scroll(lift(), 60.0, 3, &motion);
        assert_eq!(pager.page(), 1);
        assert_eq!(pager.offset(3, &motion), 1.0);
    }

    #[test]
    fn a_flick_turns_the_page_but_only_one() {
        let motion = PagerMotion {
            flick: 0.1,
            ..still()
        };
        let mut pager = vertical(&motion);
        // a short, fast flick
        pager.scroll(finger(6.0), 60.0, 5, &motion);
        pager.scroll(lift(), 60.0, 5, &motion);
        assert_eq!(pager.page(), 1);

        // a long one goes no further than the next page
        pager.scroll(finger(200.0), 60.0, 5, &motion);
        pager.scroll(lift(), 60.0, 5, &motion);
        assert_eq!(pager.page(), 2);
    }

    #[test]
    fn past_the_ends_the_rubber_band_pulls_back() {
        let motion = still();
        let mut pager = vertical(&motion);
        pager.scroll(finger(-60.0), 60.0, 2, &motion);
        let band = motion.rubber_band.expect("banded as shipped");
        assert!((pager.offset(2, &motion) + band).abs() < 0.001);
        pager.scroll(lift(), 60.0, 2, &motion);
        assert_eq!(pager.page(), 0);

        let stops = PagerMotion {
            rubber_band: None,
            ..still()
        };
        let mut pager = vertical(&stops);
        pager.scroll(finger(-60.0), 60.0, 2, &stops);
        assert_eq!(pager.offset(2, &stops), 0.0);
    }

    #[test]
    fn a_wheel_steps_a_page_a_notch() {
        let motion = still();
        let mut pager = vertical(&motion);
        pager.scroll(wheel(0.5), 60.0, 3, &motion);
        assert_eq!(pager.page(), 0, "half a notch isn't one");
        pager.scroll(wheel(0.5), 60.0, 3, &motion);
        assert_eq!(pager.page(), 1);
        pager.scroll(wheel(3.0), 60.0, 3, &motion);
        assert_eq!(pager.page(), 2);
        pager.scroll(wheel(1.0), 60.0, 3, &motion);
        assert_eq!(pager.page(), 2, "stops at the last");
        pager.scroll(wheel(-1.0), 60.0, 3, &motion);
        assert_eq!(pager.page(), 1);
    }

    #[test]
    fn a_swipe_without_an_end_ends_on_its_own() {
        let motion = PagerMotion {
            flick: f32::INFINITY,
            ..still()
        };
        let mut pager = vertical(&motion);
        pager.scroll(finger(50.0), 60.0, 3, &motion);
        std::thread::sleep(STALE + Duration::from_millis(10));
        pager.settle_stale(3, &motion);
        assert!(!pager.swiping());
        assert_eq!(pager.page(), 1);
    }
}
