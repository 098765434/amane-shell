//! A string that lives for the program's run, for the APIs that keep
//! `&'static str` ids.
//!
//! Each distinct string is kept once, so naming the same id every frame
//! costs one lookup, not a leak a frame.

use std::collections::HashSet;
use std::sync::Mutex;

use crate::sync::Locked as _;

static KEPT: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

/// `text`, kept for the program's run: the same `&'static str` each time
/// it's asked for.
pub fn intern(text: &str) -> &'static str {
    let mut kept = KEPT.locked();
    let kept = kept.get_or_insert_with(HashSet::new);
    if let Some(&found) = kept.get(text) {
        return found;
    }
    let leaked: &'static str = Box::leak(text.to_owned().into_boxed_str());
    kept.insert(leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_is_kept_once() {
        let one = intern(&format!("id.{}", 7));
        let two = intern("id.7");
        assert_eq!(one, "id.7");
        assert!(std::ptr::eq(one, two));
    }
}
