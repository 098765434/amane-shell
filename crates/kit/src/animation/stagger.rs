//! A clock for motion that comes in parts, one after another: a surface's
//! pieces arriving in turn as it opens, or a list's rows rising in one by
//! one. Each part asks how far along it is, given how long after the start
//! it sets off and how long it takes; the clock asks for frames while any
//! part it was asked about is still on its way.
//!
//! Where motion is reduced, every part fades in at once, over a fade.
//!
//! It's `Copy`, so a view can keep it in a `Cell` and hand it to whatever
//! draws the parts.

use std::time::{Duration, Instant};

use amane::Easing;

use crate::motion;
use crate::theme;

/// When a run of staggered motion started; nothing started is all arrived.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stagger {
    started: Option<Instant>,
}

impl Stagger {
    /// Not started: every part has arrived.
    pub const fn new() -> Self {
        Self { started: None }
    }

    /// Starts now.
    pub fn start(&mut self) {
        self.started = Some(Instant::now());
    }

    /// Starts once `delay` has passed: until then every part is at 0.
    pub fn start_in(&mut self, delay: Duration) {
        self.started = Some(Instant::now() + delay);
    }

    /// Back to not started: every part has arrived.
    pub fn reset(&mut self) {
        self.started = None;
    }

    /// Whether it has been started, arrived or not.
    pub fn started(&self) -> bool {
        self.started.is_some()
    }

    /// How long since it started, if it has (zero while it's yet to).
    pub fn elapsed(&self) -> Option<Duration> {
        self.started
            .map(|started| Instant::now().saturating_duration_since(started))
    }

    /// Whether it started less than `length` ago, while motion isn't
    /// reduced: for something held back at the start of the run.
    pub fn within(&self, length: Duration) -> bool {
        !theme::reduced_motion() && self.elapsed().is_some_and(|elapsed| elapsed < length)
    }

    /// How far a part is, 0 … 1 (past 1 on a spring), that sets off
    /// `delay` after the start and takes `length` on `easing`. Asks for
    /// another frame while it hasn't arrived.
    pub fn progress(&self, delay: Duration, length: Duration, easing: Easing) -> f32 {
        let Some(started) = self.started else {
            return 1.0;
        };
        let (delay, length, easing) = if theme::reduced_motion() {
            (Duration::ZERO, motion::duration::fade(), Easing::Out)
        } else {
            (delay, length, easing)
        };
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(started);
        let before = now < started;
        let amount = if before {
            0.0
        } else {
            at(elapsed, delay, length, easing)
        };
        if before || elapsed < delay + length {
            amane::request_frame();
        }
        amount
    }

    /// As [`progress`](Self::progress) for part `index` of a run whose parts
    /// set off `step` apart, after `delay`.
    pub fn nth(
        &self,
        index: usize,
        delay: Duration,
        step: Duration,
        length: Duration,
        easing: Easing,
    ) -> f32 {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        self.progress(delay + step.saturating_mul(index), length, easing)
    }
}

/// How far a part that sets off `delay` in and takes `length` on `easing`
/// is, `elapsed` into the run.
fn at(elapsed: Duration, delay: Duration, length: Duration, easing: Easing) -> f32 {
    let Some(into) = elapsed.checked_sub(delay) else {
        return 0.0;
    };
    if length.is_zero() || into >= length {
        return 1.0;
    }
    easing.apply(into.as_secs_f32() / length.as_secs_f32())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn not_started_is_arrived() {
        let clock = Stagger::new();
        assert_eq!(clock.progress(MS(100), MS(100), Easing::Out), 1.0);
        assert!(!clock.within(MS(100)));
    }

    #[test]
    fn a_part_waits_for_its_delay() {
        assert_eq!(at(MS(50), MS(100), MS(200), Easing::Linear), 0.0);
        assert_eq!(at(MS(100), MS(100), MS(200), Easing::Linear), 0.0);
        assert!((at(MS(200), MS(100), MS(200), Easing::Linear) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn a_part_lands_on_one() {
        assert_eq!(at(MS(300), MS(100), MS(200), Easing::Out), 1.0);
        assert_eq!(
            at(MS(900), MS(0), MS(200), Easing::Spring { bounce: 0.3 }),
            1.0
        );
        assert_eq!(at(MS(10), MS(0), Duration::ZERO, Easing::Out), 1.0);
    }

    #[test]
    fn later_parts_trail_earlier_ones() {
        let first = at(MS(150), MS(0), MS(200), Easing::Out);
        let third = at(MS(150), MS(60), MS(200), Easing::Out);
        assert!(first > third);
    }

    #[test]
    fn started_in_the_future_is_at_zero() {
        let mut clock = Stagger::new();
        clock.start_in(MS(10_000));
        assert!(clock.started());
        assert_eq!(clock.elapsed(), Some(Duration::ZERO));
    }
}
