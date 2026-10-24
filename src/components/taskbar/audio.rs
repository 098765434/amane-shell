//! Sound panel — a bottom-right `LayerWindow` to set the volume and pick where
//! sound goes to and comes from.
//!
//! One of the bar's [`panel`](super::panel)s, toggled by the volume button.
//! Each half (output, microphone) is a mute button, a slider and the list of
//! devices, the default one checked; clicking another makes it the default.
//! Read from the fork's `Audio` service, which follows the sound server live.

use amane::{
    Audio, AudioDevice, Center, Color, Column, End, LayerWindow, Parent, Rectangle, Row,
    ScrollArea, Service, Size, Text, Widget,
};

use super::panel::{self, Panel};
use crate::components::app_bar::TopAppBar;
use crate::components::buttons::{IconButton, Shape, Style};
use crate::components::icons::{ICON_MIC_OFF, ICON_VOLUME_OFF, audio_glyph};
use crate::components::layout::{boxed, caption, inner_width, text_width};
use crate::components::list::{self, List, ListItem};
use crate::components::slider::Slider;
use crate::tokens::{Styled, sys};

/// Layer-surface namespace; the niri `layer-rule` matches this for blur.
pub const AUDIO_NS: &str = "amane.taskbar.audio";

/// What one half of the panel controls, so output and microphone share the
/// layout below.
struct Half {
    title: &'static str,
    input: bool,
    volume: u8,
    muted: bool,
    devices: Vec<AudioDevice>,
    toggle_mute: fn(),
    set_volume: fn(u8),
    set_default: fn(&str),
}

// ───── view ──────────────────────────────────────────────────────────────────

pub fn panel() -> LayerWindow {
    panel::surface(AUDIO_NS, Panel::Audio, card)
}

fn card() -> Rectangle {
    let audio = Audio::read();
    let output_kind = audio.output_kind();
    let output = Half {
        title: "Output",
        input: false,
        volume: audio.volume(),
        muted: audio.muted(),
        devices: audio.outputs().to_vec(),
        toggle_mute: Audio::toggle_mute,
        set_volume: Audio::set_volume,
        set_default: Audio::set_output,
    };
    let input = Half {
        title: "Input",
        input: true,
        volume: audio.microphone_volume(),
        muted: audio.microphone_muted(),
        devices: audio.inputs().to_vec(),
        toggle_mute: Audio::toggle_microphone_mute,
        set_volume: Audio::set_microphone_volume,
        set_default: Audio::set_input,
    };
    drop(audio);

    let header = TopAppBar::new("Sound")
        .navigation_icon(audio_glyph(output_kind, false))
        .build();

    let mut halves = vec![half(output)];
    // a machine without a microphone has nothing to pick
    if !input.devices.is_empty() {
        halves.push(half(input));
    }

    let body = Column::new(halves).gap(sys::space::gap::xl()).width(Parent);

    panel::card(
        vec![
            boxed(header),
            boxed(
                ScrollArea::new("amane.audio.body", body)
                    .height(Size::Fixed(sys::size::list::max_height())),
            ),
        ],
        sys::space::gap::l(),
    )
}

fn half(half: Half) -> Box<dyn Widget> {
    // the mute button shows what is in use, so it doubles as the default's icon
    let default_kind = half
        .devices
        .iter()
        .find(|device| device.default())
        .map(AudioDevice::kind)
        .unwrap_or_default();

    let devices: Vec<ListItem> = half
        .devices
        .iter()
        .map(|device| device_item(device, half.input, half.set_default))
        .collect();

    let mut rows = vec![
        boxed(caption(half.title)),
        boxed(level(
            audio_glyph(default_kind, half.input),
            half.input,
            half.volume,
            half.muted,
            half.toggle_mute,
            half.set_volume,
        )),
    ];
    if !half.devices.is_empty() {
        rows.push(boxed(List::new(devices).build()));
    }

    boxed(Column::new(rows).gap(sys::space::gap::xs()).width(Parent))
}

/// Mute button, slider and percentage on one line.
fn level(
    glyph: char,
    input: bool,
    volume: u8,
    muted: bool,
    toggle_mute: fn(),
    set_volume: fn(u8),
) -> Row {
    let (glyph, color) = match (muted, input) {
        (true, false) => (ICON_VOLUME_OFF, sys::color::content::secondary()),
        (true, true) => (ICON_MIC_OFF, sys::color::content::secondary()),
        (false, _) => (glyph, sys::color::surface::panel::fg()),
    };

    let button_w = sys::size::control::height();
    // percentages are numeric (manual §11.4), with room for "100%"
    let text = sys::r#type::numeric();
    let percent_w = text_width("100%", text);
    let inner = inner_width(sys::size::popover::width());
    let slider_w = (inner - button_w - percent_w - sys::space::gap::s() * 2.0).max(1.0);

    let id = if input {
        "amane.audio.mute.input"
    } else {
        "amane.audio.mute.output"
    };
    let mute = IconButton::new(id, glyph)
        .style(Style::Tonal)
        .shape(Shape::Square)
        .color(color)
        .on_press(toggle_mute)
        .build();

    let percent = Rectangle::new()
        .width(Size::Fixed(percent_w))
        .height(Size::Fixed(button_w))
        .fill(Color::TRANSPARENT)
        .align_child(End, Center)
        .child(
            Text::new(format!("{volume}%"))
                .style(text)
                .color(sys::color::content::secondary()),
        );

    Row::new(vec![
        boxed(mute),
        boxed(
            Slider::new(volume, slider_w)
                .dimmed(muted)
                .on_value_change(set_volume)
                .build(),
        ),
        boxed(percent),
    ])
    .gap(sys::space::gap::s())
    .width(Parent)
    .align(Center)
}

/// Where sound goes to (or, with `input`, comes from), under a header, the
/// default checked; `None` with nothing to pick. Quick Settings' Sound page
/// is these two lists.
pub(crate) fn device_section(input: bool) -> Option<Box<dyn Widget>> {
    let audio = Audio::read();
    let devices = if input {
        audio.inputs().to_vec()
    } else {
        audio.outputs().to_vec()
    };
    drop(audio);

    if devices.is_empty() {
        return None;
    }
    let set_default: fn(&str) = if input {
        Audio::set_input
    } else {
        Audio::set_output
    };
    let items = devices
        .iter()
        .map(|device| device_item(device, input, set_default))
        .collect();
    let header = if input { "Input" } else { "Output" };

    Some(boxed(List::new(items).header(header).build()))
}

fn device_item(device: &AudioDevice, input: bool, set_default: fn(&str)) -> ListItem {
    let selected = device.default();
    let name = device.name().to_string();
    let side = if input { "input" } else { "output" };

    let mut item = ListItem::new(format!("amane.audio.{side}.{name}"), device.description())
        .leading(audio_glyph(device.kind(), input))
        .selected(selected);
    if selected {
        item = item.trailing(list::check());
    }

    item.on_press(move || {
        if !selected {
            set_default(&name);
        }
    })
}
