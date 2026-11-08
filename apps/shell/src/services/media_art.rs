//! Album art for what's playing: the file a player names, or a copy of the
//! art it links to on the web, fetched once into
//! `$XDG_CACHE_HOME/emaki/art/`. Quick Settings' Now Playing card and
//! the bar's Now Playing activity both show it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

emaki_kit::marker! {
    /// Art fetched from the web landed in the cache: what showed none draws
    /// again.
    Arrived
}

thread_local! {
    // the web art being fetched, so each is asked for once
    static FETCHING: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    // the web art already in the cache, by URL, so a frame doesn't look on disk
    static FOUND: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// The file holding the art at `url`: the file itself for a `file://`
/// URL, else a copy in the cache, fetched in the background the first
/// time. Called while drawing: what asked draws again when it lands.
pub fn path(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    if let Some(path) = url.strip_prefix("file://") {
        return Some(emaki_kit::percent::decode(path));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    // asked on every frame of the card and the bar's Now Playing: a file found once is kept
    Arrived::watch();
    if let Some(found) = FOUND.with_borrow(|found| found.get(url).cloned()) {
        return Some(found);
    }
    let path = cached(url);
    if path.exists() {
        let found = path.to_string_lossy().into_owned();
        FOUND.with_borrow_mut(|known| {
            // a few players' art at most; a long session's list is let go now and then
            if known.len() > 64 {
                known.clear();
            }
            known.insert(url.to_string(), found.clone());
        });
        return Some(found);
    }
    let fresh = FETCHING.with_borrow_mut(|fetching| fetching.insert(url.to_string()));
    if fresh {
        let url = url.to_string();
        std::thread::spawn(move || {
            if let Some(dir) = path.parent() {
                // a folder that can't be made fails the fetch below, which is shown as no art
                let _ = std::fs::create_dir_all(dir);
            }
            let part = path.with_extension("part");
            let fetched = std::process::Command::new("curl")
                .args(["-fsSL", "--max-time", "15", "-o"])
                .arg(&part)
                .arg(&url)
                .status()
                .is_ok_and(|status| status.success());
            if fetched && std::fs::rename(&part, &path).is_ok() {
                Arrived::changed();
            } else {
                // a part file that was never written is gone already
                let _ = std::fs::remove_file(&part);
            }
        });
    }
    None
}

/// Where the art at `url` is kept: `$XDG_CACHE_HOME/emaki/art/<hash>`.
fn cached(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let base = emaki_kit::paths::cache();
    base.join(emaki_kit::app_ids::SHELL)
        .join("art")
        .join(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_art_is_cached_by_its_url() {
        let a = cached("https://x/1.jpg");
        assert_eq!(a, cached("https://x/1.jpg"));
        assert_ne!(a, cached("https://x/2.jpg"));
        assert!(a.ends_with(a.file_name().unwrap()));
        assert!(a.to_string_lossy().contains("emaki/art"));
    }
}
