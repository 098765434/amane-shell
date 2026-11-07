//! Night Light — warms the screen's colours, by running `wlsunset`.
//!
//! The compositor gives one program at a time the screens' colour ramps
//! (wlr-gamma-control), so the shell owns the `wlsunset` that does it:
//! turning on stops any other one and starts its own, held at the chosen
//! temperature all day (its day and night temperatures a degree apart), and
//! turning off stops it, which gives the screens their colours back. A
//! `wlsunset` the compositor starts by itself would fight this one, so it
//! shouldn't (see `docs/shell/quick-settings.md`).
//!
//! A [`Schedule`] lets `wlsunset` do what it's for instead: warm from sunset
//! to sunrise where the machine is, or between two set times, and the
//! screens' own colours by day. Under a schedule, on means it follows it.
//!
//! Whether it's on, how warm and the schedule are kept in
//! `$XDG_STATE_HOME/amane-shell/night-light`, so the shell turns it back on
//! after a restart. Shell Settings changes them over IPC ([`ipc`]).

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use amane::Service;

use crate::services::{Latest, run_now, save_state, state_file};

const FILE: &str = "night-light";

/// The warmest and coolest the slider goes, in kelvin.
pub const WARMEST: u32 = 2500;
pub const COOLEST: u32 = 6000;
const DEFAULT: u32 = 4000;
/// The screens' own colour, by day under a schedule.
const DAY: u32 = 6500;

/// When it warms the screens.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Schedule {
    /// All day, while it's on.
    #[default]
    Manual,
    /// From sunset to sunrise at this latitude and longitude.
    Sun { latitude: f32, longitude: f32 },
    /// From `from` to `to`, hours and minutes, overnight if `to` is earlier.
    Hours { from: (u8, u8), to: (u8, u8) },
}

impl Schedule {
    /// `manual`, `sun 59.91 10.75` or `hours 21:00 07:00`.
    fn words(&self) -> String {
        match self {
            Schedule::Manual => String::from("manual"),
            Schedule::Sun {
                latitude,
                longitude,
            } => format!("sun {latitude} {longitude}"),
            Schedule::Hours { from, to } => format!("hours {} {}", clock(*from), clock(*to)),
        }
    }

    fn parse<'a>(mut words: impl Iterator<Item = &'a str>) -> Option<Self> {
        Some(match words.next() {
            None | Some("manual") => Schedule::Manual,
            Some("sun") => {
                let latitude: f32 = words.next()?.parse().ok()?;
                let longitude: f32 = words.next()?.parse().ok()?;
                if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
                    return None;
                }
                Schedule::Sun {
                    latitude,
                    longitude,
                }
            }
            Some("hours") => Schedule::Hours {
                from: parse_clock(words.next()?)?,
                to: parse_clock(words.next()?)?,
            },
            Some(_) => return None,
        })
    }
}

fn clock((hour, minute): (u8, u8)) -> String {
    format!("{hour:02}:{minute:02}")
}

fn parse_clock(text: &str) -> Option<(u8, u8)> {
    let (hour, minute) = text.split_once(':')?;
    let (hour, minute) = (hour.parse().ok()?, minute.parse().ok()?);
    (hour < 24 && minute < 60).then_some((hour, minute))
}

// one change at a time: a restart is a stop and a start
static APPLYING: Mutex<()> = Mutex::new(());
static TEMPERATURE: Latest<u32> = Latest::new();

pub struct NightLight {
    // what was asked for; shown at once, while `wlsunset` starts
    on: bool,
    temperature: u32,
    schedule: Schedule,
    child: Option<Child>,
    // turned back on after a restart, on the first poll
    restored: bool,
}

impl Service for NightLight {
    fn new() -> Self {
        let (on, temperature, schedule) = std::fs::read_to_string(state_file(FILE))
            .ok()
            .and_then(|text| parse(&text))
            .unwrap_or((false, DEFAULT, Schedule::Manual));
        Self {
            on,
            temperature,
            schedule,
            child: None,
            restored: !on,
        }
    }

    // to notice `wlsunset` dying under it
    fn interval() -> Duration {
        Duration::from_secs(5)
    }

    fn update(&mut self) -> bool {
        if !self.restored {
            self.restored = true;
            self.child = start(self.temperature, self.schedule);
            self.on = self.child.is_some();
            return !self.on;
        }

        let exited = self
            .child
            .as_mut()
            .is_some_and(|child| !matches!(child.try_wait(), Ok(None)));
        if exited {
            eprintln!("amane-shell: wlsunset exited; night light is off");
            self.child = None;
            self.on = false;
        }
        exited
    }
}

impl NightLight {
    pub fn present(&self) -> bool {
        crate::services::installed("wlsunset")
    }

    pub fn on(&self) -> bool {
        self.on
    }

    pub fn temperature(&self) -> u32 {
        self.temperature
    }

    pub fn schedule(&self) -> Schedule {
        self.schedule
    }

    pub fn set(on: bool) {
        std::thread::spawn(move || apply(Some(on), None, None));
    }

    /// Clamped to [`WARMEST`]…[`COOLEST`]; restarts `wlsunset` at it while on.
    pub fn set_temperature(kelvin: u32) {
        TEMPERATURE.send(kelvin.clamp(WARMEST, COOLEST), |kelvin| {
            apply(None, Some(kelvin), None)
        });
    }

    /// Restarts `wlsunset` on the new schedule while on.
    pub fn set_schedule(schedule: Schedule) {
        std::thread::spawn(move || apply(None, None, Some(schedule)));
    }
}

/// `night-light state | on | off | toggle | temperature <K> | schedule
/// manual | schedule sun <lat> <lon> | schedule hours <HH:MM> <HH:MM>`:
/// Shell Settings' Displays page. `state` answers like the state file.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let light = NightLight::read();
    if !light.present() {
        return String::from("wlsunset isn't installed");
    }
    let (on, kelvin, schedule) = (light.on, light.temperature, light.schedule);
    drop(light);
    match args.as_slice() {
        [] | ["state"] => return format(on, kelvin, schedule),
        ["on"] => NightLight::set(true),
        ["off"] => NightLight::set(false),
        ["toggle"] => NightLight::set(!on),
        ["temperature", kelvin] => match kelvin.parse() {
            Ok(kelvin) => NightLight::set_temperature(kelvin),
            Err(_) => return format!("not a temperature: {kelvin}"),
        },
        ["schedule", rest @ ..] => match Schedule::parse(rest.iter().copied()) {
            Some(schedule) => NightLight::set_schedule(schedule),
            None => {
                return String::from("schedule manual | sun <lat> <lon> | hours <HH:MM> <HH:MM>");
            }
        },
        _ => return String::from("night-light state | on | off | toggle | temperature | schedule"),
    }
    String::from("ok")
}

/// Stops the running `wlsunset` and starts what's asked for, with the
/// service unlocked meanwhile so the view can show the new state.
fn apply(on: Option<bool>, kelvin: Option<u32>, schedule: Option<Schedule>) {
    let _one = APPLYING.lock().unwrap_or_else(|e| e.into_inner());

    let (old, on, kelvin, schedule) = {
        let mut light = NightLight::write();
        let on = on.unwrap_or(light.on);
        let kelvin = kelvin.unwrap_or(light.temperature);
        let schedule = schedule.unwrap_or(light.schedule);
        let unchanged = on == light.on
            && kelvin == light.temperature
            && schedule == light.schedule
            && light.restored;
        if unchanged && (light.child.is_some() == on) {
            return;
        }
        light.on = on;
        light.temperature = kelvin;
        light.schedule = schedule;
        light.restored = true;
        (light.child.take(), on, kelvin, schedule)
    };

    stop(old);
    let child = if on { start(kelvin, schedule) } else { None };

    let mut light = NightLight::write();
    light.on = child.is_some();
    light.child = child;
    save_state(FILE, &format(light.on, kelvin, schedule));
}

/// What `wlsunset` is started with: `kelvin` all day, or by night on a
/// schedule.
fn arguments(kelvin: u32, schedule: Schedule) -> Vec<String> {
    let mut args = vec![String::from("-t"), kelvin.to_string(), String::from("-T")];
    match schedule {
        Schedule::Manual => args.extend([
            (kelvin + 1).to_string(),
            // any place will do: day and night are the same colour
            String::from("-l"),
            String::from("0"),
            String::from("-L"),
            String::from("0"),
        ]),
        Schedule::Sun {
            latitude,
            longitude,
        } => args.extend([
            DAY.to_string(),
            String::from("-l"),
            latitude.to_string(),
            String::from("-L"),
            longitude.to_string(),
        ]),
        Schedule::Hours { from, to } => args.extend([
            DAY.to_string(),
            String::from("-s"),
            clock(from),
            String::from("-S"),
            clock(to),
        ]),
    }
    args
}

/// `wlsunset` at `kelvin` on `schedule`, after stopping any other one;
/// `None` when it isn't installed.
fn start(kelvin: u32, schedule: Schedule) -> Option<Child> {
    // another wlsunset still holding the screens would make ours fail
    let _ = run_now("pkill", &["-x".into(), "wlsunset".into()]);
    std::thread::sleep(Duration::from_millis(100));

    let args = arguments(kelvin, schedule);
    Command::new("wlsunset")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|e| eprintln!("amane-shell: wlsunset: {e}"))
        .ok()
}

fn stop(child: Option<Child>) {
    if let Some(mut child) = child {
        let _ = child.kill();
        std::thread::spawn(move || child.wait());
    }
}

/// One line: `on 4000`, `off 4000 sun 59.91 10.75` or `on 3500 hours
/// 21:00 07:00`; a line without a schedule is from before there was one.
fn parse(text: &str) -> Option<(bool, u32, Schedule)> {
    let mut words = text.split_whitespace();
    let on = match words.next()? {
        "on" => true,
        "off" => false,
        _ => return None,
    };
    let kelvin = words.next()?.parse::<u32>().ok()?;
    let schedule = Schedule::parse(words).unwrap_or_default();
    Some((on, kelvin.clamp(WARMEST, COOLEST), schedule))
}

fn format(on: bool, kelvin: u32, schedule: Schedule) -> String {
    format!(
        "{} {kelvin} {}\n",
        if on { "on" } else { "off" },
        schedule.words()
    )
}

/// The slider's 0–100 for a temperature: warmer to the right.
pub fn slider_value(kelvin: u32) -> u8 {
    let span = COOLEST - WARMEST;
    let warmth = COOLEST.saturating_sub(kelvin.clamp(WARMEST, COOLEST));
    (warmth * 100 / span) as u8
}

/// The temperature for the slider's 0–100.
pub fn slider_kelvin(value: u8) -> u32 {
    let span = COOLEST - WARMEST;
    COOLEST - span * u32::from(value.min(100)) / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips() {
        for schedule in [
            Schedule::Manual,
            Schedule::Sun {
                latitude: 59.91,
                longitude: -10.75,
            },
            Schedule::Hours {
                from: (21, 30),
                to: (7, 0),
            },
        ] {
            assert_eq!(
                parse(&format(true, 3200, schedule)),
                Some((true, 3200, schedule))
            );
        }
        assert_eq!(
            parse(&format(false, 4000, Schedule::Manual)),
            Some((false, 4000, Schedule::Manual))
        );
    }

    #[test]
    fn a_bad_file_is_ignored_and_a_wild_temperature_clamped() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("maybe 4000"), None);
        assert_eq!(parse("on warm"), None);
        assert_eq!(parse("on 100"), Some((true, WARMEST, Schedule::Manual)));
        assert_eq!(parse("on 9000\n"), Some((true, COOLEST, Schedule::Manual)));
        // a schedule it can't read is dropped, not the rest
        assert_eq!(
            parse("on 4000 hours 25:00 07:00"),
            Some((true, 4000, Schedule::Manual))
        );
    }

    #[test]
    fn a_schedule_is_wlsunsets_night() {
        let hours = Schedule::Hours {
            from: (21, 0),
            to: (6, 30),
        };
        assert_eq!(
            arguments(3500, hours).join(" "),
            "-t 3500 -T 6500 -s 21:00 -S 06:30"
        );
        assert_eq!(
            arguments(4000, Schedule::Manual).join(" "),
            "-t 4000 -T 4001 -l 0 -L 0"
        );
    }

    #[test]
    fn the_slider_runs_from_cool_to_warm() {
        assert_eq!(slider_value(COOLEST), 0);
        assert_eq!(slider_value(WARMEST), 100);
        assert_eq!(slider_kelvin(0), COOLEST);
        assert_eq!(slider_kelvin(100), WARMEST);
        assert_eq!(slider_kelvin(slider_value(4250)), 4250);
    }
}
