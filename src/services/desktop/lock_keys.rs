//! Lock keys — whether Caps Lock and Num Lock are on, for the on-screen
//! display.
//!
//! Read from the keyboards' own LEDs in `/sys/class/leds` (`input3::capslock`
//! …), which niri lights as the lock changes. Nothing else says: niri's IPC
//! has no lock events, and a layer surface that never takes the keyboard
//! never sees the modifiers. Polled like the keyboard light, a little
//! quicker, as the display answers a key. Every keyboard has its own LED,
//! so a lock is on while any of them is lit, and keyboards plugged in later
//! are found on the next poll.

use std::path::Path;
use std::time::Duration;

use amane::Service;

const LEDS: &str = "/sys/class/leds";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockKeys {
    // `None` with no keyboard that has the LED
    caps: Option<bool>,
    num: Option<bool>,
}

impl Service for LockKeys {
    fn new() -> Self {
        read(Path::new(LEDS))
    }

    fn interval() -> Duration {
        Duration::from_millis(250)
    }

    fn update(&mut self) -> bool {
        let now = read(Path::new(LEDS));
        let changed = now != *self;
        *self = now;
        changed
    }
}

impl LockKeys {
    pub fn caps(&self) -> Option<bool> {
        self.caps
    }

    pub fn num(&self) -> Option<bool> {
        self.num
    }
}

/// The locks as the LEDs under `root` show them.
fn read(root: &Path) -> LockKeys {
    let Ok(entries) = std::fs::read_dir(root) else {
        return LockKeys::default();
    };
    let mut keys = LockKeys::default();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let lock = if name.ends_with("::capslock") {
            &mut keys.caps
        } else if name.ends_with("::numlock") {
            &mut keys.num
        } else {
            continue;
        };
        let lit = std::fs::read_to_string(entry.path().join("brightness"))
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
            .is_some_and(|level| level > 0);
        *lock = Some(lock.unwrap_or(false) || lit);
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_is_on_while_any_keyboard_lights_it() {
        let root = std::env::temp_dir().join(format!("amane-lock-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (name, level) in [
            ("input3::capslock", "0"),
            ("input9::capslock", "1"),
            ("input3::numlock", "0"),
            ("tpacpi::kbd_backlight", "2"),
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
            std::fs::write(root.join(name).join("brightness"), format!("{level}\n")).unwrap();
        }

        let keys = read(&root);
        assert_eq!(keys.caps(), Some(true));
        assert_eq!(keys.num(), Some(false));

        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(read(&root), LockKeys::default());
    }
}
