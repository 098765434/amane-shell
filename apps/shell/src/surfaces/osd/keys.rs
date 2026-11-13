//! The keys' actions, over IPC: each changes a level or a switch and shows
//! the new one at once, from the value it set rather than waiting for the
//! service to report it (screen and keyboard light are polled, so that
//! could take half a second). niri binds the keys to these, with the plain
//! commands as a fallback for when the shell isn't running:
//!
//! ```text
//! printf 'osd\nvolume\nup\n' | nc -U -N $XDG_RUNTIME_DIR/emaki.sock
//! ```

use std::cell::Cell;
use std::time::{Duration, Instant};

use amane::{Audio, Brightness, PowerProfile, Service};

use super::{Readout, show};
use crate::services::display::hdr_white;
use crate::services::network::rfkill::{self, Rfkill};
use crate::services::power::keyboard_backlight::KeyboardBacklight;
use crate::surfaces::launcher::profile_name;
use emaki_kit::icons::{ICON_AIRPLANE, power_profile_glyph};
use emaki_kit::shell_ipc::{self, Osd};
use emaki_kit::slider::STEP;

/// How long a level a key set outranks what its service still says, and
/// how long the services' own reports of it are left unshown.
const SETTLE: Duration = Duration::from_millis(1000);

const USAGE: &str = "usage: osd volume up|down|mute [fine] | mic mute \
    | brightness up|down [fine] | brightness dim <percent> | brightness restore | kbd-light up|down|cycle | airplane toggle \
    | profile cycle | show volume|mic|brightness|kbd-light";

thread_local! {
    // the last level each key set, and when
    static VOLUME: Cell<Option<(u8, Instant)>> = const { Cell::new(None) };
    static BRIGHTNESS: Cell<Option<(u8, Instant)>> = const { Cell::new(None) };
    static LIGHT: Cell<Option<(u32, Instant)>> = const { Cell::new(None) };

    // the brightness idle dimmed from, until it's put back
    static UNDIMMED: Cell<Option<u8>> = const { Cell::new(None) };

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

pub fn run(command: Osd) -> String {
    let step = |up: bool, fine: bool| {
        let step = i16::from(if fine { 1 } else { STEP });
        if up { step } else { -step }
    };
    match command {
        Osd::Volume { up, fine } => volume(step(up, fine)),
        Osd::Mute => mute(),
        Osd::MicMute => mic_mute(),
        Osd::Brightness { up, fine } => brightness(step(up, fine)),
        Osd::Dim(percent) => dim(percent),
        Osd::Restore => restore(),
        Osd::KeyboardLight(shell_ipc::Light::Up) => light(Light::Up),
        Osd::KeyboardLight(shell_ipc::Light::Down) => light(Light::Down),
        Osd::KeyboardLight(shell_ipc::Light::Cycle) => light(Light::Cycle),
        Osd::Airplane => airplane(),
        Osd::Profile => profile(),
        Osd::Show(what) => return show_current(&what),
    }
    String::from("ok")
}

/// One step of the volume up or down, as its key does.
pub fn step_volume(up: bool) {
    volume(if up {
        i16::from(STEP)
    } else {
        -i16::from(STEP)
    });
}

/// One step of the screen's brightness up or down, as its key does.
pub fn step_brightness(up: bool) {
    brightness(if up {
        i16::from(STEP)
    } else {
        -i16::from(STEP)
    });
}

/// Mutes or unmutes the sound, as its key does.
pub fn toggle_mute() {
    mute();
}

/// Sets the volume to `level`, showing it, as the keys would get there.
pub fn set_volume(level: u8) {
    let now = latest(&VOLUME, Audio::read().volume());
    volume(i16::from(level.min(100)) - i16::from(now));
}

/// Sets the screen's brightness to `level`, showing it.
pub fn set_brightness(level: u8) {
    let now = latest(&BRIGHTNESS, Brightness::read().percent());
    brightness(i16::from(level.min(100)) - i16::from(now));
}

/// Moves the volume by `by`, unmuting as it goes up.
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

/*
 * idle's dimming (hypridle's dim listener): quietly, with no display,
 * and through amane's Brightness so it dims in HDR too, where the
 * backlight doesn't. never brighter than it was
 */
fn dim(percent: u8) {
    let screen = Brightness::read();
    let (present, now) = (screen.present(), screen.percent());
    drop(screen);
    if !present || now <= percent {
        return;
    }
    if UNDIMMED.get().is_none() {
        UNDIMMED.set(Some(now));
    }
    hdr_white::dimmed(true);
    Brightness::set(percent.max(1));
}

fn restore() {
    if let Some(level) = UNDIMMED.take() {
        Brightness::set(level);
    }
    // once the level is back, or the dim one could still be read as rested
    amane::Timer::after(Duration::from_secs(3), || hdr_white::dimmed(false));
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

    rfkill::set_airplane(!on);
    pressed();
    show(airplane_readout(!on));
}

/// The next power mode, round the ones the daemon offers; on battery
/// without Performance, so the key can't land there by going round.
fn profile() {
    let profiles = PowerProfile::read();
    let mut available = profiles.available().to_vec();
    if crate::services::power::battery_mode::on_battery() {
        available.retain(|p| p != "performance");
    }
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
