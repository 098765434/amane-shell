//! Which conversations were read here. KDE Connect can't mark a message
//! read on the phone, so the phone's own flag stays "unread" until it's
//! opened there; opening a conversation here keeps the newest message it
//! showed, and only a newer one is unread again. Kept in
//! `~/.cache/amane-shell/messages/read.json`, as thread id to the date of
//! the newest message seen.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use amane_phone::SmsThread;

thread_local! {
    static SEEN: RefCell<Option<HashMap<i64, i64>>> = const { RefCell::new(None) };
}

fn path() -> Option<PathBuf> {
    let cache = amane_kit::paths::cache();
    Some(cache.join("amane-shell/messages/read.json"))
}

fn load() -> HashMap<i64, i64> {
    path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<HashMap<String, i64>>(&text).ok())
        .map(|seen| {
            seen.into_iter()
                .filter_map(|(thread, date)| Some((thread.parse().ok()?, date)))
                .collect()
        })
        .unwrap_or_default()
}

fn with<T>(f: impl FnOnce(&mut HashMap<i64, i64>) -> T) -> T {
    SEEN.with_borrow_mut(|seen| f(seen.get_or_insert_with(load)))
}

/// Whether `thread` has a message neither the phone nor this computer has
/// seen read.
pub fn unread(thread: &SmsThread) -> bool {
    let seen = with(|seen| seen.get(&thread.id()).copied());
    unread_since(thread.unread(), thread.latest().date(), seen)
}

/// Unread on the phone, and newer than what was seen here.
fn unread_since(on_phone: bool, latest: i64, seen: Option<i64>) -> bool {
    on_phone && seen.is_none_or(|date| latest > date)
}

/// `thread` was shown here up to its newest message.
pub fn mark(thread: &SmsThread) {
    let date = thread.latest().date();
    let changed = with(|seen| {
        if seen.get(&thread.id()).is_some_and(|old| *old >= date) {
            return false;
        }
        seen.insert(thread.id(), date);
        true
    });
    if changed && thread.unread() {
        save();
    }
}

fn save() {
    let Some(path) = path() else {
        return;
    };
    let text = with(|seen| {
        let named: HashMap<String, i64> = seen
            .iter()
            .map(|(thread, date)| (thread.to_string(), *date))
            .collect();
        serde_json::to_string(&named).unwrap_or_default()
    });
    if let Some(folder) = path.parent() {
        // the write below says so if the folder couldn't be made
        let _ = std::fs::create_dir_all(folder);
    }
    if let Err(error) = std::fs::write(path, text) {
        log::warn!("couldn't keep what was read: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seen_here_is_read_until_a_newer_message() {
        assert!(unread_since(true, 2_000, None));
        assert!(!unread_since(true, 2_000, Some(2_000)));
        assert!(unread_since(true, 3_000, Some(2_000)));
        assert!(!unread_since(false, 3_000, None));
    }
}
