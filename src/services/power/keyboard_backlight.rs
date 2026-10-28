//! Keyboard backlight — the light under the keys.
//!
//! Read from the first `*::kbd_backlight` LED in `/sys/class/leds`, polled
//! like amane's screen `Brightness` (the kernel announces no change, and the
//! keyboard's own key changes it too). The file belongs to root, so logind
//! writes it (`Session.SetBrightness("leds", …)`), which it allows whoever
//! sits at the machine. Most keyboards have a few steps (a ThinkPad's: off,
//! low, high), so it works in steps up to [`max`](KeyboardBacklight::max).

use std::path::{Path, PathBuf};
use std::time::Duration;

use amane::{Argument, Bus, Service};

use crate::services::Latest;

const LEDS: &str = "/sys/class/leds";

const LOGIND: &str = "org.freedesktop.login1";
const SESSION_PATH: &str = "/org/freedesktop/login1/session/auto";
const SESSION: &str = "org.freedesktop.login1.Session";

static PENDING: Latest<u32> = Latest::new();

pub struct KeyboardBacklight {
    path: Option<PathBuf>,
    level: u32,
    max: u32,
}

impl Service for KeyboardBacklight {
    fn new() -> Self {
        let path = find(Path::new(LEDS));
        let (level, max) = path.as_deref().map_or((0, 0), read);
        Self { path, level, max }
    }

    fn interval() -> Duration {
        Duration::from_millis(500)
    }

    fn update(&mut self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        let now = read(path);
        let changed = now != (self.level, self.max);
        (self.level, self.max) = now;
        changed
    }
}

impl KeyboardBacklight {
    pub fn present(&self) -> bool {
        self.path.is_some() && self.max > 0
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn max(&self) -> u32 {
        self.max
    }

    /// Clamped to `0..=max`.
    pub fn set(level: u32) {
        PENDING.send(level, write_level);
    }
}

fn write_level(level: u32) {
    let (path, max) = {
        let light = KeyboardBacklight::read();
        (light.path.clone(), light.max)
    };
    let Some(name) = path
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return;
    };

    let arguments = [
        Argument::from("leds"),
        Argument::from(name),
        Argument::from(level.min(max)),
    ];
    Bus::system().call(LOGIND, SESSION_PATH, SESSION, "SetBrightness", &arguments);

    KeyboardBacklight::write().update();
}

/// The first keyboard light under `root`.
fn find(root: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("::kbd_backlight"))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Its level and highest level.
fn read(path: &Path) -> (u32, u32) {
    let read = |name: &str| {
        std::fs::read_to_string(path.join(name))
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0)
    };
    (read("brightness"), read("max_brightness"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_keyboard_light_among_the_leds() {
        let root = std::env::temp_dir().join(format!("amane-leds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for name in [
            "input3::capslock",
            "tpacpi::kbd_backlight",
            "platform::mute",
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        let light = root.join("tpacpi::kbd_backlight");
        std::fs::write(light.join("brightness"), "1\n").unwrap();
        std::fs::write(light.join("max_brightness"), "2\n").unwrap();

        assert_eq!(find(&root), Some(light.clone()));
        assert_eq!(read(&light), (1, 2));

        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(find(&root), None);
    }
}
