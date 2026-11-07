//! How well a result matches, and how much its use lifts it: a fuzzy match
//! on its name (the kit's [`amane_kit::fuzzy`]), then how often and how lately it was picked (frecency), and
//! what was picked for the same few letters before. Pure, so the tests run
//! it.

use amane::serde::{Deserialize, Serialize};

/// How long a use takes to count half as much.
const HALF_LIFE: f64 = 7.0 * 24.0 * 3600.0;
/// How much a result's frecency and what was learned for the query lift it.
const FRECENCY_WEIGHT: f32 = 0.25;
const LEARNED_WEIGHT: f32 = 0.4;

pub(super) use amane_kit::fuzzy::fuzzy;

/// How much a result has been used: a score that halves every week, and
/// when it was last counted, in seconds since the epoch.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde")]
pub(super) struct Use {
    pub(super) score: f64,
    pub(super) at: u64,
}

impl Use {
    /// What the score has decayed to by `now`.
    pub(super) fn at(self, now: u64) -> f64 {
        let age = now.saturating_sub(self.at) as f64;
        self.score * 0.5f64.powf(age / HALF_LIFE)
    }

    /// One more use at `now`.
    pub(super) fn bumped(self, now: u64) -> Self {
        Self {
            score: self.at(now) + 1.0,
            at: now,
        }
    }
}

/// A result's place: how well it matched, lifted by its `frecency` and by
/// `learned`, the share of the times this query picked it.
pub(super) fn rank(matched: f32, frecency: f64, learned: f32) -> f32 {
    matched * (1.0 + FRECENCY_WEIGHT * (frecency as f32).ln_1p()) + LEARNED_WEIGHT * learned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_use_halves_in_a_week() {
        let used = Use { score: 2.0, at: 0 };
        assert!((used.at(HALF_LIFE as u64) - 1.0).abs() < 1e-9);
        let again = used.bumped(HALF_LIFE as u64);
        assert!((again.score - 2.0).abs() < 1e-9);
        assert_eq!(again.at, HALF_LIFE as u64);
    }

    #[test]
    fn use_lifts_a_weaker_match() {
        let fresh = rank(0.75, 0.0, 0.0);
        let used = rank(0.7, 10.0, 0.0);
        assert!(used > fresh);
        assert!(rank(0.5, 0.0, 1.0) > rank(0.8, 0.0, 0.0));
    }
}
