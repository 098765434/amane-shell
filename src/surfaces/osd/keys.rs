//! The keys' actions, over IPC: each changes a level or a switch and shows
//! the new one at once, from the value it set rather than waiting for the
//! service to report it (screen and keyboard light are polled, so that
//! could take half a second). niri binds the keys to these, with the plain
//! commands as a fallback for when the shell isn't running:
//!
//! ```text
//! printf 'osd\nvolume\nup\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
//! ```

use std::cell::Cell;
use std::time::{Duration, Instant};

use amane::{Audio, Brightness, PowerProfile, Service};

use super::{Readout, show};
use crate::components::icons::{ICON_AIRPLANE, power_profile_glyph};
use crate::components::slider::STEP;
use crate::services::network::rfkill::Rfkill;
use crate::services::power::keyboard_backlight::KeyboardBacklight;
use crate::surfaces::launcher::profile_name;

/// How long a level a key set outranks what its service still says, and
/// how long the services' own reports of it are left unshown.
const SETTLE: Duration = Duration::from_millis(1000);

const USAGE: &str = "usage: osd volume up|down|mute [fine] | mic mute \
    | brightness up|down [fine] | kbd-light up|down|cycle | airplane toggle \
    | profile cycle | show volume|mic|brightness|kbd-light";

thread_local! {
    // the last level each key set, and when
    static VOLUME: Cell<Option<(u8, Instant)>> = const { Cell::new(None) };
    static BRIGHTNESS: Cell<Option<(u8, Instant)>> = const { Cell::new(None) };
    static LIGHT: Cell<Option<(u32, Instant)>> = const { Cell::new(None) };

    // when a key last changed anything
    static PRESSED: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Whether a key changed something a moment ago, so the change the
/// services report now is that one, already shown.
pub(super) fn just_pressed() -> bool {
    PRESSED.get().is_some_and(|at| at.elapsed() < SETTLE)
}

/// What `key` last set, while it's fresher than the service's `reported`.
fn latest<T: Copy>(
    key: &'static std::thread::LocalKey<Cell<Option<(T, Instant)>>>,
    reported: T,
) -> T {
    match key.get() {
        Some((value, at)) if at.elapsed() < SETTLE => value,
        _ => reported,
    }
}

fn pressed() {
    PRESSED.set(Some(Instant::now()));
}

pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let fine = args.last() == Some(&"fine");
    let step = if fine { 1 } else { STEP };

    match args.as_slice() {
        ["volume", "up", ..] => volume(i16::from(step)),
        ["volume", "down", ..] => volume(-i16::from(step)),
        ["volume", "mute", ..] => mute(),
        ["mic", "mute"] => mic_mute(),
        ["brightness", "up", ..] => brightness(i16::from(step)),
        ["brightness", "down", ..] => brightness(-i16::from(step)),
        ["kbd-light", "up"] => light(Light::Up),
        ["kbd-light", "down"] => light(Light::Down),
        ["kbd-light", "cycle"] => light(Light::Cycle),
        ["airplane", "toggle"] => airplane(),
        ["profile", "cycle"] => profile(),
        ["show", what] => return show_current(what),
        _ => return String::from(USAGE),
    }
    String::from("ok")
}

/// Moves the volume by `by`, unmuting as it goes up (as macOS does).
fn volume(by: i16) {
    let audio = Audio::read();
    let (now, muted, kind) = (audio.volume(), audio.muted(), audio.output_kind());
    drop(audio);

    let level = (i16::from(latest(&VOLUME, now)) + by).clamp(0, 100) as u8;
    let unmute = muted && by > 0;
    Audio::set_volume(level);
    if unmute {
        Audio::toggle_mute();
    }
    VOLUME.set(Some((level, Instant::now())));
    pressed();
    show(Readout::Volume {
        level,
        muted: muted && !unmute,
        kind,
    });
}

fn mute() {
    let audio = Audio::read();
    let (level, muted, kind) = (audio.volume(), audio.muted(), audio.output_kind());
    drop(audio);

    Audio::toggle_mute();
    pressed();
    show(Readout::Volume {
        level: latest(&VOLUME, level),
        muted: !muted,
        kind,
    });
}

fn mic_mute() {
    let audio = Audio::read();
    let (level, muted) = (audio.microphone_volume(), audio.microphone_muted());
    drop(audio);

    Audio::toggle_microphone_mute();
    pressed();
    show(Readout::Mic {
        level,
        muted: !muted,
    });
}

/// Moves the screen's brightness by `by`; keys never take it to black.
fn brightness(by: i16) {
    let screen = Brightness::read();
    let (present, now) = (screen.present(), screen.percent());
    drop(screen);
    if !present {
        return;
    }

    let level = (i16::from(latest(&BRIGHTNESS, now)) + by).clamp(1, 100) as u8;
    Brightness::set(level);
    BRIGHTNESS.set(Some((level, Instant::now())));
    pressed();
    show(Readout::Brightness(level));
}

enum Light {
    Up,
    Down,
    // up a step, and from the brightest back to off
    Cycle,
}

fn light(change: Light) {
    let keyboard = KeyboardBacklight::read();
    let (present, now, max) = (keyboard.present(), keyboard.level(), keyboard.max());
    drop(keyboard);
    if !present {
        return;
    }

    let now = latest(&LIGHT, now);
    let level = match change {
        Light::Up => (now + 1).min(max),
        Light::Down => now.saturating_sub(1),
        Light::Cycle if now >= max => 0,
        Light::Cycle => now + 1,
    };
    KeyboardBacklight::set(level);
    LIGHT.set(Some((level, Instant::now())));
    pressed();
    show(Readout::KeyboardLight { level, max });
}

fn airplane() {
    let radios = Rfkill::read();
    let (present, on) = (radios.present(), radios.airplane());
    drop(radios);
    if !present {
        return;
    }

    Rfkill::set_airplane(!on);
    pressed();
    show(airplane_readout(!on));
}

/// The next power mode, round the ones the daemon offers.
fn profile() {
    let profiles = PowerProfile::read();
    let available = profiles.available().to_vec();
    let active = profiles.active().to_string();
    drop(profiles);
    if available.is_empty() {
        return;
    }

    let at = available.iter().position(|p| *p == active).unwrap_or(0);
    let next = available[(at + 1) % available.len()].clone();
    PowerProfile::set(&next);
    pressed();
    show(profile_readout(&next));
}

/// Shows the level of `what` as it is, changing nothing, for scripts.
fn show_current(what: &str) -> String {
    let readout = match what {
        "volume" => {
            let audio = Audio::read();
            Readout::Volume {
                level: audio.volume(),
                muted: audio.muted(),
                kind: audio.output_kind(),
            }
        }
        "mic" => {
            let audio = Audio::read();
            Readout::Mic {
                level: audio.microphone_volume(),
                muted: audio.microphone_muted(),
            }
        }
        "brightness" => Readout::Brightness(Brightness::read().percent()),
        "kbd-light" => {
            let keyboard = KeyboardBacklight::read();
            Readout::KeyboardLight {
                level: keyboard.level(),
                max: keyboard.max(),
            }
        }
        _ => return String::from(USAGE),
    };
    show(readout);
    String::from("ok")
}

pub(super) fn airplane_readout(on: bool) -> Readout {
    Readout::Label {
        glyph: ICON_AIRPLANE,
        text: String::from(if on {
            "Airplane Mode On"
        } else {
            "Airplane Mode Off"
        }),
    }
}

pub(super) fn profile_readout(profile: &str) -> Readout {
    Readout::Label {
        glyph: power_profile_glyph(profile),
        text: profile_name(profile),
    }
}
