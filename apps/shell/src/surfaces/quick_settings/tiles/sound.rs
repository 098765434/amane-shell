//! Sound — Quick Settings' Sound card: GNOME's output slider (the glyph
//! mutes; the chevron opens the page) and its page: the output's and the
//! microphone's levels, each with the devices to pick from, the default
//! checked, and a level for each app playing. Read from the fork's `Audio`
//! service, which follows the sound server live.

use amane::{
    Audio, AudioDevice, AudioStream, Center, Column, Parent, Row, ScrollArea, Service, Text, Widget,
};

use super::super::grid::Span;
use super::super::slider_card::{self, SliderRow};
use super::super::{Page, open};
use super::emaki_settings_link;
use crate::surfaces::quick_settings::detail::DetailPage;
use crate::surfaces::quick_settings::detail::page_width;
use crate::surfaces::status_bar::Indicator;
use crate::tokens::{Styled, sys};
use emaki_kit::app_icon;
use emaki_kit::buttons::{IconButton, Shape, Style, expect, optimistic};
use emaki_kit::icons::{
    ICON_CHEVRON_RIGHT, ICON_MIC, ICON_MIC_OFF, ICON_VOLUME_OFF, ICON_VOLUME_UP, audio_glyph,
    volume_glyph,
};
use emaki_kit::layout::{boxed, caption, split_row};
use emaki_kit::list::{self, List, ListItem};
use emaki_kit::material;
use emaki_kit::slider::Slider;

pub(in crate::surfaces::quick_settings) fn cell(
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
    let glyph = volume_glyph(muted, volume, kind);
    let output = SliderRow::new("Sound", glyph, audio_glyph(kind, false), move |width| {
        Slider::new(volume, width)
            .dimmed(muted)
            .inert(inert)
            .on_value_change(Audio::set_volume)
    })
    .leading(mute_button(MUTE_ID, glyph, inert, move || {
        expect(MUTE_ID, !muted);
        Audio::toggle_mute();
    }))
    // the page has the microphone and the devices
    .trailing(
        IconButton::new("emaki.qs.volume.devices", ICON_CHEVRON_RIGHT)
            .style(Style::Plain)
            .shape(Shape::Round)
            .color(material::label(material::Level::Secondary))
            .disabled(inert)
            .on_press(|| open(Page::Sound))
            .build(),
    );
    // a right click opens the page, as on a tile
    Some(
        slider_card::card(output, span)
            .on_secondary(|| open(Page::Sound))
            .into(),
    )
}

const MUTE_ID: &str = "emaki.qs.volume.mute";
const OUTPUT_MUTE_ID: &str = "emaki.qs.sound.mute.output";
const MIC_MUTE_ID: &str = "emaki.qs.microphone.mute";

/// The glyph before a track, which mutes.
fn mute_button(
    id: &'static str,
    glyph: char,
    inert: bool,
    press: impl Fn() + 'static,
) -> amane::Rectangle {
    IconButton::new(id, glyph)
        .size(sys::size::icon::s())
        .color(material::label(material::Level::Primary))
        .disabled(inert)
        .on_press(press)
        .build()
}

/// One side's level: what it is and how loud over a slider whose glyph
/// mutes.
struct Level {
    title: &'static str,
    glyph: char,
    volume: u8,
    muted: bool,
    mute_id: &'static str,
    toggle_mute: fn(),
    set_volume: fn(u8),
}

impl Level {
    fn build(self) -> Box<dyn Widget> {
        // percentages are numeric (manual §11.4)
        let percent = Text::new(format!("{}%", self.volume))
            .style(sys::r#type::numeric())
            .color(material::label(material::Level::Secondary));
        boxed(
            Column::new(vec![
                boxed(split_row(caption(self.title), vec![boxed(percent)])),
                boxed(
                    Slider::new(self.volume, page_width())
                        .large(self.glyph)
                        .dimmed(self.muted)
                        .on_icon_press(self.mute_id, self.toggle_mute)
                        .on_value_change(self.set_volume)
                        .build(),
                ),
            ])
            .gap(sys::space::gap::xs())
            .width(Parent),
        )
    }
}

pub(in crate::surfaces::quick_settings) fn page() -> DetailPage {
    let audio = Audio::read();
    let output = Level {
        title: "Output",
        glyph: if audio.muted() {
            ICON_VOLUME_OFF
        } else {
            audio_glyph(audio.output_kind(), false)
        },
        volume: audio.volume(),
        muted: audio.muted(),
        mute_id: OUTPUT_MUTE_ID,
        toggle_mute: Audio::toggle_mute,
        set_volume: Audio::set_volume,
    };
    let microphone = (!audio.inputs().is_empty()).then(|| Level {
        title: "Microphone",
        glyph: if audio.microphone_muted() {
            ICON_MIC_OFF
        } else {
            ICON_MIC
        },
        volume: audio.microphone_volume(),
        muted: audio.microphone_muted(),
        mute_id: MIC_MUTE_ID,
        toggle_mute: Audio::toggle_microphone_mute,
        set_volume: Audio::set_microphone_volume,
    });
    let outputs = audio.outputs().to_vec();
    let inputs = audio.inputs().to_vec();
    let streams = audio.streams().to_vec();
    drop(audio);

    let page = DetailPage::new("emaki.qs.sound.page", "Sound")
        .subtitle(if output.muted {
            String::from("Muted")
        } else {
            format!("{}%", output.volume)
        })
        .status_bar(Indicator::Volume)
        .body(move |height| {
            let mut sections = vec![output.build()];
            sections.extend(device_section(&outputs, false));
            sections.extend(streams.iter().map(app_level));
            // a machine without a microphone has nothing to set
            if let Some(microphone) = microphone {
                sections.push(microphone.build());
                sections.extend(device_section(&inputs, true));
            }
            vec![boxed(
                ScrollArea::new(
                    "emaki.qs.sound.devices",
                    Column::new(sections)
                        .gap(sys::space::gap::xl())
                        .width(Parent),
                )
                .height(height),
            )]
        });
    emaki_settings_link(page, "Sound Settings…", "sound", "pavucontrol")
}

/// One app's level: its icon and name (and what's playing) over a slider
/// whose glyph mutes it alone.
fn app_level(stream: &AudioStream) -> Box<dyn Widget> {
    let index = stream.index();
    let name = if stream.app().is_empty() {
        stream.binary()
    } else {
        stream.app()
    };
    let label = if stream.media().is_empty() || stream.media() == name {
        name.to_string()
    } else {
        format!("{name} · {}", stream.media())
    };
    let key = app_icon::first_known(&[stream.icon(), stream.binary(), stream.app()]);
    let mute_id = format!("emaki.qs.sound.app.{index}.mute");
    // a click on mute shows before the server agrees
    let muted = optimistic(&mute_id, stream.muted());
    let side = sys::size::icon::s();
    let percent = Text::new(if muted {
        String::from("Muted")
    } else {
        format!("{}%", stream.volume())
    })
    .style(sys::r#type::numeric())
    .color(material::label(material::Level::Secondary));
    let head = Row::new(vec![
        boxed(app_icon::tile(key, name, side)),
        boxed(caption(label)),
    ])
    .gap(sys::space::gap::xs())
    .align(Center);
    let pressed = mute_id.clone();
    boxed(
        Column::new(vec![
            boxed(split_row(head, vec![boxed(percent)])),
            boxed(
                Slider::new(stream.volume(), page_width())
                    .large(if muted {
                        ICON_VOLUME_OFF
                    } else {
                        ICON_VOLUME_UP
                    })
                    .dimmed(muted)
                    .on_icon_press(mute_id, move || {
                        expect(&pressed, !muted);
                        Audio::set_stream_muted(index, !muted);
                    })
                    .on_value_change(move |value| Audio::set_stream_volume(index, value))
                    .build(),
            ),
        ])
        .gap(sys::space::gap::xs())
        .width(Parent),
    )
}

/// Where sound goes to (or, with `input`, comes from), the default
/// checked; `None` with nothing to pick.
fn device_section(devices: &[AudioDevice], input: bool) -> Option<Box<dyn Widget>> {
    if devices.is_empty() {
        return None;
    }
    let items = devices
        .iter()
        .map(|device| device_item(device, input))
        .collect();
    Some(boxed(List::new(items).build()))
}

fn device_item(device: &AudioDevice, input: bool) -> ListItem {
    let selected = device.default();
    let name = device.name().to_string();
    let side = if input { "input" } else { "output" };

    let mut item = ListItem::new(
        format!("emaki.qs.sound.{side}.{name}"),
        device.description(),
    )
    .leading(audio_glyph(device.kind(), input))
    .selected(selected);
    if selected {
        item = item.trailing(list::check());
    }

    item.on_press(move || {
        if selected {
            return;
        }
        if input {
            Audio::set_input(&name);
        } else {
            Audio::set_output(&name);
        }
    })
}
