//! Sound — Control Center's Sound card: GNOME's output slider (the glyph
//! mutes; the chevron picks the device when there's more than one) and,
//! while something records, the microphone's under it, as in GNOME. Its
//! page is the device lists from the Sound panel.

use amane::{Audio, Column, Parent, ScrollArea, Service, Size, Widget};

use super::super::grid::Span;
use super::super::slider_card::{self, SliderRow};
use super::super::{Page, open};
use super::settings_link;
use crate::components::buttons::{IconButton, Shape, Style, expect, optimistic};
use crate::components::icons::{
    ICON_CHEVRON_RIGHT, ICON_MIC, ICON_MIC_OFF, ICON_VOLUME_DOWN, ICON_VOLUME_OFF, audio_glyph,
};
use crate::components::layout::{boxed, caption};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::detail::page_width;
use crate::components::slider::Slider;
use crate::components::taskbar::audio;
use crate::tokens::sys;

pub(in crate::components::quick_settings) fn cell(
    span: Span,
    inert: bool,
) -> Option<Box<dyn Widget>> {
    let audio = Audio::read();
    let volume = audio.volume();
    // a click on mute shows before the server agrees
    let muted = optimistic(MUTE_ID, audio.muted());
    let kind = audio.output_kind();
    let outputs = audio.outputs().len();
    drop(audio);

    if outputs == 0 {
        return None;
    }

    // the small glyph before the track mutes, and says how loud it is
    let glyph = level_glyph(muted, volume, audio_glyph(kind, false));
    let mut output = SliderRow::new("Sound", glyph, audio_glyph(kind, false), move |width| {
        Slider::new(volume, width)
            .dimmed(muted)
            .inert(inert)
            .on_value_change(Audio::set_volume)
    })
    .leading(mute_button(MUTE_ID, glyph, inert, move || {
        expect(MUTE_ID, !muted);
        Audio::toggle_mute();
    }));
    if outputs > 1 {
        output = output.trailing(
            IconButton::new("amane.qs.volume.devices", ICON_CHEVRON_RIGHT)
                .style(Style::Plain)
                .shape(Shape::Round)
                .color(sys::color::content::secondary())
                .disabled(inert)
                .on_press(|| open(Page::Sound))
                .build(),
        );
    }
    // a right click opens the page, as on a tile
    Some(
        slider_card::card(output, span)
            .on_secondary(|| open(Page::Sound))
            .into(),
    )
}

/// The glyph for a level: off while muted or silent, down while quiet.
fn level_glyph(muted: bool, volume: u8, loud: char) -> char {
    if muted || volume == 0 {
        ICON_VOLUME_OFF
    } else if volume < 50 && loud == audio_glyph(amane::AudioKind::Internal, false) {
        ICON_VOLUME_DOWN
    } else {
        loud
    }
}

const MUTE_ID: &str = "amane.qs.volume.mute";
const MIC_MUTE_ID: &str = "amane.qs.microphone.mute";

/// The glyph before a track, which mutes.
fn mute_button(
    id: &'static str,
    glyph: char,
    inert: bool,
    press: impl Fn() + 'static,
) -> amane::Rectangle {
    IconButton::new(id, glyph)
        .size(sys::size::icon::s())
        .color(sys::color::surface::panel::fg())
        .disabled(inert)
        .on_press(press)
        .build()
}

pub(in crate::components::quick_settings) fn page() -> DetailPage {
    let audio = Audio::read();
    let volume = audio.volume();
    let muted = audio.muted();
    let has_input = !audio.inputs().is_empty();
    let mic_volume = audio.microphone_volume();
    let mic_muted = audio.microphone_muted();
    drop(audio);

    let page = DetailPage::new("amane.qs.sound.page", "Sound")
        .subtitle(if muted {
            String::from("Muted")
        } else {
            format!("{volume}%")
        })
        .body(move |height| {
            let width = page_width();
            let mut sections: Vec<Box<dyn Widget>> = Vec::new();
            // the microphone's level, as Control Center's Sound module
            // shows it while something records
            if has_input {
                let glyph = if mic_muted { ICON_MIC_OFF } else { ICON_MIC };
                sections.push(boxed(
                    Column::new(vec![
                        boxed(caption("Microphone")),
                        boxed(
                            Slider::new(mic_volume, width)
                                .large(glyph)
                                .dimmed(mic_muted)
                                .on_icon_press(MIC_MUTE_ID, Audio::toggle_microphone_mute)
                                .on_value_change(Audio::set_microphone_volume)
                                .build(),
                        ),
                    ])
                    .gap(sys::space::gap::xs())
                    .width(Parent),
                ));
            }
            sections.extend([false, true].into_iter().filter_map(audio::device_section));
            vec![boxed(
                ScrollArea::new(
                    "amane.qs.sound.devices",
                    Column::new(sections)
                        .gap(sys::space::gap::xl())
                        .width(Parent),
                )
                .height(Size::Fixed(height)),
            )]
        });
    settings_link(page, "Sound Settings…", "pavucontrol")
}
