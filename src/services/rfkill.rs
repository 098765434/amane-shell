//! Airplane mode — every radio (Wi-Fi, Bluetooth, mobile, NFC) blocked.
//!
//! Read from the kernel's rfkill switches in `/sys/class/rfkill`, polled
//! because sysfs announces no change there, and set with `rfkill block all`
//! / `unblock all` (the seat's user may write `/dev/rfkill`). It's on when
//! every radio is blocked, by software or by a hardware switch.

use std::path::Path;

use amane::Service;

use super::run;

const ROOT: &str = "/sys/class/rfkill";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Radio {
    pub kind: String,
    pub soft: bool,
    pub hard: bool,
}

#[derive(PartialEq)]
pub struct Rfkill {
    radios: Vec<Radio>,
}

impl Service for Rfkill {
    fn new() -> Self {
        Self {
            radios: radios(Path::new(ROOT)),
        }
    }

    fn update(&mut self) -> bool {
        let radios = radios(Path::new(ROOT));
        let changed = radios != self.radios;
        self.radios = radios;
        changed
    }
}

impl Rfkill {
    /// Whether there's a radio to block, which the tile needs to show.
    pub fn present(&self) -> bool {
        !self.radios.is_empty()
    }

    /// Whether there's a radio of `kind` (`wlan`, `bluetooth`, `wwan` …).
    pub fn has(&self, kind: &str) -> bool {
        self.radios.iter().any(|radio| radio.kind == kind)
    }

    /// Whether every radio of `kind` is held off by a hardware switch.
    pub fn hard_blocked(&self, kind: &str) -> bool {
        let mut radios = self.radios.iter().filter(|radio| radio.kind == kind);
        radios.clone().next().is_some() && radios.all(|radio| radio.hard)
    }

    pub fn airplane(&self) -> bool {
        airplane(&self.radios)
    }

    pub fn set_airplane(on: bool) {
        let verb = if on { "block" } else { "unblock" };
        run("rfkill", vec![verb.into(), "all".into()]);
    }
}

fn airplane(radios: &[Radio]) -> bool {
    !radios.is_empty() && radios.iter().all(|radio| radio.soft || radio.hard)
}

/// Every switch under `root`, as sysfs lists them.
fn radios(root: &Path) -> Vec<Radio> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let read = |dir: &Path, name: &str| {
        std::fs::read_to_string(dir.join(name))
            .map(|text| text.trim().to_string())
            .unwrap_or_default()
    };

    let mut radios: Vec<(String, Radio)> = entries
        .flatten()
        .map(|entry| {
            let dir = entry.path();
            let radio = Radio {
                kind: read(&dir, "type"),
                soft: read(&dir, "soft") == "1",
                hard: read(&dir, "hard") == "1",
            };
            (entry.file_name().to_string_lossy().into_owned(), radio)
        })
        .filter(|(_, radio)| !radio.kind.is_empty())
        .collect();
    // sysfs order isn't stable, and a reorder isn't a change
    radios.sort_by(|a, b| a.0.cmp(&b.0));
    radios.into_iter().map(|(_, radio)| radio).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn switch(root: &Path, name: &str, kind: &str, soft: u8, hard: u8) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("type"), format!("{kind}\n")).unwrap();
        std::fs::write(dir.join("soft"), format!("{soft}\n")).unwrap();
        std::fs::write(dir.join("hard"), format!("{hard}\n")).unwrap();
    }

    #[test]
    fn airplane_mode_needs_every_radio_blocked() {
        let root = std::env::temp_dir().join(format!("amane-rfkill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        switch(&root, "rfkill0", "bluetooth", 0, 0);
        switch(&root, "rfkill1", "wlan", 1, 0);

        let found = radios(&root);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, "bluetooth");
        assert!(!airplane(&found));

        // a hardware switch counts as blocked too
        switch(&root, "rfkill0", "bluetooth", 0, 1);
        assert!(airplane(&radios(&root)));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn no_radios_is_not_airplane_mode() {
        assert!(!airplane(&[]));
        assert!(radios(Path::new("/nonexistent/rfkill")).is_empty());
    }
}
