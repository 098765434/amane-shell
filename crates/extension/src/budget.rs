//! What a program may send, and how often: the numbers the bar holds every
//! program to, so none can wake it every frame or fill its memory. A payload
//! over a limit is refused or clamped where it's read; updates over a rate are
//! held and coalesced into the latest by the bar.
//!
//! See `docs/developer/extensions-plan.md` §2 for where the numbers come from.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// The most JSON one call may carry, in bytes.
pub const MAX_PAYLOAD: usize = 64 * 1024;

/// The shortest time between two updates the bar applies to one item or activity.
pub const OBJECT_INTERVAL: Duration = Duration::from_millis(100);

/// The calls a program may make in a second, across everything it put up.
pub const OWNER_RATE: u32 = 20;

/// The updates the bar holds for one program at once; one more is refused as busy.
pub const QUEUE_DEPTH: usize = 10;

/// The buttons or menu entries a tile page or a widget may offer.
pub const MAX_ACTIONS: usize = 5;

/// The buttons an activity's expanded view shows.
pub const MAX_ACTIVITY_BUTTONS: usize = 3;

/// How long an activity lasts before the bar ends it.
pub const MAX_ACTIVITY_AGE: Duration = Duration::from_secs(8 * 60 * 60);

/// The entries one level of an item's menu may have.
pub const MAX_MENU_ENTRIES: usize = 64;

/// How deep submenus may nest; deeper ones are cut off.
pub const MAX_MENU_DEPTH: usize = 4;

/// The characters a title, label, badge or line of text may have.
pub const MAX_TEXT: usize = 256;

/// The characters a tooltip may have.
pub const MAX_TOOLTIP: usize = 1024;

/// What the bar does with one update, by [`Limiter::update`].
#[derive(Debug, Clone, PartialEq)]
pub enum Offer<T> {
    /// Apply it now.
    Apply(T),

    /// Held, replacing any update held for the same thing, until this moment; then
    /// [`Limiter::due`] hands it back.
    Held(Instant),

    /// Refused: [`QUEUE_DEPTH`] other things already have an update held.
    Busy,
}

/// One program's budget, kept by the bar: [`OWNER_RATE`] calls a second across everything
/// it put up, one update per [`OBJECT_INTERVAL`] to each thing, and updates over either
/// held and coalesced into the latest. Time is passed in, so it is tested without waiting.
#[derive(Debug)]
pub struct Limiter<K, T> {
    tokens: f64,
    refilled: Instant,
    applied: HashMap<K, Instant>,
    held: Vec<(K, T)>,
}

impl<K: Clone + Eq + Hash, T> Limiter<K, T> {
    /// A full budget at `now`.
    pub fn new(now: Instant) -> Self {
        Self {
            tokens: f64::from(OWNER_RATE),
            refilled: now,
            applied: HashMap::new(),
            held: Vec::new(),
        }
    }

    fn refill(&mut self, now: Instant) {
        let since = now.saturating_duration_since(self.refilled).as_secs_f64();
        self.tokens = (self.tokens + since * f64::from(OWNER_RATE)).min(f64::from(OWNER_RATE));
        self.refilled = now;
    }

    /// When `object` may next be updated: its interval passed and a call left in the second.
    fn free_at(&self, object: &K, now: Instant) -> Instant {
        let interval = self
            .applied
            .get(object)
            .map_or(now, |at| (*at + OBJECT_INTERVAL).max(now));
        let token = if self.tokens >= 1.0 {
            now
        } else {
            now + Duration::from_secs_f64((1.0 - self.tokens) / f64::from(OWNER_RATE))
        };
        interval.max(token)
    }

    fn spend(&mut self, object: K, now: Instant) {
        self.tokens -= 1.0;
        self.applied.insert(object, now);
    }

    /// An update to `object`: applied now if the budget allows, else held. One already held
    /// for it is replaced, `merge` folding the old one into the new (an alert kept, say).
    pub fn update(
        &mut self,
        object: K,
        payload: T,
        now: Instant,
        merge: impl FnOnce(T, &mut T),
    ) -> Offer<T> {
        self.refill(now);

        if let Some(index) = self.held.iter().position(|(held, _)| *held == object) {
            let mut payload = payload;
            let (_, old) = self.held.remove(index);
            merge(old, &mut payload);
            self.held.insert(index, (object.clone(), payload));
            return Offer::Held(self.free_at(&object, now));
        }

        if self.free_at(&object, now) <= now {
            self.spend(object, now);
            return Offer::Apply(payload);
        }

        if self.held.len() >= QUEUE_DEPTH {
            return Offer::Busy;
        }

        let until = self.free_at(&object, now);
        self.held.push((object, payload));
        Offer::Held(until)
    }

    /// A call that can't wait or be merged (putting something up, taking it down): it spends
    /// a call of the second, and is refused when none is left.
    pub fn call(&mut self, now: Instant) -> bool {
        self.refill(now);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }

    /// Drops what is held for `object`, as it went.
    pub fn forget(&mut self, object: &K) {
        self.held.retain(|(held, _)| held != object);
        self.applied.remove(object);
    }

    /// The held updates whose time has come, in the order they were first held; they count
    /// as applied.
    pub fn due(&mut self, now: Instant) -> Vec<(K, T)> {
        self.refill(now);
        let mut due = Vec::new();
        let mut index = 0;
        while index < self.held.len() {
            if self.free_at(&self.held[index].0, now) <= now {
                let (object, payload) = self.held.remove(index);
                self.spend(object.clone(), now);
                due.push((object, payload));
            } else {
                index += 1;
            }
        }
        due
    }

    /// When the next held update is due, if any is held.
    pub fn next(&self, now: Instant) -> Option<Instant> {
        self.held
            .iter()
            .map(|(object, _)| self.free_at(object, now))
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keep_new(_old: u32, _new: &mut u32) {}

    #[test]
    fn an_update_inside_the_interval_is_held_and_coalesced() {
        let start = Instant::now();
        let mut limiter = Limiter::new(start);

        assert_eq!(limiter.update("a", 1, start, keep_new), Offer::Apply(1));

        let soon = start + Duration::from_millis(10);
        let mut until = None;
        for n in 2..=11 {
            match limiter.update("a", n, soon, keep_new) {
                Offer::Held(at) => until = Some(at),
                other => panic!("expected a held update, got {other:?}"),
            }
        }
        assert_eq!(until, Some(start + OBJECT_INTERVAL));
        assert!(limiter.due(soon).is_empty());
        assert_eq!(limiter.next(soon), Some(start + OBJECT_INTERVAL));
        assert_eq!(limiter.due(start + OBJECT_INTERVAL), [("a", 11)]);
        assert_eq!(limiter.next(start + OBJECT_INTERVAL), None);
    }

    #[test]
    fn merge_sees_the_update_it_replaces() {
        let start = Instant::now();
        let mut limiter = Limiter::new(start);
        let or = |old: bool, new: &mut bool| *new |= old;

        assert_eq!(limiter.update("a", false, start, or), Offer::Apply(false));
        limiter.update("a", true, start, or);
        limiter.update("a", false, start, or);
        assert_eq!(limiter.due(start + OBJECT_INTERVAL), [("a", true)]);
    }

    #[test]
    fn too_many_things_held_at_once_is_busy() {
        let start = Instant::now();
        let mut limiter = Limiter::new(start);

        for _ in 0..OWNER_RATE {
            assert!(limiter.call(start));
        }
        for n in 0..QUEUE_DEPTH as u32 {
            assert!(matches!(
                limiter.update(100 + n, n, start, keep_new),
                Offer::Held(_)
            ));
        }
        assert_eq!(limiter.update(999, 0, start, keep_new), Offer::Busy);
        assert!(!limiter.call(start));

        // one call comes back every 1/OWNER_RATE of a second
        let later = start + Duration::from_secs(1);
        assert_eq!(limiter.due(later).len(), QUEUE_DEPTH);
        assert!(limiter.call(later));
    }

    #[test]
    fn a_full_second_refills_the_budget() {
        let start = Instant::now();
        let mut limiter: Limiter<u32, u32> = Limiter::new(start);

        for _ in 0..OWNER_RATE {
            assert!(limiter.call(start));
        }
        assert!(!limiter.call(start));
        let later = start + Duration::from_secs(1);
        for _ in 0..OWNER_RATE {
            assert!(limiter.call(later));
        }
    }

    #[test]
    fn forgetting_drops_what_is_held() {
        let start = Instant::now();
        let mut limiter = Limiter::new(start);

        limiter.update("a", 1, start, keep_new);
        limiter.update("a", 2, start, keep_new);
        limiter.forget(&"a");
        assert_eq!(limiter.next(start), None);
        assert_eq!(limiter.update("a", 3, start, keep_new), Offer::Apply(3));
    }
}
