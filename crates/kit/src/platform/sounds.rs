//! The freedesktop sound theme's sounds (`sounds/freedesktop/stereo/` in
//! the XDG data folders), and playing one through PipeWire's `pw-play`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::process::{Cmd, installed};
use crate::sync::Locked as _;

/// The theme every sound comes from; freedesktop's is the one the spec
/// falls back to, and the one installed alongside PipeWire.
const THEME: &str = "freedesktop";

/// The formats `pw-play` reads.
const EXTENSIONS: [&str; 3] = ["oga", "ogg", "wav"];

/// The theme's sounds by name (`message-new-instant`), each its file in
/// the first data folder that has it, sorted by name.
pub fn theme_sounds() -> Vec<(String, PathBuf)> {
    sounds_in(&data_dirs())
}

/// The file of the theme's sound `name`, when it's installed.
pub fn sound_path(name: &str) -> Option<PathBuf> {
    theme_sounds()
        .into_iter()
        .find(|(found, _)| found == name)
        .map(|(_, path)| path)
}

fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![crate::paths::data()];
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| String::from("/usr/local/share:/usr/share"));
    dirs.extend(std::env::split_paths(&system));
    dirs
}

fn sounds_in(dirs: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut found = BTreeMap::new();
    for dir in dirs {
        let stereo = dir.join("sounds").join(THEME).join("stereo");
        for entry in std::fs::read_dir(&stereo).into_iter().flatten().flatten() {
            let path = entry.path();
            let Some(name) = sound_name(&path) else {
                continue;
            };
            // the first folder wins, as XDG orders them
            found.entry(name).or_insert(path);
        }
    }
    found.into_iter().collect()
}

fn sound_name(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?;
    if !EXTENSIONS.contains(&extension) {
        return None;
    }
    Some(path.file_stem()?.to_str()?.to_string())
}

/// What a menu calls the sound `name`: the theme's names in words, a few
/// put the way people say them.
pub fn label(name: &str) -> String {
    let known = match name {
        "message-new-instant" => Some("Message"),
        "message" => Some("Message (soft)"),
        "bell" => Some("Bell"),
        "complete" => Some("Complete"),
        "dialog-information" => Some("Information"),
        "dialog-warning" => Some("Warning"),
        "window-attention" => Some("Attention"),
        "phone-incoming-call" => Some("Incoming call"),
        "alarm-clock-elapsed" => Some("Alarm"),
        _ => None,
    };
    if let Some(known) = known {
        return known.to_string();
    }
    let words = name.replace(['-', '_'], " ");
    let mut chars = words.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Plays the theme's sound `name` at `volume` (0 to 100), as an event
/// sound, without waiting for it. Nothing plays when `pw-play` or the
/// sound isn't installed, or when a sound started less than `gap` ago, so
/// a burst of notifications plays one.
pub fn play(name: &str, volume: u8, gap: Duration) {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let mut last = LAST.locked();
        if last.is_some_and(|at| at.elapsed() < gap) {
            return;
        }
        *last = Some(Instant::now());
    }
    let Some(path) = sound_path(name) else {
        log::warn!("no sound named {name} in the {THEME} theme");
        return;
    };
    if !installed("pw-play") {
        return;
    }
    let volume = f32::from(volume.min(100)) / 100.0;
    let played = Cmd::new("pw-play")
        // pulse calls this role "event", which mixers leave out
        .arg("--media-role=Notification")
        .arg(format!("--volume={volume:.2}"))
        .arg(&path)
        .detach();
    if let Err(error) = played {
        log::warn!("pw-play: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_found_by_name_first_folder_first() {
        let root = std::env::temp_dir().join(format!("emaki-sounds-{}", std::process::id()));
        let stereo = |dir: &str| root.join(dir).join("sounds/freedesktop/stereo");
        for dir in ["a", "b"] {
            std::fs::create_dir_all(stereo(dir)).expect("folder");
        }
        std::fs::write(stereo("a").join("bell.oga"), "").expect("file");
        std::fs::write(stereo("b").join("bell.oga"), "").expect("file");
        std::fs::write(stereo("b").join("complete.wav"), "").expect("file");
        std::fs::write(stereo("b").join("index.theme"), "").expect("file");

        let found = sounds_in(&[root.join("a"), root.join("b"), root.join("none")]);
        let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["bell", "complete"]);
        assert_eq!(found[0].1, stereo("a").join("bell.oga"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_read_as_words() {
        assert_eq!(label("message-new-instant"), "Message");
        assert_eq!(
            label("network-connectivity-lost"),
            "Network connectivity lost"
        );
        assert_eq!(label(""), "");
    }
}
