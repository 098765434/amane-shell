//! How well a result matches, and how much its use lifts it: a fuzzy match
//! on its name, then how often and how lately it was picked (frecency), and
//! what was picked for the same few letters before. Pure, so the tests run
//! it.

use amane::serde::{Deserialize, Serialize};

/// How long a use takes to count half as much.
const HALF_LIFE: f64 = 7.0 * 24.0 * 3600.0;
/// How much a result's frecency and what was learned for the query lift it.
const FRECENCY_WEIGHT: f32 = 0.25;
const LEARNED_WEIGHT: f32 = 0.4;

/// How well `needle` (already lowercased) matches `hay`, 0 … 1, or `None`
/// when it doesn't: the whole name, then its start, a word's start, the
/// words' initials ("vsc" for Visual Studio Code), anywhere in it, and last
/// its letters in order with others between.
pub(super) fn fuzzy(needle: &str, hay: &str) -> Option<f32> {
    if needle.is_empty() {
        return Some(0.0);
    }
    let hay = hay.to_lowercase();
    // a longer name matched by the same letters is a little less likely
    let share = needle.chars().count() as f32 / hay.chars().count().max(1) as f32;
    if hay == needle {
        return Some(1.0);
    }
    if hay.starts_with(needle) {
        return Some(0.85 + 0.1 * share);
    }
    let words: Vec<&str> = hay.split(is_separator).filter(|w| !w.is_empty()).collect();
    if words.iter().any(|word| word.starts_with(needle)) {
        return Some(0.75 + 0.1 * share);
    }
    let initials: String = words
        .iter()
        .filter_map(|word| word.chars().next())
        .collect();
    if needle.chars().count() >= 2 && initials.starts_with(needle) {
        return Some(0.7);
    }
    if hay.contains(needle) {
        return Some(0.55 + 0.1 * share);
    }
    subsequence(needle, &hay)
}

fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '_' | '.' | '/' | ':' | '(' | ')')
}

/// `needle`'s letters in order through `hay`, scored by how many come
/// together; a single letter anywhere isn't a match.
fn subsequence(needle: &str, hay: &str) -> Option<f32> {
    let wanted: Vec<char> = needle.chars().filter(|c| !c.is_whitespace()).collect();
    if wanted.len() < 2 {
        return None;
    }
    let mut at = 0;
    let mut runs = 0;
    let mut last: Option<usize> = None;
    for (index, c) in hay.chars().enumerate() {
        if at < wanted.len() && c == wanted[at] {
            if last.is_some_and(|last| last + 1 == index) {
                runs += 1;
            }
            last = Some(index);
            at += 1;
        }
    }
    (at == wanted.len()).then(|| 0.3 + 0.2 * runs as f32 / (wanted.len() - 1) as f32)
}

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

    fn score(needle: &str, hay: &str) -> f32 {
        fuzzy(needle, hay).unwrap_or(-1.0)
    }

    #[test]
    fn a_closer_match_scores_higher() {
        assert_eq!(score("firefox", "Firefox"), 1.0);
        assert!(score("fire", "Firefox") > score("fox", "Firefox"));
        assert!(score("code", "Visual Studio Code") > score("od", "Visual Studio Code"));
        assert!(score("vsc", "Visual Studio Code") > score("vsd", "Visual Studio Code"));
    }

    #[test]
    fn letters_in_order_still_match() {
        assert!(fuzzy("frx", "firefox").is_some());
        assert!(fuzzy("xf", "firefox").is_none());
        // one letter has to be in the name itself
        assert!(fuzzy("z", "firefox").is_none());
        assert!(fuzzy("f", "firefox").is_some());
    }

    #[test]
    fn letters_together_beat_letters_apart() {
        assert!(score("tex", "kate editor text") > score("tdr", "kate editor text"));
    }

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
