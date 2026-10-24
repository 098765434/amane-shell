//! Night Light — warms the screen's colours, by running `wlsunset`.
//!
//! The compositor gives one program at a time the screens' colour ramps
//! (wlr-gamma-control), so the shell owns the `wlsunset` that does it:
//! turning on stops any other one and starts its own, held at the chosen
//! temperature all day (its day and night temperatures a degree apart), and
//! turning off stops it, which gives the screens their colours back. A
//! `wlsunset` the compositor starts by itself would fight this one, so it
//! shouldn't (see `docs/quick-settings.md`).
//!
//! Whether it's on and how warm are kept in `$XDG_STATE_HOME/amane-shell/
//! night-light`, so the shell turns it back on after a restart.

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use amane::Service;

use super::{Latest, run_now, save_state, state_file};

const FILE: &str = "night-light";

/// The warmest and coolest the slider goes, in kelvin.
pub const WARMEST: u32 = 2500;
pub const COOLEST: u32 = 6000;
const DEFAULT: u32 = 4000;

// one change at a time: a restart is a stop and a start
static APPLYING: Mutex<()> = Mutex::new(());
static TEMPERATURE: Latest<u32> = Latest::new();

pub struct NightLight {
    // what was asked for; shown at once, while `wlsunset` starts
    on: bool,
    temperature: u32,
    child: Option<Child>,
    // turned back on after a restart, on the first poll
    restored: bool,
}

impl Service for NightLight {
    fn new() -> Self {
        let (on, temperature) = std::fs::read_to_string(state_file(FILE))
            .ok()
            .and_then(|text| parse(&text))
            .unwrap_or((false, DEFAULT));
        Self {
            on,
            temperature,
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
            self.child = start(self.temperature);
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
        super::installed("wlsunset")
    }

    pub fn on(&self) -> bool {
        self.on
    }

    pub fn temperature(&self) -> u32 {
        self.temperature
    }

    pub fn set(on: bool) {
        std::thread::spawn(move || apply(Some(on), None));
    }

    /// Clamped to [`WARMEST`]…[`COOLEST`]; restarts `wlsunset` at it while on.
    pub fn set_temperature(kelvin: u32) {
        TEMPERATURE.send(kelvin.clamp(WARMEST, COOLEST), |kelvin| {
            apply(None, Some(kelvin))
        });
    }
}

/// Stops the running `wlsunset` and starts what's asked for, with the
/// service unlocked meanwhile so the view can show the new state.
fn apply(on: Option<bool>, kelvin: Option<u32>) {
    let _one = APPLYING.lock().unwrap_or_else(|e| e.into_inner());

    let (old, on, kelvin) = {
        let mut light = NightLight::write();
        let on = on.unwrap_or(light.on);
        let kelvin = kelvin.unwrap_or(light.temperature);
        let unchanged = on == light.on && kelvin == light.temperature && light.restored;
        if unchanged && (light.child.is_some() == on) {
            return;
        }
        light.on = on;
        light.temperature = kelvin;
        light.restored = true;
        (light.child.take(), on, kelvin)
    };

    stop(old);
    let child = if on { start(kelvin) } else { None };

    let mut light = NightLight::write();
    light.on = child.is_some();
    light.child = child;
    save_state(FILE, &format(light.on, kelvin));
}

/// `wlsunset` held at `kelvin`, after stopping any other one; `None` when
/// it isn't installed.
fn start(kelvin: u32) -> Option<Child> {
    // another wlsunset still holding the screens would make ours fail
    let _ = run_now("pkill", &["-x".into(), "wlsunset".into()]);
    std::thread::sleep(Duration::from_millis(100));

    let args = [
        "-t".to_string(),
        kelvin.to_string(),
        "-T".to_string(),
        (kelvin + 1).to_string(),
        // any place will do: day and night are the same colour
        "-l".to_string(),
        "0".to_string(),
        "-L".to_string(),
        "0".to_string(),
    ];
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

/// One line: `on 4000` or `off 4000`.
fn parse(text: &str) -> Option<(bool, u32)> {
    let mut words = text.split_whitespace();
    let on = match words.next()? {
        "on" => true,
        "off" => false,
        _ => return None,
    };
    let kelvin = words.next()?.parse::<u32>().ok()?;
    Some((on, kelvin.clamp(WARMEST, COOLEST)))
}

fn format(on: bool, kelvin: u32) -> String {
    format!("{} {kelvin}\n", if on { "on" } else { "off" })
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
        assert_eq!(parse(&format(true, 3200)), Some((true, 3200)));
        assert_eq!(parse(&format(false, 4000)), Some((false, 4000)));
    }

    #[test]
    fn a_bad_file_is_ignored_and_a_wild_temperature_clamped() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("maybe 4000"), None);
        assert_eq!(parse("on warm"), None);
        assert_eq!(parse("on 100"), Some((true, WARMEST)));
        assert_eq!(parse("on 9000\n"), Some((true, COOLEST)));
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
