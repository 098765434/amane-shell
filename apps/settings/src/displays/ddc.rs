//! An external monitor's own brightness and contrast, over DDC/CI with `ddcutil`. The
//! connector's sysfs folder links to its I²C bus (`ddc`), so no detection is needed. Each call
//! takes a tenth of a second or more, so reads and writes run off the UI thread, and a slider
//! dragged sends only where it rests.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use amane::Service;

use super::drm;
use crate::system::nixos::run;

/// The VCP features the page sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    Brightness,
    Contrast,
}

impl Feature {
    fn code(self) -> &'static str {
        match self {
            Feature::Brightness => "10",
            Feature::Contrast => "12",
        }
    }
}

/// A feature's value and its maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub value: u32,
    pub max: u32,
}

impl Level {
    /// 0–100 for a slider.
    pub fn percent(self) -> u8 {
        if self.max == 0 {
            return 0;
        }
        (u64::from(self.value.min(self.max)) * 100 / u64::from(self.max)) as u8
    }

    fn from_percent(percent: u8, max: u32) -> u32 {
        (u64::from(percent.min(100)) * u64::from(max) / 100) as u32
    }
}

/// What a monitor answered.
#[derive(Clone, Debug, PartialEq)]
pub enum Monitor {
    Reading,
    /// It doesn't speak DDC/CI, or `ddcutil` isn't there.
    Silent,
    Levels {
        bus: u32,
        brightness: Option<Level>,
        contrast: Option<Level>,
    },
}

/// Each external monitor's answer, by connector.
#[derive(Default)]
pub struct Ddc {
    monitors: HashMap<String, Monitor>,
}

impl Service for Ddc {
    fn new() -> Self {
        Self::default()
    }

    // only the page asks
    fn listen() {}
}

impl Ddc {
    /// The monitor on `connector`, asking it the first time.
    pub fn monitor(connector: &str) -> Monitor {
        if let Some(monitor) = Self::read().monitors.get(connector) {
            return monitor.clone();
        }
        Self::write()
            .monitors
            .insert(connector.to_string(), Monitor::Reading);
        let connector = connector.to_string();
        thread::spawn(move || {
            let monitor = ask(&connector);
            Self::write().monitors.insert(connector, monitor);
        });
        Monitor::Reading
    }

    /// Sets `feature` to `percent` once the slider rests, and shows it at once.
    pub fn set(connector: &str, feature: Feature, percent: u8) {
        static LATEST: AtomicU64 = AtomicU64::new(0);
        let turn = LATEST.fetch_add(1, Ordering::Relaxed) + 1;

        let target = {
            let mut ddc = Self::write();
            let Some(Monitor::Levels {
                bus,
                brightness,
                contrast,
            }) = ddc.monitors.get_mut(connector)
            else {
                return;
            };
            let level = match feature {
                Feature::Brightness => brightness,
                Feature::Contrast => contrast,
            };
            let Some(level) = level else {
                return;
            };
            level.value = Level::from_percent(percent, level.max);
            (*bus, level.value)
        };

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            if LATEST.load(Ordering::Relaxed) != turn {
                return;
            }
            let (bus, value) = target;
            let _ = run(
                "ddcutil",
                &[
                    "--bus",
                    &bus.to_string(),
                    "--noverify",
                    "setvcp",
                    feature.code(),
                    &value.to_string(),
                ],
                None,
            );
        });
    }
}

/// The I²C bus the connector's DDC goes over.
fn bus(connector: &str) -> Option<u32> {
    let link = fs::read_link(drm::sysfs(connector)?.join("ddc")).ok()?;
    link.file_name()?
        .to_str()?
        .strip_prefix("i2c-")?
        .parse()
        .ok()
}

fn ask(connector: &str) -> Monitor {
    let Some(bus) = bus(connector) else {
        return Monitor::Silent;
    };
    let get = |feature: Feature| {
        run(
            "ddcutil",
            &[
                "--bus",
                &bus.to_string(),
                "--brief",
                "getvcp",
                feature.code(),
            ],
            None,
        )
        .ok()
        .and_then(|out| parse_brief(&out))
    };
    let brightness = get(Feature::Brightness);
    if brightness.is_none() {
        return Monitor::Silent;
    }
    Monitor::Levels {
        bus,
        brightness,
        contrast: get(Feature::Contrast),
    }
}

/// `ddcutil --brief getvcp`'s answer for a continuous feature: `VCP 10 C 50 100`.
fn parse_brief(out: &str) -> Option<Level> {
    let line = out.lines().find(|line| line.starts_with("VCP "))?;
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        ["VCP", _, "C", value, max, ..] => Some(Level {
            value: value.parse().ok()?,
            max: max.parse().ok()?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ddcutils_brief_answer() {
        assert_eq!(
            parse_brief("VCP 10 C 50 100\n"),
            Some(Level {
                value: 50,
                max: 100
            })
        );
        // a feature the monitor doesn't have
        assert_eq!(parse_brief("VCP 12 ERR\n"), None);
        assert_eq!(parse_brief(""), None);
        assert_eq!(Level { value: 30, max: 60 }.percent(), 50);
        assert_eq!(Level::from_percent(50, 60), 30);
    }
}
