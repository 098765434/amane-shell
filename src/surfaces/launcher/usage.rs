//! What the launcher remembers of its use, in
//! `$XDG_STATE_HOME/amane-shell/launcher-usage.json`: how much each result
//! was used, what was picked for the first few letters of a query, and the
//! queries run, newest first, for Up to bring back.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use amane::serde::{Deserialize, Serialize};

use crate::services::{save_state, state_file};

use super::rank::Use;

const FILE: &str = "launcher-usage.json";
/// How much is kept: results, query prefixes, and queries run.
const USES: usize = 500;
const PREFIXES: usize = 2000;
const HISTORY: usize = 50;
/// How many letters of a query what's picked is learned for.
const PREFIX_CHARS: usize = 8;

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub(super) struct Usage {
    uses: HashMap<String, Use>,
    // a query's first letters, lowercased → what was picked for them, and how often
    picks: HashMap<String, HashMap<String, u32>>,
    history: VecDeque<String>,
}

impl Usage {
    /// How much `key` has been used, as of `now`.
    pub(super) fn frecency(&self, key: &str, now: u64) -> f64 {
        self.uses.get(key).map_or(0.0, |used| used.at(now))
    }

    /// The share of the picks for `query`'s first letters that were `key`.
    pub(super) fn learned(&self, query: &str, key: &str) -> f32 {
        let Some(picked) = self.picks.get(&prefix(query)) else {
            return 0.0;
        };
        let total: u32 = picked.values().sum();
        let mine = picked.get(key).copied().unwrap_or(0);
        if total == 0 {
            0.0
        } else {
            mine as f32 / total as f32
        }
    }

    /// The `count` keys starting with `kind` used most, most first.
    pub(super) fn top(&self, kind: &str, count: usize, now: u64) -> Vec<String> {
        let mut keys: Vec<(&String, f64)> = self
            .uses
            .iter()
            .filter(|(key, _)| key.starts_with(kind))
            .map(|(key, used)| (key, used.at(now)))
            .collect();
        keys.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        keys.into_iter()
            .take(count)
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// The queries run, newest first.
    pub(super) fn history(&self) -> &VecDeque<String> {
        &self.history
    }

    /// `key` was picked for `query` at `now`.
    pub(super) fn record(&mut self, key: &str, query: &str, now: u64) {
        let used = self.uses.entry(key.to_string()).or_default();
        *used = used.bumped(now);
        let query = query.trim();
        if !query.is_empty() {
            *self
                .picks
                .entry(prefix(query))
                .or_default()
                .entry(key.to_string())
                .or_default() += 1;
            self.history.retain(|old| old != query);
            self.history.push_front(query.to_string());
        }
        self.prune(now);
    }

    /// Keeps what's kept within its limits: the least used results and
    /// the oldest queries go first.
    fn prune(&mut self, now: u64) {
        if self.uses.len() > USES {
            let mut scores: Vec<(String, f64)> = self
                .uses
                .iter()
                .map(|(key, used)| (key.clone(), used.at(now)))
                .collect();
            scores.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (key, _) in scores.into_iter().skip(USES) {
                self.uses.remove(&key);
            }
        }
        if self.picks.len() > PREFIXES {
            let mut prefixes: Vec<(String, u32)> = self
                .picks
                .iter()
                .map(|(prefix, picked)| (prefix.clone(), picked.values().sum()))
                .collect();
            prefixes.sort_by_key(|prefix| std::cmp::Reverse(prefix.1));
            for (prefix, _) in prefixes.into_iter().skip(PREFIXES) {
                self.picks.remove(&prefix);
            }
        }
        self.history.truncate(HISTORY);
    }
}

/// The part of `query` what's picked is learned for.
fn prefix(query: &str) -> String {
    query
        .trim()
        .to_lowercase()
        .chars()
        .take(PREFIX_CHARS)
        .collect()
}

/// Seconds since the epoch.
pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

thread_local! {
    static USAGE: RefCell<Option<Usage>> = const { RefCell::new(None) };
}

/// Reads the launcher's usage, from its file the first time.
pub(super) fn read<R>(f: impl FnOnce(&Usage) -> R) -> R {
    USAGE.with_borrow_mut(|usage| f(usage.get_or_insert_with(load)))
}

/// Counts `key` picked for `query`, and saves it.
pub(super) fn record(key: &str, query: &str) {
    let text = USAGE.with_borrow_mut(|usage| {
        let usage = usage.get_or_insert_with(load);
        usage.record(key, query, now());
        serde_json::to_string(usage).ok()
    });
    if let Some(text) = text {
        save_state(FILE, &text);
    }
}

fn load() -> Usage {
    std::fs::read_to_string(state_file(FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pick_is_counted_and_learned() {
        let mut usage = Usage::default();
        usage.record("app:firefox.desktop", "fi", 100);
        usage.record("app:firefox.desktop", "fir", 200);
        usage.record("app:files.desktop", "fi", 300);
        assert!(usage.frecency("app:firefox.desktop", 300) > 1.9);
        assert_eq!(usage.learned("FI", "app:firefox.desktop"), 0.5);
        assert_eq!(usage.learned("zz", "app:firefox.desktop"), 0.0);
        assert_eq!(
            usage.top("app:", 1, 300),
            vec![String::from("app:firefox.desktop")]
        );
    }

    #[test]
    fn history_is_newest_first_without_repeats() {
        let mut usage = Usage::default();
        usage.record("a", "one", 1);
        usage.record("a", "two", 2);
        usage.record("a", "one", 3);
        usage.record("a", "  ", 4);
        assert_eq!(
            usage.history().iter().collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }

    #[test]
    fn what_is_kept_stays_within_its_limits() {
        let mut usage = Usage::default();
        for n in 0..(USES + 10) {
            usage.record(&format!("app:{n}"), &format!("q{n}"), n as u64);
        }
        assert_eq!(usage.uses.len(), USES);
        assert_eq!(usage.history.len(), HISTORY);
        // the newest stay
        assert!(usage.uses.contains_key(&format!("app:{}", USES + 9)));
    }
}
