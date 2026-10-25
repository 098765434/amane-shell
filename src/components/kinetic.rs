//! Fingers on a touchpad, and what they leave behind: how fast they were
//! going ([`Tracker`]), and a scroll that follows them one to one and
//! coasts on after they lift, slowing to a stop ([`Kinetic`]).
//!
//! The compositor reports fingers moving and then lifting (a scroll's
//! `stop`); the momentum after that is the client's to draw. Here a fling
//! is only an animation: where the scroll would come to rest is worked out
//! the moment the fingers lift, and an ease-out carries it there. Past an
//! end, the scroll pulls on a rubber band and springs back.

use std::time::{Duration, Instant};

use amane::{Animation, Easing, Scroll, ScrollSource};

use crate::components::motion;

/// A gesture whose fingers haven't been heard of for this long has ended,
/// for a compositor that doesn't say when they lift.
pub const STALE: Duration = Duration::from_millis(200);

// TODO(token): `sys.motion.coast.*` once Penpot has them (manual §1.1).
/// How quickly a fling's momentum dies out: it travels its speed times
/// this before it stops, as iOS's "normal" deceleration does.
const DECAY: f32 = 0.325;
/// How long a fling coasts for.
const COAST: Duration = Duration::from_millis(850);
/// Slower than this, in pixels a second, lifting the fingers just stops.
const MIN_FLING: f32 = 60.0;
/// Fingers resting this long before they lift leave no momentum behind.
const HELD: Duration = Duration::from_millis(80);
/// How far past an end the scroll pulls, as a share of the fingers' travel.
const RUBBER_BAND: f32 = 0.3;

/// How fast fingers are moving, smoothed so a flick is how a swipe ends.
#[derive(Clone, Copy, Debug)]
pub struct Tracker {
    // units a second, smoothed
    velocity: f32,
    last: Instant,
}

impl Tracker {
    pub fn new(now: Instant) -> Self {
        Self {
            velocity: 0.0,
            last: now,
        }
    }

    /// Fingers moved by `moved` at `now`.
    pub fn feed(&mut self, moved: f32, now: Instant) {
        if moved == 0.0 {
            return;
        }
        let dt = now.duration_since(self.last).as_secs_f32().max(0.004);
        // the latest movement counts most: a flick is how it ends
        self.velocity = self.velocity * 0.5 + (moved / dt) * 0.5;
        self.last = now;
    }

    /// How fast they were going when last heard of.
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// How fast they're going at `now`: nothing once they've rested.
    pub fn velocity_at(&self, now: Instant) -> f32 {
        if now.duration_since(self.last) >= HELD {
            0.0
        } else {
            self.velocity
        }
    }

    /// Whether they haven't been heard of for [`STALE`].
    pub fn stale(&self, now: Instant) -> bool {
        now.duration_since(self.last) >= STALE
    }
}

/// Which way something scrolls or pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// How far `scroll` goes along this axis, in lines (a wheel's notches).
    pub fn lines(self, scroll: &Scroll) -> f32 {
        match self {
            Self::Horizontal => scroll.x,
            Self::Vertical => scroll.y,
        }
    }

    /// The same in logical pixels, for following fingers one to one.
    pub fn pixels(self, scroll: &Scroll) -> f32 {
        let (x, y) = scroll.pixels();
        match self {
            Self::Horizontal => x,
            Self::Vertical => y,
        }
    }
}

/// How far fingers go before a gesture picks the axis it moves along.
const LOCK: f32 = 8.0;

/// One finger gesture moving along one axis: the first few pixels decide
/// which, and it keeps to it until the fingers lift. Two fingers on a
/// grid either scroll it or turn its page, never both at once.
#[derive(Clone, Copy, Debug, Default)]
pub struct AxisLock {
    axis: Option<Axis>,
    // movement before the axis is picked, carried over once it is
    pending: (f32, f32),
    last: Option<Instant>,
}

impl AxisLock {
    /// No gesture under way.
    pub const DEFAULT: Self = Self {
        axis: None,
        pending: (0.0, 0.0),
        last: None,
    };

    /// Fingers moved by (`x`, `y`) pixels at `now`: the axis the gesture
    /// keeps to and how far along it they moved, or none while it's still
    /// undecided. `sideways` false keeps every gesture vertical.
    pub fn feed(&mut self, x: f32, y: f32, sideways: bool, now: Instant) -> Option<(Axis, f32)> {
        if self
            .last
            .is_some_and(|last| now.duration_since(last) >= STALE)
        {
            *self = Self::default();
        }
        self.last = Some(now);
        if let Some(axis) = self.axis {
            return Some((axis, along(axis, x, y)));
        }
        self.pending.0 += x;
        self.pending.1 += y;
        let (x, y) = self.pending;
        if x.abs().max(y.abs()) < LOCK {
            return None;
        }
        let axis = if sideways && x.abs() > y.abs() {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        self.axis = Some(axis);
        Some((axis, along(axis, x, y)))
    }

    /// The fingers lifted: the next gesture picks again.
    pub fn release(&mut self) {
        *self = Self::default();
    }
}

fn along(axis: Axis, x: f32, y: f32) -> f32 {
    match axis {
        Axis::Horizontal => x,
        Axis::Vertical => y,
    }
}

/// `offset` kept in `0 ..= max`, pulled only `band` of the way past an
/// end (or stopped there without one).
pub fn banded(offset: f32, max: f32, band: Option<f32>) -> f32 {
    let edge = offset.clamp(0.0, max.max(0.0));
    match band {
        Some(band) => edge + (offset - edge) * band,
        None => edge,
    }
}

/// One axis of scrolling, in pixels from `0` to `max`: following fingers
/// one to one, coasting after a flick, and easing after a wheel or a
/// [`go`](Kinetic::go).
pub struct Kinetic {
    // where it is when no fingers are on it
    motion: Animation,
    // where `motion` is heading
    target: f32,
    // fingers on it: where they've put it (past the ends too), and how fast
    swipe: Option<(f32, Tracker)>,
    max: f32,
}

impl Default for Kinetic {
    fn default() -> Self {
        Self::new()
    }
}

impl Kinetic {
    /// At rest at the start.
    pub fn new() -> Self {
        Self {
            motion: eased(0.0),
            target: 0.0,
            swipe: None,
            max: 0.0,
        }
    }

    /// How far it is now: under the fingers (pulled past an end by the
    /// rubber band), or settling.
    pub fn value(&self) -> f32 {
        match &self.swipe {
            Some((offset, _)) => banded(*offset, self.max, Some(RUBBER_BAND)),
            None => self.motion.value(),
        }
    }

    /// Where it comes to rest.
    pub fn target(&self) -> f32 {
        match &self.swipe {
            Some((offset, _)) => offset.clamp(0.0, self.max),
            None => self.target,
        }
    }

    /// Whether fingers are on it.
    pub fn swiping(&self) -> bool {
        self.swipe.is_some()
    }

    /// How far it can go. A target out of range now eases back into it.
    pub fn set_max(&mut self, max: f32) {
        self.max = max.max(0.0);
        if self.swipe.is_none() && self.target > self.max {
            self.go(self.max);
        }
    }

    /// Eases to `to`, kept in range: a wheel, or bringing something into
    /// view.
    pub fn go(&mut self, to: f32) {
        let from = self.value();
        self.swipe = None;
        self.target = to.clamp(0.0, self.max);
        self.motion = eased(from);
        self.motion.to(self.target);
    }

    /// Puts it at `to` at once.
    pub fn jump(&mut self, to: f32) {
        self.swipe = None;
        self.target = to.clamp(0.0, self.max);
        self.motion = eased(self.target);
    }

    /// `scroll` along `axis`: fingers are followed, a wheel eases on by
    /// `wheel_step` a notch.
    pub fn scroll(&mut self, scroll: &Scroll, axis: Axis, wheel_step: f32) {
        match scroll.source {
            ScrollSource::Finger | ScrollSource::Continuous => {
                self.follow(axis.pixels(scroll), scroll.stop, Instant::now());
            }
            ScrollSource::Wheel | ScrollSource::Tilt => {
                let to = self.target() + axis.lines(scroll) * wheel_step;
                self.go(to);
            }
        }
    }

    /// Fingers moved by `moved` pixels at `now`, lifting if `stop`.
    pub fn follow(&mut self, moved: f32, stop: bool, now: Instant) {
        let start = self.value();
        let (offset, tracker) = self.swipe.get_or_insert_with(|| (start, Tracker::new(now)));
        *offset += moved;
        tracker.feed(moved, now);
        if stop {
            self.release(now);
        }
    }

    /// A swipe whose end never came ends now. Call it once a frame.
    pub fn settle_stale(&mut self) {
        let now = Instant::now();
        if self
            .swipe
            .as_ref()
            .is_some_and(|(_, tracker)| tracker.stale(now))
        {
            self.release(now);
        }
    }

    /// The fingers lifted: it coasts on as far as their speed carries it,
    /// or springs back from past an end.
    fn release(&mut self, now: Instant) {
        let Some((offset, tracker)) = self.swipe.take() else {
            return;
        };
        let from = banded(offset, self.max, Some(RUBBER_BAND));
        let velocity = tracker.velocity_at(now);

        if from < 0.0 || from > self.max {
            // past an end: back to it on a spring
            self.target = from.clamp(0.0, self.max);
            self.motion = motion::spring(from);
            self.motion.to(self.target);
        } else if velocity.abs() >= MIN_FLING {
            self.target = projected(from, velocity).clamp(0.0, self.max);
            self.motion = Animation::new(from).duration(COAST).easing(Easing::Out);
            self.motion.to(self.target);
        } else {
            self.target = from;
            self.motion = eased(from);
        }
    }
}

/// Where a fling from `from` at `velocity` (pixels a second) comes to rest.
pub fn projected(from: f32, velocity: f32) -> f32 {
    from + velocity * DECAY
}

/// The ease a wheel and a `go` take.
fn eased(value: f32) -> Animation {
    motion::fade(value, motion::PRESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinetic(max: f32) -> Kinetic {
        let mut kinetic = Kinetic::new();
        kinetic.set_max(max);
        kinetic
    }

    #[test]
    fn the_fingers_are_followed_one_to_one() {
        let mut scroll = kinetic(500.0);
        let now = Instant::now();
        scroll.follow(40.0, false, now);
        scroll.follow(25.0, false, now + Duration::from_millis(10));
        assert!(scroll.swiping());
        assert_eq!(scroll.value(), 65.0);
    }

    #[test]
    fn a_flick_coasts_on_as_far_as_its_speed_carries_it() {
        let mut scroll = kinetic(5000.0);
        let now = Instant::now();
        scroll.follow(10.0, false, now);
        scroll.follow(10.0, false, now + Duration::from_millis(10));
        scroll.follow(0.0, true, now + Duration::from_millis(12));
        assert!(!scroll.swiping());
        assert!(scroll.target() > 100.0, "went on past {}", scroll.target());
    }

    #[test]
    fn a_fling_stops_at_the_end() {
        let mut scroll = kinetic(100.0);
        let now = Instant::now();
        scroll.follow(30.0, false, now);
        scroll.follow(30.0, false, now + Duration::from_millis(5));
        scroll.follow(0.0, true, now + Duration::from_millis(6));
        assert_eq!(scroll.target(), 100.0);
    }

    #[test]
    fn fingers_that_rested_before_lifting_leave_no_momentum() {
        let mut scroll = kinetic(5000.0);
        let now = Instant::now();
        scroll.follow(30.0, false, now);
        scroll.follow(0.0, true, now + HELD + Duration::from_millis(5));
        assert_eq!(scroll.target(), 30.0);
    }

    #[test]
    fn past_an_end_the_rubber_band_pulls_and_springs_back() {
        let mut scroll = kinetic(100.0);
        let now = Instant::now();
        scroll.follow(-50.0, false, now);
        assert!((scroll.value() + 50.0 * RUBBER_BAND).abs() < 0.001);
        scroll.follow(0.0, true, now + Duration::from_millis(500));
        assert_eq!(scroll.target(), 0.0);
    }

    #[test]
    fn a_wheel_steps_and_stays_in_range() {
        let mut scroll = kinetic(100.0);
        let wheel = |y| Scroll {
            y,
            ..Scroll::default()
        };
        scroll.scroll(&wheel(1.0), Axis::Vertical, 60.0);
        assert_eq!(scroll.target(), 60.0);
        scroll.scroll(&wheel(1.0), Axis::Vertical, 60.0);
        assert_eq!(scroll.target(), 100.0);
        scroll.scroll(&wheel(-3.0), Axis::Vertical, 60.0);
        assert_eq!(scroll.target(), 0.0);
    }

    #[test]
    fn a_shorter_range_brings_it_back_into_it() {
        let mut scroll = kinetic(300.0);
        scroll.jump(250.0);
        scroll.set_max(100.0);
        assert_eq!(scroll.target(), 100.0);
    }

    #[test]
    fn a_gesture_keeps_to_the_axis_it_set_out_on() {
        let now = Instant::now();
        let mut lock = AxisLock::default();
        assert_eq!(lock.feed(5.0, 1.0, true, now), None, "still deciding");
        assert_eq!(
            lock.feed(5.0, 1.0, true, now),
            Some((Axis::Horizontal, 10.0)),
            "what it held back is carried over"
        );
        assert_eq!(
            lock.feed(0.0, 30.0, true, now),
            Some((Axis::Horizontal, 0.0))
        );
        lock.release();
        assert_eq!(
            lock.feed(1.0, 12.0, true, now),
            Some((Axis::Vertical, 12.0))
        );
    }

    #[test]
    fn without_sideways_every_gesture_is_vertical() {
        let mut lock = AxisLock::default();
        let picked = lock.feed(20.0, 1.0, false, Instant::now());
        assert_eq!(picked, Some((Axis::Vertical, 1.0)));
    }

    #[test]
    fn a_gesture_gone_quiet_starts_over() {
        let now = Instant::now();
        let mut lock = AxisLock::default();
        lock.feed(20.0, 0.0, true, now);
        let later = now + STALE + Duration::from_millis(1);
        assert_eq!(lock.feed(0.0, 3.0, true, later), None);
    }

    #[test]
    fn velocity_is_smoothed_towards_the_latest_movement() {
        let now = Instant::now();
        let mut tracker = Tracker::new(now);
        tracker.feed(10.0, now + Duration::from_millis(10));
        assert!((tracker.velocity() - 500.0).abs() < 1.0);
        tracker.feed(20.0, now + Duration::from_millis(20));
        assert!((tracker.velocity() - 1250.0).abs() < 1.0);
    }
}
