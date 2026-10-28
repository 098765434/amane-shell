//! Refresh rate by power source: with "Lower the refresh rate on battery"
//! on (Shell Settings' Displays page, `displays.json`), each display runs at
//! its slowest rate at the same resolution while on battery, and goes back
//! to the rate it had once on AC.
//!
//! The change is niri's temporary kind (`niri msg output … mode`), so
//! `displays.kdl` keeps the rate for AC. A config reload puts that back
//! even on battery; the next check lowers it again. Checked every five
//! seconds from amane's `Battery`, and niri is only asked while on battery
//! or while a display waits to go back.

use std::collections::BTreeMap;
use std::sync::Once;
use std::time::Duration;

use amane::serde::Deserialize;
use amane::{Battery, Service};
use amane_kit::settings::displays;

use super::run_now;

const EVERY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(crate = "amane::serde")]
struct Mode {
    width: u16,
    height: u16,
    refresh_rate: u32,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "amane::serde")]
struct Output {
    modes: Vec<Mode>,
    current_mode: Option<usize>,
}

/// Starts watching, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        std::thread::spawn(|| {
            // the mode each lowered display had on AC, by connector
            let mut lowered: BTreeMap<String, Mode> = BTreeMap::new();
            loop {
                check(&mut lowered);
                std::thread::sleep(EVERY);
            }
        });
    });
}

fn check(lowered: &mut BTreeMap<String, Mode>) {
    let battery = Battery::read();
    let on_battery = battery.present() && !battery.on_ac();
    drop(battery);
    let wanted = on_battery && displays().battery_refresh;

    if !wanted && lowered.is_empty() {
        return;
    }
    let Some(outputs) = outputs() else {
        return;
    };

    for (name, output) in outputs {
        let Some(now) = output
            .current_mode
            .and_then(|i| output.modes.get(i).copied())
        else {
            continue;
        };
        if wanted {
            let slowest = slowest(&output.modes, now);
            if slowest != now {
                lowered.entry(name.clone()).or_insert(now);
                set_mode(&name, slowest);
            }
        } else if let Some(before) = lowered.remove(&name) {
            // only if it's still where it was put: a change made since stays
            if (now.width, now.height) == (before.width, before.height) && now != before {
                set_mode(&name, before);
            }
        }
    }
    if !wanted {
        lowered.clear();
    }
}

/// The slowest mode at `now`'s resolution.
fn slowest(modes: &[Mode], now: Mode) -> Mode {
    modes
        .iter()
        .copied()
        .filter(|m| (m.width, m.height) == (now.width, now.height))
        .min_by_key(|m| m.refresh_rate)
        .unwrap_or(now)
}

fn outputs() -> Option<BTreeMap<String, Output>> {
    let json = run_now("niri", &["msg".into(), "--json".into(), "outputs".into()])?;
    serde_json::from_str(&json).ok()
}

fn set_mode(name: &str, mode: Mode) {
    let target = format!(
        "{}x{}@{:.3}",
        mode.width,
        mode.height,
        f64::from(mode.refresh_rate) / 1000.
    );
    let _ = run_now(
        "niri",
        &[
            "msg".into(),
            "output".into(),
            name.into(),
            "mode".into(),
            target,
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(width: u16, height: u16, hz: u32) -> Mode {
        Mode {
            width,
            height,
            refresh_rate: hz * 1000,
        }
    }

    #[test]
    fn the_slowest_rate_at_the_same_resolution() {
        let modes = [
            mode(2880, 1800, 60),
            mode(2880, 1800, 120),
            mode(1920, 1200, 48),
        ];
        assert_eq!(slowest(&modes, modes[1]), modes[0]);
        // nothing slower at its size
        assert_eq!(slowest(&modes, modes[2]), modes[2]);
    }
}
