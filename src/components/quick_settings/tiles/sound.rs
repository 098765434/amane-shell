//! Sound — GNOME's output and input sliders (the glyph mutes; the chevron
//! picks the device when there's more than one) and their page, the
//! device lists from the Sound panel. The microphone's slider shows only
//! while something records, as in GNOME.

use amane::{Audio, Column, Parent, Row, ScrollArea, Service, Size, Widget};

use super::{GridCell, settings_link};
use crate::components::buttons::{IconButton, Shape, Style};
use crate::components::icons::{ICON_CHEVRON_RIGHT, ICON_MIC_OFF, ICON_VOLUME_OFF, audio_glyph};
use crate::components::layout::boxed;
use crate::components::quick_settings::detail::DetailPage;
use crate::components::quick_settings::grid::Cell;
use crate::components::quick_settings::tile::Span;
use crate::components::quick_settings::{Page, open};
use crate::components::slider::Slider;
use crate::components::taskbar::audio;
use crate::services::privacy::Privacy;
use crate::tokens::sys;

pub(in crate::components::quick_settings) fn output_cell() -> Option<GridCell> {
    let audio = Audio::read();
    let volume = audio.volume();
    let muted = audio.muted();
    let kind = audio.output_kind();
    let devices = audio.outputs().len();
    drop(audio);

    if devices == 0 {
        return None;
    }
    let glyph = if muted {
        ICON_VOLUME_OFF
    } else {
        audio_glyph(kind, false)
    };
    Some(row("amane.qs.volume", devices > 1, |width| {
        Slider::new(volume, width)
            .large(glyph)
            .dimmed(muted)
            .on_icon_press("amane.qs.volume.mute", Audio::toggle_mute)
            .on_value_change(Audio::set_volume)
    }))
}

pub(in crate::components::quick_settings) fn input_cell() -> Option<GridCell> {
    if !Privacy::read().recording() {
        return None;
    }
    let audio = Audio::read();
    let volume = audio.microphone_volume();
    let muted = audio.microphone_muted();
    let kind = audio.input_kind();
    let devices = audio.inputs().len();
    drop(audio);

    if devices == 0 {
        return None;
    }
    let glyph = if muted {
        ICON_MIC_OFF
    } else {
        audio_glyph(kind, true)
    };
    Some(row("amane.qs.microphone", devices > 1, |width| {
        Slider::new(volume, width)
            .large(glyph)
            .dimmed(muted)
            .on_icon_press("amane.qs.microphone.mute", Audio::toggle_microphone_mute)
            .on_value_change(Audio::set_microphone_volume)
    }))
}

/// A full-width slider, `slider` made for the width it gets, followed by a
/// chevron to the Sound page when there's a device to pick.
pub(in crate::components::quick_settings) fn row(
    id: &str,
    chevron: bool,
    slider: impl FnOnce(f32) -> Slider,
) -> GridCell {
    let height = sys::size::control::height();
    let full = Span::Full.width();
    let mut items: Vec<Box<dyn Widget>> = Vec::new();

    if chevron {
        let slider_w = full - height - sys::space::gap::s();
        items.push(boxed(slider(slider_w).build()));
        items.push(boxed(
            IconButton::new(format!("{id}.devices"), ICON_CHEVRON_RIGHT)
                .style(Style::Tonal)
                .shape(Shape::Round)
                .color(sys::color::content::secondary())
                .on_press(|| open(Page::Sound))
                .build(),
        ));
    } else {
        items.push(boxed(slider(full).build()));
    }

    Cell::Full(boxed(Row::new(items).gap(sys::space::gap::s())), height)
}

pub(in crate::components::quick_settings) fn page() -> DetailPage {
    let kind = Audio::read().output_kind();

    let sections: Vec<Box<dyn Widget>> = [false, true]
        .into_iter()
        .filter_map(audio::device_section)
        .collect();
    let height = sys::size::list::max_height();
    let body = ScrollArea::new(
        "amane.qs.sound.devices",
        Column::new(sections)
            .gap(sys::space::gap::xl())
            .width(Parent),
    )
    .height(Size::Fixed(height));

    let page = DetailPage::new("amane.qs.sound", audio_glyph(kind, false), "Sound")
        .body(vec![boxed(body)], height);
    settings_link(page, "Sound Settings…", "pavucontrol")
}
