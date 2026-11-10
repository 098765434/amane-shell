//! Refresh rate by power source. `displays.kdl` (Emaki Settings' Displays
//! page) holds each display's rate on the power adapter; `displays.json`
//! holds what it does on battery ([`BatteryRefresh`]): stay the same, its
//! slowest rate, a rate up to a cap, or variable refresh up to a cap.
//!
//! On battery each display is moved there with niri's temporary changes
//! (`niri msg output … mode` / `vrr`). Back on AC, or when every choice goes
//! back to Same, niri reads its config again, which puts back exactly what
//! `displays.kdl` sets, a rate changed in Settings meanwhile and on-demand
//! variable refresh included (niri's outputs only say whether it's on now,
//! not how it's set). A config reload on battery brings back the adapter's
//! settings for a moment, until the check that `Outputs` changing starts
//! moves them again.
//!
//! Checked when amane's `Battery` or `Outputs` change, or the setting does;
//! the outputs are only read while on battery or while a display waits to
//! go back.

use std::collections::BTreeMap;
use std::sync::Once;

use amane::{Battery, Config, Outputs, Service, niri};
use emaki_kit::settings::{BatteryRefresh, Displays, displays, output_key};

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
    /// What [`displays.json`](Displays) keeps its choice under.
    key: String,
    modes: Vec<Mode>,
    current_mode: Option<usize>,
    vrr_supported: bool,
    vrr_enabled: bool,
}

/// Where a display should be on battery.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    mode: Mode,
    /// Variable refresh on, always.
    vrr: bool,
}

/// A display moved for the battery.
#[derive(Clone, Copy, Debug, Default)]
struct Moved {
    /// Whether variable refresh was turned on here.
    vrr: bool,
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
            // the displays moved for the battery, by connector
            let mut moved: BTreeMap<String, Moved> = BTreeMap::new();
            loop {
                check(&mut moved);
                WAKE.wait(None);
            }
        });
    });
}

fn check(moved: &mut BTreeMap<String, Moved>) {
    let battery = Battery::read();
    let on_battery = battery.present() && !battery.on_ac();
    drop(battery);
    let choices = on_battery.then(|| displays().clone());
    let any = choices.as_ref().is_some_and(|choices| {
        choices.battery_refresh
            || choices
                .on_battery
                .values()
                .any(|choice| *choice != BatteryRefresh::Same)
    });

    if !any && moved.is_empty() {
        return;
    }
    let Some(outputs) = outputs() else {
        return;
    };

    let mut reload = false;
    for (name, output) in outputs {
        let Some(now) = output
            .current_mode
            .and_then(|i| output.modes.get(i).copied())
        else {
            continue;
        };
        let choice = choices
            .as_ref()
            .map_or(BatteryRefresh::Same, |choices| choices.battery(&output.key));
        match target(choice, &output.modes, now, output.vrr_supported) {
            Some(target) => {
                let turn_on = target.vrr && !output.vrr_enabled;
                if target.mode == now && !turn_on && !moved.contains_key(&name) {
                    // already there on AC's own settings: nothing to put back
                    continue;
                }
                if target.mode != now {
                    set_mode(&name, target.mode);
                }
                let before = moved.entry(name.clone()).or_default();
                if turn_on {
                    set_vrr(&name);
                    before.vrr = true;
                } else if !target.vrr && before.vrr {
                    // was variable, now isn't: niri's config has the adapter's
                    before.vrr = false;
                    reload = true;
                }
            }
            None => {
                // back to what displays.kdl sets, by reading it again
                reload |= moved.remove(&name).is_some();
            }
        }
    }
    if !on_battery {
        moved.clear();
    }
    if reload {
        load_config();
    }
}

/// Where `choice` puts a display showing `now`, or none to leave it as on AC.
fn target(
    choice: BatteryRefresh,
    modes: &[Mode],
    now: Mode,
    vrr_supported: bool,
) -> Option<Target> {
    let fixed = |mode| Target { mode, vrr: false };
    match choice {
        BatteryRefresh::Same => None,
        BatteryRefresh::Slowest => Some(fixed(slowest(modes, now))),
        BatteryRefresh::Rate { mhz } => Some(fixed(up_to(modes, now, mhz))),
        BatteryRefresh::Variable { mhz } => Some(Target {
            mode: up_to(modes, now, mhz),
            vrr: vrr_supported,
        }),
    }
}

/// The modes at `now`'s resolution.
fn same_size(modes: &[Mode], now: Mode) -> impl Iterator<Item = Mode> + '_ {
    modes
        .iter()
        .copied()
        .filter(move |m| (m.width, m.height) == (now.width, now.height))
}

/// The slowest mode at `now`'s resolution.
fn slowest(modes: &[Mode], now: Mode) -> Mode {
    same_size(modes, now)
        .min_by_key(|m| m.refresh_rate)
        .unwrap_or(now)
}

/// The fastest mode at `now`'s resolution that isn't above `mhz` (half a
/// hertz either way, as 59.94 counts as 60), else the slowest.
fn up_to(modes: &[Mode], now: Mode, mhz: u32) -> Mode {
    same_size(modes, now)
        .filter(|m| m.refresh_rate <= mhz.saturating_add(500))
        .max_by_key(|m| m.refresh_rate)
        .unwrap_or_else(|| slowest(modes, now))
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
                        key: output_key(output),
                        modes: output.modes.iter().copied().map(Mode::from).collect(),
                        current_mode: output.current_mode,
                        vrr_supported: output.vrr_supported,
                        vrr_enabled: output.vrr_enabled,
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
        log::warn!("couldn't set {name}'s refresh rate: {err}");
    }
}

fn set_vrr(name: &str) {
    let change = niri::OutputChange::Vrr {
        on: true,
        on_demand: false,
    };
    if let Err(err) = niri::set_output(name, change) {
        log::warn!("couldn't turn on {name}'s variable refresh: {err}");
    }
}

/// niri reads its config again, putting back what `displays.kdl` sets.
fn load_config() {
    let args = ["msg", "action", "load-config-file"].map(String::from);
    let _ = emaki_kit::process::run_now("niri", &args);
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

    #[test]
    fn a_cap_picks_the_fastest_rate_under_it() {
        let modes = [
            mode(2880, 1800, 60),
            mode(2880, 1800, 90),
            mode(2880, 1800, 120),
        ];
        let now = modes[2];
        assert_eq!(up_to(&modes, now, 100_000), modes[1]);
        assert_eq!(up_to(&modes, now, 60_000), modes[0]);
        // 59.94 counts as 60
        let ntsc = [Mode {
            refresh_rate: 59_940,
            ..modes[0]
        }];
        assert_eq!(up_to(&ntsc, ntsc[0], 59_000), ntsc[0]);
        // nothing that slow: the slowest
        assert_eq!(up_to(&modes, now, 30_000), modes[0]);
    }

    #[test]
    fn each_choice_has_its_target() {
        let modes = [mode(2880, 1800, 60), mode(2880, 1800, 120)];
        let now = modes[1];
        assert_eq!(target(BatteryRefresh::Same, &modes, now, true), None);
        assert_eq!(
            target(BatteryRefresh::Slowest, &modes, now, true),
            Some(Target {
                mode: modes[0],
                vrr: false
            })
        );
        assert_eq!(
            target(BatteryRefresh::Variable { mhz: 60_000 }, &modes, now, true),
            Some(Target {
                mode: modes[0],
                vrr: true
            })
        );
        // a panel without it only gets the rate
        assert_eq!(
            target(
                BatteryRefresh::Variable { mhz: 120_000 },
                &modes,
                now,
                false
            ),
            Some(Target {
                mode: modes[1],
                vrr: false
            })
        );
    }

    const OUTPUTS: &str = include_str!("../../../tests/fixtures/niri/outputs.json");

    /// An output's modes and the index of the one it shows.
    type Modes = (Vec<Mode>, Option<usize>);

    /// niri's `outputs` JSON, by output name.
    fn parse_outputs(json: &str) -> Option<BTreeMap<String, Modes>> {
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
                    (name.clone(), (modes, current_mode))
                })
                .collect(),
        )
    }

    #[test]
    fn the_recorded_panel_at_120_hz_lowers_to_60() {
        let outputs = parse_outputs(OUTPUTS).expect("niri's outputs JSON");
        let (modes, current) = &outputs["eDP-1"];
        assert_eq!(modes.len(), 13);
        let now = modes[current.expect("a mode is on")];
        assert_eq!(now, mode(2880, 1800, 120));
        assert_eq!(slowest(modes, now), mode(2880, 1800, 60));
        assert_eq!(up_to(modes, now, 60_000), mode(2880, 1800, 60));
        assert_eq!(
            target(BatteryRefresh::Variable { mhz: 60_000 }, modes, now, true),
            Some(Target {
                mode: mode(2880, 1800, 60),
                vrr: true
            })
        );
        assert!(parse_outputs("not json").is_none());
    }
}
