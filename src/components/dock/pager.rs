//! Turning the dock's pages: two fingers on a touchpad slide the icons up
//! or down to the next workspace's, following the fingers one to one and
//! settling on a page when they lift; a wheel steps a page a notch.
//!
//! Where it is is a page offset: 1.5 is half way from the second page to
//! the third. Only how far it is moves; which workspaces the pages are is
//! the view's business.

use std::time::{Duration, Instant};

use amane::{Animation, Scroll, ScrollSource};

use super::motion::{DockMotion, anim};

/// A swipe whose fingers haven't been heard of for this long has ended,
/// for a compositor that doesn't say when they lift.
const STALE: Duration = Duration::from_millis(200);

pub struct Pager {
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
    // pages a second, smoothed
    velocity: f32,
    last: Instant,
}

impl Pager {
    /// Resting on the first page.
    pub fn new(motion: &DockMotion) -> Self {
        Self {
            motion: anim(&motion.page, 0.0),
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
    pub fn offset(&self, count: usize, motion: &DockMotion) -> f32 {
        match &self.swipe {
            Some(swipe) => banded(swipe.offset, count, motion.rubber_band),
            None => self.motion.value(),
        }
    }

    /// Puts it on `page` at once: the dock coming out on the workspace
    /// that's showing.
    pub fn jump(&mut self, page: usize, motion: &DockMotion) {
        self.swipe = None;
        self.notches = 0.0;
        self.page = page;
        self.motion = anim(&motion.page, page as f32);
    }

    /// Settles on `page`, from wherever it is.
    pub fn go(&mut self, page: usize, count: usize, motion: &DockMotion) {
        let page = page.min(count.saturating_sub(1));
        let from = self.offset(count, motion);
        self.swipe = None;
        self.page = page;
        self.motion = anim(&motion.page, from);
        self.motion.to(page as f32);
    }

    /// A scroll over the dock, `page_px` the distance that turns a page.
    pub fn scroll(&mut self, scroll: Scroll, page_px: f32, count: usize, motion: &DockMotion) {
        if count == 0 {
            return;
        }
        match scroll.source {
            ScrollSource::Finger | ScrollSource::Continuous => {
                self.follow(scroll, page_px, count, motion)
            }
            ScrollSource::Wheel | ScrollSource::Tilt => self.step(scroll.y, count, motion),
        }
    }

    /// Fingers moving, or lifting.
    fn follow(&mut self, scroll: Scroll, page_px: f32, count: usize, motion: &DockMotion) {
        let now = Instant::now();
        let moved = scroll.pixels().1 / page_px.max(1.0);
        let swipe = self.swipe.get_or_insert_with(|| Swipe {
            offset: self.motion.value(),
            velocity: 0.0,
            last: now,
        });

        if moved != 0.0 {
            let dt = now.duration_since(swipe.last).as_secs_f32().max(0.004);
            swipe.offset += moved;
            // the latest movement counts most: a flick is how it ends
            swipe.velocity = swipe.velocity * 0.5 + (moved / dt) * 0.5;
            swipe.last = now;
        }

        if scroll.stop {
            self.release(count, motion);
        }
    }

    /// The fingers lifted: a flick turns the page however far it got, else
    /// it settles on the nearest one; never more than a page from where the
    /// swipe began.
    fn release(&mut self, count: usize, motion: &DockMotion) {
        let Some(swipe) = &self.swipe else {
            return;
        };
        let from = self.page as f32;
        let target = if swipe.velocity.abs() >= motion.flick {
            from + swipe.velocity.signum()
        } else {
            swipe.offset.round()
        };
        let target = target.clamp(from - 1.0, from + 1.0);
        let last = count.saturating_sub(1) as f32;
        self.go(target.clamp(0.0, last) as usize, count, motion);
    }

    /// A swipe whose end never came ends now. Call it once a frame.
    pub fn settle_stale(&mut self, count: usize, motion: &DockMotion) {
        if self
            .swipe
            .as_ref()
            .is_some_and(|swipe| swipe.last.elapsed() >= STALE)
        {
            self.release(count, motion);
        }
    }

    /// A wheel: a page a notch, part-notches adding up.
    fn step(&mut self, lines: f32, count: usize, motion: &DockMotion) {
        self.notches += lines;
        if self.notches.abs() < 1.0 {
            return;
        }
        let way = self.notches.signum();
        self.notches = 0.0;
        let page = (self.page as f32 + way).clamp(0.0, count.saturating_sub(1) as f32);
        self.go(page as usize, count, motion);
    }
}

/// `offset` within the pages, pulled only `band` of the way past an end
/// (or stopped there without one).
fn banded(offset: f32, count: usize, band: Option<f32>) -> f32 {
    let last = count.saturating_sub(1) as f32;
    let edge = offset.clamp(0.0, last);
    match band {
        Some(band) => edge + (offset - edge) * band,
        None => edge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::dock::motion::{Anim, DOCK};

    /// The dock's motion with paging that arrives at once.
    fn still() -> DockMotion {
        DockMotion {
            page: Anim::ease(0).off(),
            ..DOCK
        }
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
        let mut pager = Pager::new(&motion);
        pager.scroll(finger(30.0), 60.0, 3, &motion);
        assert!(pager.swiping());
        assert!((pager.offset(3, &motion) - 0.5).abs() < 0.001);
    }

    #[test]
    fn a_slow_swipe_settles_on_the_nearest_page() {
        let motion = DockMotion {
            flick: f32::INFINITY,
            ..still()
        };
        let mut pager = Pager::new(&motion);
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
        let motion = DockMotion {
            flick: 0.1,
            ..still()
        };
        let mut pager = Pager::new(&motion);
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
        let mut pager = Pager::new(&motion);
        pager.scroll(finger(-60.0), 60.0, 2, &motion);
        let band = motion.rubber_band.expect("banded as shipped");
        assert!((pager.offset(2, &motion) + band).abs() < 0.001);
        pager.scroll(lift(), 60.0, 2, &motion);
        assert_eq!(pager.page(), 0);

        let stops = DockMotion {
            rubber_band: None,
            ..still()
        };
        let mut pager = Pager::new(&stops);
        pager.scroll(finger(-60.0), 60.0, 2, &stops);
        assert_eq!(pager.offset(2, &stops), 0.0);
    }

    #[test]
    fn a_wheel_steps_a_page_a_notch() {
        let motion = still();
        let mut pager = Pager::new(&motion);
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
        let motion = DockMotion {
            flick: f32::INFINITY,
            ..still()
        };
        let mut pager = Pager::new(&motion);
        pager.scroll(finger(50.0), 60.0, 3, &motion);
        std::thread::sleep(STALE + Duration::from_millis(10));
        pager.settle_stale(3, &motion);
        assert!(!pager.swiping());
        assert_eq!(pager.page(), 1);
    }
}
