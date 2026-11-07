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

use amane::{Center, Row, Service, TextInput, watch_file};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::PushButton;
use amane_kit::dropdown::Dropdown;
use amane_kit::form::{FormRow, Group, SliderRow};
use amane_kit::layout::boxed;
use amane_kit::shell_ipc::{NightLight as Ask, ShellCommand};
use amane_kit::slider::Slider;
use amane_kit::style;
use amane_kit::tokens::sys;

use crate::{group, switch_row};

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
    /// The shell has been asked once; until then the section says it's
    /// asking, not that Night Light is missing.
    asked: bool,
    /// What the last change ran into: a value that doesn't read, or the
    /// shell's refusal.
    error: Option<String>,
}

impl Service for NightLight {
    // asked from listen(), so opening the page never waits on the shell
    fn new() -> Self {
        Self {
            light: None,
            asked: false,
            error: None,
        }
    }

    // the shell saves its state on every change, from here or the tile
    fn listen() {
        let path = state_file();
        if let Some(folder) = path.parent() {
            // a write that follows says so if the folder couldn't be made
            let _ = fs::create_dir_all(folder);
        }
        {
            let light = ask();
            let mut now = Self::write();
            now.light = light;
            now.asked = true;
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
    amane_kit::paths::state().join("amane-shell/night-light.json")
}

fn ask() -> Option<Light> {
    let answer = ShellCommand::NightLight(Ask::State).send().ok()?;
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

/// Asks the shell for `ask`, off the UI thread, then reads it again.
fn tell(asked: Ask) {
    thread::spawn(move || {
        let error = ShellCommand::NightLight(asked)
            .send()
            .err()
            .map(|why| format!("The shell didn't take it: {why}"));
        let light = ask();
        let mut now = NightLight::write();
        now.light = light;
        now.error = error;
    });
}

/// Puts what went wrong under the section, or clears it.
fn refuse(error: Option<String>) {
    let mut now = NightLight::write();
    if now.error == error {
        now.quiet();
    } else {
        now.error = error;
    }
}

/// A latitude and longitude as typed, when they're a place on Earth.
fn place(lat: &str, lon: &str) -> Option<(f32, f32)> {
    let lat: f32 = lat.trim().parse().ok()?;
    let lon: f32 = lon.trim().parse().ok()?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

/// A time of day as HH:MM, written back with two digits each.
fn time(text: &str) -> Option<String> {
    let (hours, minutes) = text.trim().split_once(':')?;
    let hours: u8 = hours.parse().ok()?;
    let minutes: u8 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60).then(|| format!("{hours:02}:{minutes:02}"))
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

pub fn section() -> Group {
    let (light, asked, error) = {
        let now = NightLight::read();
        (now.light.clone(), now.asked, now.error.clone())
    };
    if !asked {
        return group().header("Night Light").row(FormRow::new(
            "settings.displays.night.asking",
            "Asking the shell…",
        ));
    }
    let Some(light) = light else {
        return group().header("Night Light").row(
            FormRow::new(
                "settings.displays.night.missing",
                "Night Light isn't available",
            )
            .subtitle("The shell isn't answering, or Night Light isn't installed (wlsunset)")
            .trailing(
                PushButton::new("settings.displays.night.retry", "Try Again")
                    .on_press(|| {
                        thread::spawn(|| {
                            let light = ask();
                            NightLight::write().light = light;
                        });
                    })
                    .build(),
            ),
        );
    };

    let warmth = (COOLEST - light.kelvin.clamp(WARMEST, COOLEST)) * 100 / (COOLEST - WARMEST);
    let on = light.on;
    let temperature = SliderRow::new("Color temperature", move |width| {
        Slider::new(warmth as u8, width)
            .wheel(false)
            .dimmed(!on)
            .on_value_change(|value| {
                let kelvin = COOLEST - (COOLEST - WARMEST) * u32::from(value) / 100;
                tell(Ask::Temperature(kelvin));
            })
            .build()
    })
    .value(format!("{} K, warmer to the right", light.kelvin));

    let at = match light.schedule {
        Schedule::Manual => 0,
        Schedule::Sun(..) => 1,
        Schedule::Hours(..) => 2,
    };
    // choosing the schedule already in use keeps its place or hours
    let schedule = Dropdown::new(
        "settings.displays.night.schedule",
        ["All day", "Sunset to sunrise", "Set hours"]
            .map(String::from)
            .to_vec(),
        at,
    )
    .width(200.0)
    .on_select(move |index| match index {
        _ if index == at => {}
        0 => tell(Ask::Schedule(words(&["manual"]))),
        1 => {
            let (lat, lon) = zone_place().unwrap_or((0.0, 0.0));
            tell(Ask::Schedule(words(&[
                "sun",
                &lat.to_string(),
                &lon.to_string(),
            ])));
        }
        _ => tell(Ask::Schedule(words(&["hours", "21:00", "07:00"]))),
    })
    .build();

    let group = group()
        .header("Night Light")
        .row(switch_row(
            "settings.displays.night.on",
            "Warm the screen's colors",
            light.on,
            |on| tell(if on { Ask::On } else { Ask::Off }),
        ))
        .slider(temperature)
        .row(FormRow::new("settings.displays.night.schedule-row", "Schedule").trailing(schedule));
    let error = error.map(|why| format!("{why}\n")).unwrap_or_default();
    match &light.schedule {
        Schedule::Manual => group.footer(format!("{error}Warm all day while it's on.")),
        Schedule::Sun(lat, lon) => group
            .row(
                FormRow::new("settings.displays.night.place", "Place").detail(
                    pair(
                        (LATITUDE, "Latitude", &format!("{lat:.2}")),
                        (LONGITUDE, "Longitude", &format!("{lon:.2}")),
                        |lat, lon| match place(lat, lon) {
                            Some((lat, lon)) => {
                                refuse(None);
                                tell(Ask::Schedule(words(&["sun", &lat.to_string(), &lon.to_string()])));
                            }
                            None => refuse(Some(String::from(
                                "A latitude is between -90 and 90, a longitude between -180 and 180.",
                            ))),
                        },
                    ),
                    style::control_height(),
                ),
            )
            .footer(format!(
                "{error}Warm from sunset to sunrise here, the screen's own colors by day. The \
                 place starts at your time zone's."
            )),
        Schedule::Hours(from, to) => group
            .row(
                FormRow::new("settings.displays.night.hours", "Hours").detail(
                    pair((FROM, "From", from), (TO, "To", to), |from, to| {
                        match (time(from), time(to)) {
                            (Some(from), Some(to)) => {
                                refuse(None);
                                tell(Ask::Schedule(words(&["hours", &from, &to])));
                            }
                            _ => refuse(Some(String::from("Times read like 21:00 or 07:30."))),
                        }
                    }),
                    style::control_height(),
                ),
            )
            .footer(format!(
                "{error}Warm between these times, overnight when To is earlier."
            )),
    }
}

/// Two short labelled fields and a Set button that sends both; Return in
/// either sends them too.
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
    let label = |text: &str| {
        boxed({
            use amane_kit::tokens::Styled;
            amane::Text::new(text)
                .style(style::control_font())
                .color(style::label_secondary())
        })
    };
    let send = set.clone();
    let submit = move |_: String| send(TextInput::text(a).trim(), TextInput::text(b).trim());
    let submit2 = submit.clone();
    Row::new(vec![
        label(first.1),
        boxed(field_box(a, text_field(a, first.1).on_submit(submit), 90.0)),
        label(second.1),
        boxed(field_box(
            b,
            text_field(b, second.1).on_submit(submit2),
            90.0,
        )),
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
    fn places_and_times_are_checked() {
        assert_eq!(place("59.9", "10.75"), Some((59.9, 10.75)));
        assert_eq!(place("91", "0"), None);
        assert_eq!(place("north", "0"), None);
        assert_eq!(time("7:5").as_deref(), Some("07:05"));
        assert_eq!(time("24:00"), None);
        assert_eq!(time("2100"), None);
    }

    #[test]
    fn zone_tab_places_read() {
        let (lat, lon) = coordinates("+5955+01045").unwrap();
        assert!((lat - 59.9167).abs() < 0.001 && (lon - 10.75).abs() < 0.001);
        let (lat, lon) = coordinates("-332052+1513745").unwrap();
        assert!((lat + 33.3478).abs() < 0.001 && (lon - 151.6292).abs() < 0.001);
    }
}
