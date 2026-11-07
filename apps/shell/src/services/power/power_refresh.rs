//! Refresh rate by power source: with "Lower the refresh rate on battery"
//! on (Shell Settings' Displays page, `displays.json`), each display runs at
//! its slowest rate at the same resolution while on battery, and goes back
//! to the rate it had once on AC.
//!
//! The change is niri's temporary kind (`niri msg output … mode`), so
//! `displays.kdl` keeps the rate for AC. A config reload puts that back
//! even on battery; the next check lowers it again. Checked when amane's
//! `Battery` or `Outputs` change, or the setting does; the outputs are only
//! read while on battery or while a display waits to go back.

use std::collections::BTreeMap;
use std::sync::Once;

use amane::{Battery, Config, Outputs, Service, niri};
use amane_kit::settings::{Displays, displays};

use crate::services::Wake;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mode {
    width: u16,
    height: u16,
    refresh_rate: u32,
}

impl From<niri::Mode> for Mode {
    fn from(mode: niri::Mode) -> Self {
        Self {
            width: mode.width,
            height: mode.height,
            refresh_rate: mode.refresh_rate,
        }
    }
}

#[derive(Debug)]
struct Output {
    modes: Vec<Mode>,
    current_mode: Option<usize>,
}

static WAKE: Wake = Wake::new();

/// Starts watching, once.
pub fn watch() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Battery::subscribe(|_| WAKE.nudge()).forever();
        Outputs::subscribe(|_| WAKE.nudge()).forever();
        Config::<Displays>::subscribe(|_| WAKE.nudge()).forever();
        std::thread::spawn(|| {
            // the mode each lowered display had on AC, by connector
            let mut lowered: BTreeMap<String, Mode> = BTreeMap::new();
            loop {
                check(&mut lowered);
                WAKE.wait(None);
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

// read only here, while a change is wanted: reading keeps amane's Outputs watched
fn outputs() -> Option<BTreeMap<String, Output>> {
    let outputs = Outputs::read();
    if outputs.all().is_empty() {
        return None;
    }
    Some(
        outputs
            .all()
            .iter()
            .map(|output| {
                (
                    output.name.clone(),
                    Output {
                        modes: output.modes.iter().copied().map(Mode::from).collect(),
                        current_mode: output.current_mode,
                    },
                )
            })
            .collect(),
    )
}

fn set_mode(name: &str, mode: Mode) {
    let mode = niri::Mode {
        width: mode.width,
        height: mode.height,
        refresh_rate: mode.refresh_rate,
        is_preferred: false,
    };
    if let Err(err) = niri::set_output_mode(name, mode) {
        eprintln!("amane-shell: couldn't set {name}'s refresh rate: {err}");
    }
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

    const OUTPUTS: &str = include_str!("../../../tests/fixtures/niri/outputs.json");

    /// niri's `outputs` JSON, by output name.
    fn parse_outputs(json: &str) -> Option<BTreeMap<String, Output>> {
        let outputs: serde_json::Value = serde_json::from_str(json).ok()?;
        let mode = |m: &serde_json::Value| {
            Some(Mode {
                width: u16::try_from(m["width"].as_u64()?).ok()?,
                height: u16::try_from(m["height"].as_u64()?).ok()?,
                refresh_rate: u32::try_from(m["refresh_rate"].as_u64()?).ok()?,
            })
        };
        Some(
            outputs
                .as_object()?
                .iter()
                .map(|(name, output)| {
                    let modes = output["modes"]
                        .as_array()
                        .map(|modes| modes.iter().filter_map(mode).collect())
                        .unwrap_or_default();
                    let current_mode = output["current_mode"]
                        .as_u64()
                        .and_then(|i| usize::try_from(i).ok());
                    (
                        name.clone(),
                        Output {
                            modes,
                            current_mode,
                        },
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn the_recorded_panel_at_120_hz_lowers_to_60() {
        let outputs = parse_outputs(OUTPUTS).expect("niri's outputs JSON");
        let panel = &outputs["eDP-1"];
        assert_eq!(panel.modes.len(), 13);
        let now = panel.modes[panel.current_mode.expect("a mode is on")];
        assert_eq!(now, mode(2880, 1800, 120));
        assert_eq!(slowest(&panel.modes, now), mode(2880, 1800, 60));
        assert!(parse_outputs("not json").is_none());
    }
}
