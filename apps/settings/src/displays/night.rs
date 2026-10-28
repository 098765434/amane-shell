//! Night Light on the Displays page: the shell runs it (`wlsunset`), so
//! this asks the running shell over IPC (`night-light state | on | off |
//! temperature | schedule`) and watches its state file to stay in step with
//! the Quick Settings tile.
//!
//! "Sunset to sunrise" needs where the machine is; it starts from the
//! time zone's own place in `zone.tab`, which is near enough for when the
//! sun sets.

use std::fs;
use std::path::PathBuf;
use std::thread;

use amane::{Center, Column, Row, Service, TextInput, ipc_call, watch_file};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Segment, SegmentedButton, Switch};
use amane_kit::layout::boxed;
use amane_kit::slider::Slider;
use amane_kit::tokens::sys;

use crate::{labelled, note, setting};

const SHELL: &str = "amane";

/// The slider's ends, as the shell clamps them.
const WARMEST: u32 = 2500;
const COOLEST: u32 = 6000;

#[derive(Clone, Debug, PartialEq)]
enum Schedule {
    Manual,
    Sun(f32, f32),
    Hours(String, String),
}

#[derive(Clone, Debug, PartialEq)]
struct Light {
    on: bool,
    kelvin: u32,
    schedule: Schedule,
}

/// The shell's night light; none while the shell doesn't answer or has
/// no `wlsunset`.
struct NightLight {
    light: Option<Light>,
}

impl Service for NightLight {
    fn new() -> Self {
        Self { light: ask() }
    }

    // the shell saves its state on every change, from here or the tile
    fn listen() {
        let path = state_file();
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        for _ in watch_file(&path.to_string_lossy()) {
            let light = ask();
            let mut now = Self::write();
            if now.light == light {
                now.quiet();
                continue;
            }
            now.light = light;
        }
    }
}

fn state_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_default()
        .join("amane-shell/night-light")
}

fn ask() -> Option<Light> {
    let answer = ipc_call(SHELL, "night-light", &[String::from("state")]).ok()?;
    parse(&answer)
}

/// The shell's answer: `on 4000 manual`, `off 3500 sun 59.91 10.75`,
/// `on 4000 hours 21:00 07:00`.
fn parse(text: &str) -> Option<Light> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let on = match *words.first()? {
        "on" => true,
        "off" => false,
        _ => return None,
    };
    let kelvin = words.get(1)?.parse().ok()?;
    let schedule = match words.get(2..).unwrap_or_default() {
        ["sun", lat, lon] => Schedule::Sun(lat.parse().ok()?, lon.parse().ok()?),
        ["hours", from, to] => Schedule::Hours(from.to_string(), to.to_string()),
        _ => Schedule::Manual,
    };
    Some(Light {
        on,
        kelvin,
        schedule,
    })
}

/// Asks the shell to run `args`, off the UI thread, then reads it again.
fn tell(args: Vec<String>) {
    thread::spawn(move || {
        let _ = ipc_call(SHELL, "night-light", &args);
        NightLight::write().light = ask();
    });
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

/// The time zone's place, from `zone.tab`: `+5955+01045` is 59°55′N 10°45′E.
fn zone_place() -> Option<(f32, f32)> {
    let zone = std::env::var("TZ")
        .ok()
        .map(|tz| tz.trim_start_matches(':').to_string())
        .filter(|tz| tz.contains('/'))
        .or_else(|| {
            let target = fs::canonicalize("/etc/localtime").ok()?;
            let target = target.to_string_lossy();
            Some(target.split_once("zoneinfo/")?.1.to_string())
        })?;
    let folder = std::env::var("TZDIR").unwrap_or_else(|_| String::from("/etc/zoneinfo"));
    let table = fs::read_to_string(PathBuf::from(folder).join("zone.tab")).ok()?;
    let line = table
        .lines()
        .find(|line| line.split('\t').nth(2) == Some(zone.as_str()))?;
    coordinates(line.split('\t').nth(1)?)
}

fn coordinates(text: &str) -> Option<(f32, f32)> {
    let split = text[1..].find(['+', '-'])? + 1;
    Some((angle(&text[..split], 2)?, angle(&text[split..], 3)?))
}

/// `±DDMM[SS]`, `degrees` digits of degrees.
fn angle(text: &str, degrees: usize) -> Option<f32> {
    let sign = if text.starts_with('-') { -1.0 } else { 1.0 };
    let digits = &text[1..];
    let whole: f32 = digits.get(..degrees)?.parse().ok()?;
    let minutes: f32 = digits.get(degrees..degrees + 2)?.parse().ok()?;
    let seconds: f32 = digits
        .get(degrees + 2..degrees + 4)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    Some(sign * (whole + minutes / 60.0 + seconds / 3600.0))
}

const LATITUDE: &str = "settings.displays.night.latitude";
const LONGITUDE: &str = "settings.displays.night.longitude";
const FROM: &str = "settings.displays.night.from";
const TO: &str = "settings.displays.night.to";

pub fn section() -> Vec<Box<dyn amane::Widget>> {
    let Some(light) = NightLight::read().light.clone() else {
        return vec![boxed(setting(
            "Night Light",
            Column::new(vec![
                boxed(note(
                    "The shell isn't answering, or wlsunset isn't installed, so Night Light \
                     can't be set here.",
                )),
                boxed(
                    PushButton::new("settings.displays.night.retry", "Try Again")
                        .on_press(|| {
                            thread::spawn(|| NightLight::write().light = ask());
                        })
                        .build(),
                ),
            ])
            .gap(sys::space::gap::s()),
        ))];
    };

    let warmth = (COOLEST - light.kelvin.clamp(WARMEST, COOLEST)) * 100 / (COOLEST - WARMEST);
    let temperature = Row::new(vec![
        boxed(
            Slider::new(warmth as u8, 240.0)
                .wheel(false)
                .dimmed(!light.on)
                .on_value_change(|value| {
                    let kelvin = COOLEST - (COOLEST - WARMEST) * u32::from(value) / 100;
                    tell(vec![String::from("temperature"), kelvin.to_string()]);
                })
                .build(),
        ),
        boxed(note(format!("{} K, warmer to the right", light.kelvin))),
    ])
    .gap(sys::space::gap::m())
    .align(Center);

    let at = match light.schedule {
        Schedule::Manual => 0,
        Schedule::Sun(..) => 1,
        Schedule::Hours(..) => 2,
    };
    let schedule = SegmentedButton::new(
        "settings.displays.night.schedule",
        ["All day", "Sunset to sunrise", "Set hours"]
            .into_iter()
            .map(Segment::new)
            .collect(),
        Some(at),
    )
    .on_select(|index, _| match index {
        0 => tell(words(&["schedule", "manual"])),
        1 => {
            let (lat, lon) = zone_place().unwrap_or((0.0, 0.0));
            tell(words(&[
                "schedule",
                "sun",
                &lat.to_string(),
                &lon.to_string(),
            ]));
        }
        _ => tell(words(&["schedule", "hours", "21:00", "07:00"])),
    })
    .build();

    let mut column: Vec<Box<dyn amane::Widget>> = vec![
        boxed(labelled(
            "Warm the screen's colours",
            Switch::new("settings.displays.night.on", light.on)
                .on_toggle(|on| tell(words(&[if on { "on" } else { "off" }])))
                .build(),
        )),
        boxed(temperature),
        boxed(schedule),
    ];
    match &light.schedule {
        Schedule::Manual => column.push(boxed(note("Warm all day while it's on."))),
        Schedule::Sun(lat, lon) => {
            column.push(boxed(pair(
                (LATITUDE, "Latitude", &format!("{lat:.2}")),
                (LONGITUDE, "Longitude", &format!("{lon:.2}")),
                |lat, lon| tell(words(&["schedule", "sun", lat, lon])),
            )));
            column.push(boxed(note(
                "Warm from sunset to sunrise here, the screen's own colours by day. The place \
                 starts at your time zone's.",
            )));
        }
        Schedule::Hours(from, to) => {
            column.push(boxed(pair(
                (FROM, "From", from),
                (TO, "To", to),
                |from, to| tell(words(&["schedule", "hours", from, to])),
            )));
            column.push(boxed(note(
                "Warm between these times, as HH:MM, overnight when To is earlier.",
            )));
        }
    }

    vec![boxed(setting(
        "Night Light",
        Column::new(column).gap(sys::space::gap::s()),
    ))]
}

/// Two short fields and a Set button that sends both.
fn pair(
    first: (&'static str, &str, &str),
    second: (&'static str, &str, &str),
    set: impl Fn(&str, &str) + Clone + 'static,
) -> Row {
    for (id, _, value) in [first, second] {
        if TextInput::text(id).is_empty() {
            TextInput::set_text(id, value);
        }
    }
    let (a, b) = (first.0, second.0);
    Row::new(vec![
        boxed(field_box(a, text_field(a, first.1), 120.0)),
        boxed(field_box(b, text_field(b, second.1), 120.0)),
        boxed(
            PushButton::new("settings.displays.night.set", "Set")
                .on_press(move || set(TextInput::text(a).trim(), TextInput::text(b).trim()))
                .build(),
        ),
    ])
    .gap(sys::space::gap::s())
    .align(Center)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shells_answer() {
        assert_eq!(
            parse("on 3500 hours 21:00 07:00\n"),
            Some(Light {
                on: true,
                kelvin: 3500,
                schedule: Schedule::Hours(String::from("21:00"), String::from("07:00")),
            })
        );
        assert_eq!(parse("off 4000").unwrap().schedule, Schedule::Manual);
        assert_eq!(parse("wlsunset isn't installed"), None);
    }

    #[test]
    fn zone_tab_places_read() {
        let (lat, lon) = coordinates("+5955+01045").unwrap();
        assert!((lat - 59.9167).abs() < 0.001 && (lon - 10.75).abs() < 0.001);
        let (lat, lon) = coordinates("-332052+1513745").unwrap();
        assert!((lat + 33.3478).abs() < 0.001 && (lon - 151.6292).abs() < 0.001);
    }
}
