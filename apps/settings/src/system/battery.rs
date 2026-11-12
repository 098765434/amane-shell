//! Battery charge thresholds on the Power & Lock page: how full the battery
//! charges (end) and how empty it gets before charging starts again (start).
//! Sysfs is world-readable, so this reads it directly; writing goes through
//! `pkexec emaki-charge-threshold`, the polkit-guarded helper in the NixOS
//! module (see `nix/nixos-module.nix`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

use amane::Service;
use emaki_kit::form::{Group, SliderRow};
use emaki_kit::slider::Slider;

use crate::group;

/// The lowest `end` the slider offers. Below 50 % the charge control stops
/// being useful on most firmware (some clamp the lower bound anyway).
const MIN_END: u8 = 50;

/// What a battery exposes under `/sys/class/power_supply/BAT*`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Limits {
    start: u8,
    end: u8,
}

/// The thresholds this machine lets us set, or none when nothing under
/// `/sys/class/power_supply/BAT*` exposes both files.
pub struct ChargeThreshold {
    limits: Option<Limits>,
    /// Set once the first read ran; before that the section says it's asking,
    /// not that charge control is missing.
    asked: bool,
    /// What the last write ran into, cleared on the next successful write.
    error: Option<String>,
}

impl Service for ChargeThreshold {
    fn new() -> Self {
        Self {
            limits: None,
            asked: false,
            error: None,
        }
    }

    fn listen() {
        let limits = read_sysfs();
        let mut now = Self::write();
        now.limits = limits;
        now.asked = true;
    }
}

/// `/sys/class/power_supply/BAT*/charge_control_*_threshold`, as a pair of
/// integers, or `None` if no battery exposes both files.
fn read_sysfs() -> Option<Limits> {
    for (start_file, end_file) in battery_files() {
        let start = read_u8(&start_file);
        let end = read_u8(&end_file);
        if let (Some(start), Some(end)) = (start, end) {
            return Some(Limits { start, end });
        }
    }
    None
}

/// Every battery's `(start_file, end_file)` pair.
fn battery_files() -> Vec<(PathBuf, PathBuf)> {
    let Ok(entries) = fs::read_dir("/sys/class/power_supply") else {
        return Vec::new();
    };
    let mut batteries = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("BAT") {
            continue;
        }
        let start_file = entry.path().join("charge_control_start_threshold");
        let end_file = entry.path().join("charge_control_end_threshold");
        batteries.push((start_file, end_file));
    }
    batteries
}

fn read_u8(file: &Path) -> Option<u8> {
    fs::read_to_string(file).ok()?.trim().parse().ok()
}

/// Asks `emaki-charge-threshold` to write `(start, end)`, off the UI thread,
/// then re-reads sysfs so the sliders rehydrate.
fn tell(start: u8, end: u8) {
    thread::spawn(move || {
        let output = Command::new("pkexec")
            .arg("emaki-charge-threshold")
            .arg(start.to_string())
            .arg(end.to_string())
            .output();
        let error = match output {
            Ok(result) if result.status.success() => None,
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
                Some(if stderr.is_empty() {
                    format!("The helper exited {}", result.status)
                } else {
                    stderr
                })
            }
            Err(why) => Some(format!("Couldn't run pkexec: {why}")),
        };
        let limits = read_sysfs();
        let mut now = ChargeThreshold::write();
        now.limits = limits;
        now.error = error;
    });
}

pub fn section() -> Group {
    let (limits, asked, error) = {
        let now = ChargeThreshold::read();
        (now.limits, now.asked, now.error.clone())
    };

    if !asked {
        return group().header("Battery charge").row(crate::loading_row(
            "settings.system.charge.asking",
            "Reading the battery…",
        ));
    }

    let Some(limits) = limits else {
        // every section() call rebuilds Group, so dropping the group on no-threshold machines
        // keeps the page from carrying a useless row; the caller filters empty groups out
        return Group::new();
    };

    let end_now = limits.end;
    let start_now = limits.start;

    let stop = SliderRow::new("Stop charging at", move |width| {
        // sliders are 0..=100 u8 percent, which is also the sysfs range
        Slider::new(end_now, width)
            .on_value_change(move |value| {
                // never below MIN_END: some firmware clamps to 50, and below that the
                // threshold stops being useful
                let end = value.max(MIN_END);
                // keep start strictly below end, with a 5-point gap so the kernel never
                // sees a window of zero
                let start = start_now.min(end.saturating_sub(5));
                tell(start, end);
            })
            .build()
    })
    .value(format!("{} %", limits.end));

    let end_capped = limits.end.saturating_sub(5);
    let start = SliderRow::new("Start charging below", move |width| {
        Slider::new(start_now, width)
            .on_value_change(move |value| {
                let start = value.min(end_capped);
                tell(start, end_now);
            })
            .build()
    })
    .value(format!("{} %", limits.start));

    let footer = match error {
        Some(error) => format!(
            "Couldn't set it: {error}. Lower limits keep the battery healthy longer; \
             Lenovo recommends stopping around 80 %."
        ),
        None => String::from(
            "Lower limits keep the battery healthy longer. Lenovo recommends stopping \
             around 80 %, and charging again at 75 %.",
        ),
    };

    group()
        .header("Battery charge")
        .slider(stop)
        .slider(start)
        .footer(footer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_min_end_is_raised() {
        // the slider cannot ask for below MIN_END (the max() clamp here mirrors the one
        // inside the on_value_change)
        let value: u8 = 30;
        assert_eq!(value.max(MIN_END), MIN_END);
    }

    #[test]
    fn start_stays_strictly_below_end() {
        let end: u8 = 80;
        assert_eq!(90_u8.min(end.saturating_sub(5)), 75);
        assert_eq!(10_u8.min(end.saturating_sub(5)), 10);
    }
}

