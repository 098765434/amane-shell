//! Sound: the output and input, their levels and the output's balance, a
//! level meter for the microphone, and what each app is playing. The sound
//! a notification plays ([`alerts`]) is drawn on Notifications.
//!
//! The devices, levels and apps are the sound server's own, through amane's
//! `Audio`, so a change shows wherever else they're set (Quick Settings,
//! the volume keys, pavucontrol). The notification sound is `sound.json`,
//! which the shell plays from.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use amane::{Audio, AudioLevel, AudioStream, Center, Column, Row, Service};
use emaki_kit::buttons::{IconButton, PushButton};
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::{FormRow, SliderRow};
use emaki_kit::icons::{
    ICON_MIC, ICON_MIC_OFF, ICON_PLAY, ICON_VOLUME_OFF, ICON_VOLUME_UP, audio_glyph,
};
use emaki_kit::layout::boxed;
use emaki_kit::settings::{change_sound, sound};
use emaki_kit::slider::Slider;
use emaki_kit::spinner::ProgressBar;
use emaki_kit::{app_icon, sounds};

use crate::system::apps::icon_side;
use crate::{group, groups, switch_row};

/// What plays when the sound is turned on again.
const DEFAULT_SOUND: &str = "message-new-instant";

/// A mute button's width beside its slider.
const MUTE: f32 = 36.0;

pub fn page() -> Column {
    let audio = Audio::read();
    let outputs = audio.outputs().to_vec();
    let inputs = audio.inputs().to_vec();
    let (volume, muted) = (audio.volume(), audio.muted());
    let (mic, mic_muted) = (audio.microphone_volume(), audio.microphone_muted());
    let balance = audio.balance();
    let streams = audio.streams().to_vec();
    let (output_kind, input_kind) = (audio.output_kind(), audio.input_kind());
    drop(audio);

    let mut output = group().header("Output");
    if outputs.is_empty() {
        output = output.row(crate::empty_row(
            "settings.sound.output.none",
            "No speakers or headphones are connected",
        ));
    } else {
        let names: Vec<String> = outputs.iter().map(|d| d.name().to_string()).collect();
        let now = outputs.iter().position(|d| d.default()).unwrap_or(0);
        let menu = Dropdown::choice(
            "settings.sound.output.device.menu",
            outputs.iter().map(|d| d.description().to_string()).zip(0..),
            now,
            move |index: usize| {
                if let Some(name) = names.get(index) {
                    Audio::set_output(name);
                }
            },
        )
        .width(crate::MENU_W_WIDE)
        .build();
        output = output
            .row(
                FormRow::new("settings.sound.output.device", "Output device")
                    .icon(
                        audio_glyph(output_kind, false),
                        emaki_kit::color::Accent::Pink,
                    )
                    .trailing(menu),
            )
            .slider(level_row(
                "settings.sound.output.volume",
                "Volume",
                (volume, muted),
                (ICON_VOLUME_UP, ICON_VOLUME_OFF),
                Audio::set_volume,
                Audio::toggle_mute,
            ));
        if let Some(balance) = balance {
            output = output
                .slider(
                    SliderRow::new("Balance", move |width| {
                        Slider::new(balance_value(balance), width)
                            .on_value_change(|value| Audio::set_balance(balance_of(value)))
                            .build()
                    })
                    .value(balance_label(balance)),
                )
                .row(
                    FormRow::new("settings.sound.output.balance", "Test the balance")
                        .trailing(test_buttons()),
                );
        }
    }

    let mut input = group().header("Input");
    if inputs.is_empty() {
        input = input.row(crate::empty_row(
            "settings.sound.input.none",
            "No microphone is connected",
        ));
    } else {
        let names: Vec<String> = inputs.iter().map(|d| d.name().to_string()).collect();
        let now = inputs.iter().position(|d| d.default()).unwrap_or(0);
        let menu = Dropdown::choice(
            "settings.sound.input.device.menu",
            inputs.iter().map(|d| d.description().to_string()).zip(0..),
            now,
            move |index: usize| {
                if let Some(name) = names.get(index) {
                    Audio::set_input(name);
                }
            },
        )
        .width(crate::MENU_W_WIDE)
        .build();
        // read each frame, which keeps the meter recording while the page shows
        let peak = AudioLevel::read().peak();
        input = input
            .row(
                FormRow::new("settings.sound.input.device", "Input device")
                    .icon(
                        audio_glyph(input_kind, true),
                        emaki_kit::color::Accent::Pink,
                    )
                    .trailing(menu),
            )
            .slider(level_row(
                "settings.sound.input.volume",
                "Input volume",
                (mic, mic_muted),
                (ICON_MIC, ICON_MIC_OFF),
                Audio::set_microphone_volume,
                Audio::toggle_microphone_mute,
            ))
            .row(
                FormRow::new("settings.sound.input.level", "Input level")
                    .trailing(ProgressBar::new(meter(peak), crate::MENU_W).build()),
            )
            .footer("The level shows only while this page is open.");
    }

    let mut apps = group().header("Apps");
    if streams.is_empty() {
        apps = apps.row(crate::empty_row(
            "settings.sound.apps.none",
            "Apps playing sound show here",
        ));
    } else {
        for stream in &streams {
            apps = apps.slider(app_row(stream));
        }
        apps = apps.footer("Each app's level is a share of the output volume.");
    }

    groups(vec![output, input, apps])
}

/// A level with a mute button before its slider.
fn level_row(
    id: &'static str,
    title: &str,
    (volume, muted): (u8, bool),
    (glyph, muted_glyph): (char, char),
    set: fn(u8),
    toggle: fn(),
) -> SliderRow {
    SliderRow::new(title, move |width| {
        muted_slider(
            id,
            volume,
            muted,
            width,
            if muted { muted_glyph } else { glyph },
            set,
            toggle,
        )
    })
    .value(percent(volume, muted))
}

fn muted_slider(
    id: &str,
    volume: u8,
    muted: bool,
    width: f32,
    glyph: char,
    set: impl Fn(u8) + 'static,
    toggle: impl Fn() + 'static,
) -> Row {
    Row::new(vec![
        boxed(
            IconButton::new(format!("{id}.mute"), glyph)
                .tooltip(crate::WINDOW, if muted { "Unmute" } else { "Mute" })
                .selected(muted)
                .width(MUTE)
                .on_press(toggle)
                .build(),
        ),
        boxed(
            Slider::new(volume, (width - MUTE - 4.0).max(40.0))
                .dimmed(muted)
                .on_value_change(set)
                .build(),
        ),
    ])
    .gap(emaki_kit::tokens::sys::space::gap::xs())
    .align(Center)
}

fn app_row(stream: &AudioStream) -> SliderRow {
    let index = stream.index();
    let name = if stream.app().is_empty() {
        stream.binary().to_string()
    } else {
        stream.app().to_string()
    };
    let key = app_icon::first_known(&[stream.icon(), stream.binary(), stream.app()]).to_string();
    let (volume, muted) = (stream.volume(), stream.muted());
    let id = format!("settings.sound.apps.{index}");
    SliderRow::new(name.clone(), move |width| {
        let glyph = if muted {
            ICON_VOLUME_OFF
        } else {
            ICON_VOLUME_UP
        };
        muted_slider(
            &id,
            volume,
            muted,
            width,
            glyph,
            move |value| Audio::set_stream_volume(index, value),
            move || Audio::toggle_stream_mute(index),
        )
    })
    .leading(app_icon::tile(&key, &name, icon_side()))
    .value(if stream.paused() {
        String::from("Paused")
    } else {
        percent(volume, muted)
    })
}

fn test_buttons() -> Row {
    let test = |id: &'static str, label: &str, sound: &'static str| {
        PushButton::new(id, label)
            .icon(ICON_PLAY)
            .on_press(move || sounds::play(sound, 80, Duration::ZERO))
            .build()
    };
    Row::new(vec![
        boxed(test(
            "settings.sound.output.balance.left",
            "Left",
            "audio-channel-front-left",
        )),
        boxed(test(
            "settings.sound.output.balance.right",
            "Right",
            "audio-channel-front-right",
        )),
    ])
    .gap(emaki_kit::tokens::sys::space::gap::m())
    .align(Center)
}

/// The notification sound, on Notifications: whether one plays, which, and how loud.
pub fn alerts() -> emaki_kit::form::Group {
    let now = sound().clone();
    let on = now.notification.is_some();
    let chosen = now
        .notification
        .clone()
        .unwrap_or_else(|| String::from(DEFAULT_SOUND));
    let theme = theme();
    let alerts = group().header("Sound").row(switch_row(
        "settings.sound.notifications",
        "Play a sound for notifications",
        on,
        |on| {
            change_sound(|now| {
                now.notification = on.then(|| String::from(DEFAULT_SOUND));
            });
        },
    ));
    // the sound and its volume stay in sight, dimmed while it's off
    let volume = now.volume;
    let names: Vec<String> = theme.iter().map(|(name, _)| name.clone()).collect();
    let at = names.iter().position(|name| *name == chosen).unwrap_or(0);
    let menu = Dropdown::choice(
        "settings.sound.notifications.sound.menu",
        theme.iter().map(|(name, _)| sounds::label(name)).zip(0..),
        at,
        move |index: usize| {
            if let Some(name) = names.get(index) {
                sounds::play(name, volume, Duration::ZERO);
                let name = name.clone();
                change_sound(|now| now.notification = Some(name));
            }
        },
    )
    .width(crate::MENU_W)
    .disabled(!on)
    .build();
    let play = IconButton::new("settings.sound.notifications.play", ICON_PLAY)
        .tooltip(crate::WINDOW, "Play")
        .disabled(!on)
        .on_press(move || sounds::play(&chosen, volume, Duration::ZERO))
        .build();
    alerts
        .row(
            FormRow::new("settings.sound.notifications.sound", "Sound")
                .child()
                .disabled(!on)
                .trailing(
                    Row::new(vec![boxed(menu), boxed(play)])
                        .gap(emaki_kit::tokens::sys::space::gap::xs())
                        .align(Center),
                ),
        )
        .slider(
            SliderRow::new("Alert volume", move |width| {
                Slider::new(volume, width)
                    .dimmed(!on)
                    .inert(!on)
                    .on_value_change(|value| change_sound(|now| now.volume = value))
                    .build()
            })
            .value(format!("{volume}%")),
        )
        .footer("Do Not Disturb, and apps set to List Only or Off below, stay silent.")
}

/// The theme's sounds, read once: they change only with the system.
fn theme() -> &'static [(String, PathBuf)] {
    static THEME: OnceLock<Vec<(String, PathBuf)>> = OnceLock::new();
    THEME.get_or_init(sounds::theme_sounds)
}

fn percent(volume: u8, muted: bool) -> String {
    if muted {
        String::from("Muted")
    } else {
        format!("{volume}%")
    }
}

/// The slider's 0 to 100 for a balance of −1 to 1.
fn balance_value(balance: f32) -> u8 {
    ((balance.clamp(-1.0, 1.0) + 1.0) * 50.0).round() as u8
}

/// The balance a slider at `value` sets; near the middle is the middle.
fn balance_of(value: u8) -> f32 {
    if value.abs_diff(50) <= 3 {
        0.0
    } else {
        f32::from(value.min(100)) / 50.0 - 1.0
    }
}

fn balance_label(balance: f32) -> String {
    let lean = (balance.abs() * 100.0).round() as u8;
    match lean {
        0 => String::from("Centered"),
        _ if balance < 0.0 => format!("Left {lean}%"),
        _ => format!("Right {lean}%"),
    }
}

/// How full the meter is for a peak of 0 to 1: decibels from −60 up, as a
/// voice reads across the middle rather than at the bottom.
fn meter(peak: f32) -> f32 {
    if peak <= 0.0 {
        return 0.0;
    }
    ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_goes_both_ways_and_snaps_to_the_middle() {
        assert_eq!(balance_value(0.0), 50);
        assert_eq!(balance_value(-1.0), 0);
        assert_eq!(balance_value(1.0), 100);
        assert_eq!(balance_of(52), 0.0);
        assert_eq!(balance_of(0), -1.0);
        assert_eq!(balance_of(100), 1.0);
        assert!((balance_of(balance_value(0.4)) - 0.4).abs() < 0.02);
    }

    #[test]
    fn balance_reads_as_a_side() {
        assert_eq!(balance_label(0.0), "Centered");
        assert_eq!(balance_label(-0.2), "Left 20%");
        assert_eq!(balance_label(0.5), "Right 50%");
    }

    #[test]
    fn the_meter_reads_in_decibels() {
        assert_eq!(meter(0.0), 0.0);
        assert_eq!(meter(1.0), 1.0);
        assert!((meter(0.03) - 0.49).abs() < 0.02);
        assert_eq!(meter(0.0001), 0.0);
    }
}
