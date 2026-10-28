//! Changes made outside the shell's keys: a script's `wpctl`, a headset's
//! buttons, the keyboard light's own key (a ThinkPad's Fn+Space is the
//! firmware's), a lock key, the sound moving to another device. Each view
//! of the display reads the services, so it runs as one of them changes,
//! and shows what differs from the last reading.
//!
//! Left out, on purpose: the screen's brightness (hypridle dims it on idle
//! with `brightnessctl`, which the display shouldn't answer; its keys come
//! through [`keys`](super::keys)), and the microphone's level (apps turn it
//! up and down themselves; only muting shows).
//!
//! Nothing shows for the first reading, a device that's gone or not found
//! yet, a key's own change (already shown), or, while Quick Settings is
//! open, anything its controls change (they're their own feedback).

use std::cell::RefCell;

use amane::{Audio, AudioKind, PowerProfile, Service};

use super::keys::{airplane_readout, just_pressed, profile_readout};
use super::{Readout, show};
use crate::components::icons::{ICON_CAPS_LOCK, ICON_NUM_LOCK, audio_glyph};
use crate::components::panel::{Panel, is_open};
use crate::services::desktop::lock_keys::LockKeys;
use crate::services::network::rfkill::Rfkill;
use crate::services::power::keyboard_backlight::KeyboardBacklight;

/// One reading of everything the display answers.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    volume: u8,
    muted: bool,
    kind: AudioKind,
    mic: u8,
    mic_muted: bool,
    // the default output's name and description, none while the sound
    // server is away
    output: Option<(String, String)>,
    // level and steps, none without a keyboard light
    light: Option<(u32, u32)>,
    caps: Option<bool>,
    num: Option<bool>,
    // none without radios
    airplane: Option<bool>,
    // none until the daemon answers
    profile: Option<String>,
}

thread_local! {
    static SEEN: RefCell<Option<Seen>> = const { RefCell::new(None) };
}

fn read() -> Seen {
    let audio = Audio::read();
    let output = audio
        .outputs()
        .iter()
        .find(|device| device.default())
        .map(|device| (device.name().to_string(), device.description().to_string()));
    let (volume, muted, kind) = (audio.volume(), audio.muted(), audio.output_kind());
    let (mic, mic_muted) = (audio.microphone_volume(), audio.microphone_muted());
    drop(audio);

    let keyboard = KeyboardBacklight::read();
    let light = keyboard
        .present()
        .then(|| (keyboard.level(), keyboard.max()));
    drop(keyboard);

    let locks = *LockKeys::read();

    let radios = Rfkill::read();
    let airplane = radios.present().then(|| radios.airplane());
    drop(radios);

    let profiles = PowerProfile::read();
    let profile = (profiles.present() && !profiles.active().is_empty())
        .then(|| profiles.active().to_string());
    drop(profiles);

    Seen {
        volume,
        muted,
        kind,
        mic,
        mic_muted,
        output,
        light,
        caps: locks.caps(),
        num: locks.num(),
        airplane,
        profile,
    }
}

/// Reads the services and shows what changed since the last reading.
pub(super) fn changes() {
    let now = read();
    let Some(before) = SEEN.replace(Some(now.clone())) else {
        return;
    };
    if let Some(readout) = changed(&before, &now, just_pressed(), is_open(Panel::QuickSettings)) {
        show(readout);
    }
}

/// What to show for the change from `before` to `now`, if anything: one
/// thing, the one that says most. `pressed` is a key's own change, already
/// shown; `controls` is Quick Settings open.
fn changed(before: &Seen, now: &Seen, pressed: bool, controls: bool) -> Option<Readout> {
    // a lock key is never anything else's doing
    if let Some(on) = switched(before.caps, now.caps) {
        return Some(lock("Caps Lock", ICON_CAPS_LOCK, on));
    }
    if let Some(on) = switched(before.num, now.num) {
        return Some(lock("Num Lock", ICON_NUM_LOCK, on));
    }
    if pressed || controls {
        return None;
    }

    // the sound server away, or back: its levels mean nothing yet
    let (Some(was), Some(output)) = (&before.output, &now.output) else {
        return None;
    };
    if was.0 != output.0 {
        return Some(Readout::Label {
            glyph: audio_glyph(now.kind, false),
            text: output.1.clone(),
        });
    }
    if (before.volume, before.muted) != (now.volume, now.muted) {
        return Some(Readout::Volume {
            level: now.volume,
            muted: now.muted,
            kind: now.kind,
        });
    }
    if before.mic_muted != now.mic_muted {
        return Some(Readout::Mic {
            level: now.mic,
            muted: now.mic_muted,
        });
    }
    if let (Some((was, steps)), Some((level, max))) = (before.light, now.light)
        && was != level
        && steps == max
    {
        return Some(Readout::KeyboardLight { level, max });
    }
    if let Some(on) = switched(before.airplane, now.airplane) {
        return Some(airplane_readout(on));
    }
    if let (Some(was), Some(profile)) = (&before.profile, &now.profile)
        && was != profile
    {
        return Some(profile_readout(profile));
    }
    None
}

/// The new state of a switch that was known and changed.
fn switched(before: Option<bool>, now: Option<bool>) -> Option<bool> {
    match (before, now) {
        (Some(was), Some(on)) if was != on => Some(on),
        _ => None,
    }
}

fn lock(name: &str, glyph: char, on: bool) -> Readout {
    Readout::Label {
        glyph,
        text: format!("{name} {}", if on { "On" } else { "Off" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen() -> Seen {
        Seen {
            volume: 40,
            muted: false,
            kind: AudioKind::Internal,
            mic: 70,
            mic_muted: false,
            output: Some((String::from("speakers"), String::from("Speakers"))),
            light: Some((0, 2)),
            caps: Some(false),
            num: Some(false),
            airplane: Some(false),
            profile: Some(String::from("balanced")),
        }
    }

    #[test]
    fn nothing_changed_shows_nothing() {
        assert_eq!(changed(&seen(), &seen(), false, false), None);
    }

    #[test]
    fn a_volume_change_from_elsewhere_shows() {
        let now = Seen {
            volume: 55,
            ..seen()
        };
        assert_eq!(
            changed(&seen(), &now, false, false),
            Some(Readout::Volume {
                level: 55,
                muted: false,
                kind: AudioKind::Internal
            })
        );
    }

    #[test]
    fn a_keys_own_change_and_quick_settings_stay_quiet() {
        let now = Seen {
            volume: 55,
            ..seen()
        };
        assert_eq!(changed(&seen(), &now, true, false), None);
        assert_eq!(changed(&seen(), &now, false, true), None);
    }

    #[test]
    fn lock_keys_show_even_while_quick_settings_is_open() {
        let now = Seen {
            caps: Some(true),
            ..seen()
        };
        assert_eq!(
            changed(&seen(), &now, true, true),
            Some(Readout::Label {
                glyph: ICON_CAPS_LOCK,
                text: String::from("Caps Lock On")
            })
        );
    }

    #[test]
    fn a_new_output_shows_its_name_not_its_level() {
        let now = Seen {
            volume: 80,
            kind: AudioKind::Headphones,
            output: Some((String::from("bluez"), String::from("AirPods"))),
            ..seen()
        };
        assert_eq!(
            changed(&seen(), &now, false, false),
            Some(Readout::Label {
                glyph: audio_glyph(AudioKind::Headphones, false),
                text: String::from("AirPods")
            })
        );
    }

    #[test]
    fn the_sound_server_coming_back_shows_nothing() {
        let away = Seen {
            output: None,
            volume: 0,
            ..seen()
        };
        assert_eq!(changed(&away, &seen(), false, false), None);
        assert_eq!(changed(&seen(), &away, false, false), None);
    }

    #[test]
    fn the_first_power_mode_shows_nothing() {
        let unknown = Seen {
            profile: None,
            ..seen()
        };
        assert_eq!(changed(&unknown, &seen(), false, false), None);
        let saver = Seen {
            profile: Some(String::from("power-saver")),
            ..seen()
        };
        assert!(changed(&seen(), &saver, false, false).is_some());
    }

    #[test]
    fn a_keyboard_light_found_late_shows_nothing() {
        let none = Seen {
            light: None,
            ..seen()
        };
        assert_eq!(changed(&none, &seen(), false, false), None);
        let lit = Seen {
            light: Some((2, 2)),
            ..seen()
        };
        assert_eq!(
            changed(&seen(), &lit, false, false),
            Some(Readout::KeyboardLight { level: 2, max: 2 })
        );
    }
}
