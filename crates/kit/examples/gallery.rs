//! Every control the kit has, in one app window: `cargo run -p amane-kit --example gallery`.

use amane::{Column, Key, Row, Service, Size, Text, Widget, Window, modifiers};
use amane_kit::app::{
    Dialog, Page, app, app_window, field_box, sidebar, standard_shortcut, text_field,
};
use amane_kit::buttons::{Checkbox, PushButton, Radio, Shape, Style, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::icons::{ICON_DARK_MODE, ICON_TUNE};
use amane_kit::layout::boxed;
use amane_kit::material::{self, Level};
use amane_kit::slider::Slider;
use amane_kit::tokens::{Styled, sys};

struct Gallery {
    page: usize,
    sounds: bool,
    badges: bool,
    wifi: bool,
    size: usize,
    accent: usize,
    level: u8,
    answer: String,
}

impl Service for Gallery {
    fn new() -> Self {
        Self {
            page: 0,
            sounds: true,
            badges: false,
            wifi: true,
            size: 1,
            accent: 0,
            level: 40,
            answer: String::from("no answer yet"),
        }
    }

    fn listen() {}
}

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];
const ACCENTS: [&str; 4] = ["Blue", "Purple", "Green", "Orange"];

fn main() {
    app("amane-kit-gallery")
        .normal_window("gallery", gallery)
        .run();
}

fn gallery() -> Window {
    let state = Gallery::read();

    let pages = [
        Page::new(ICON_TUNE, "Controls"),
        Page::new(ICON_DARK_MODE, "Text"),
    ];

    let nav = sidebar("gallery.nav", &pages, state.page, |page| {
        Gallery::write().page = page
    });

    let page: Box<dyn Widget> = match state.page {
        0 => boxed(controls(&state)),
        _ => boxed(text()),
    };

    let content = Row::new(vec![
        boxed(Column::new(vec![boxed(nav)]).width(Size::Fixed(180.0))),
        page,
    ])
    .gap(sys::space::gap::l())
    .width(Size::Parent)
    .height(Size::Parent);

    app_window(content)
        .title("Kit gallery")
        .size(720.0, 480.0)
        .min_size(560.0, 360.0)
        .on_shortcut(|key| {
            // Ctrl+D opens the dialog from the keyboard too, Ctrl+S the
            // stacked one
            if modifiers().ctrl && key == Key::Character('d') {
                ask();

                return true;
            }
            if modifiers().ctrl && key == Key::Character('s') {
                ask_stacked();

                return true;
            }

            standard_shortcut(key)
        })
}

fn controls(state: &Gallery) -> Column {
    let label = |text: &str| {
        Text::new(text)
            .style(sys::r#type::caption())
            .color(material::label(Level::Secondary))
    };

    let mut radios: Vec<Box<dyn Widget>> = Vec::new();

    for (index, size) in SIZES.iter().enumerate() {
        radios.push(boxed(
            Radio::new(format!("gallery.size.{index}"), state.size == index)
                .label(*size)
                .on_select(move || Gallery::write().size = index)
                .build(),
        ));
    }

    Column::new(vec![
        boxed(label("Checkboxes")),
        boxed(
            Checkbox::new("gallery.sounds", state.sounds)
                .label("Play sounds")
                .on_toggle(|on| Gallery::write().sounds = on)
                .build(),
        ),
        boxed(
            Checkbox::new("gallery.badges", state.badges)
                .label("Show badges")
                .on_toggle(|on| Gallery::write().badges = on)
                .build(),
        ),
        boxed(label("Radio buttons")),
        boxed(Column::new(radios)),
        boxed(label("Switch, dropdown and slider")),
        boxed(
            Row::new(vec![
                boxed(
                    Switch::new("gallery.wifi", state.wifi)
                        .on_toggle(|on| Gallery::write().wifi = on)
                        .build(),
                ),
                boxed(
                    Dropdown::new(
                        "gallery.accent",
                        ACCENTS.iter().map(|accent| String::from(*accent)).collect(),
                        state.accent,
                    )
                    .on_select(|accent| Gallery::write().accent = accent)
                    .build(),
                ),
                boxed(
                    Slider::new(state.level, 160.0)
                        .on_value_change(|level| Gallery::write().level = level)
                        .build(),
                ),
            ])
            .gap(sys::space::gap::l())
            .align(amane::Center),
        ),
        boxed(label("Dialog")),
        boxed(
            Row::new(vec![
                boxed(
                    PushButton::new("gallery.ask", "Ask (Ctrl+D)")
                        .style(Style::Tonal)
                        .shape(Shape::Round)
                        .on_press(ask)
                        .build(),
                ),
                boxed(
                    PushButton::new("gallery.ask-stacked", "Three answers (Ctrl+S)")
                        .style(Style::Tonal)
                        .shape(Shape::Round)
                        .on_press(ask_stacked)
                        .build(),
                ),
                boxed(
                    Text::new(state.answer.clone())
                        .style(sys::r#type::label())
                        .color(sys::color::surface::popover::fg()),
                ),
            ])
            .gap(sys::space::gap::m())
            .align(amane::Center),
        ),
    ])
    .gap(sys::space::gap::s())
    .width(Size::Parent)
}

fn text() -> Column {
    Column::new(vec![
        boxed(
            Text::new("Name")
                .style(sys::r#type::caption())
                .color(material::label(Level::Secondary)),
        ),
        boxed(field_box(
            "gallery.name",
            text_field("gallery.name", "Your name").focused(),
            280.0,
        )),
    ])
    .gap(sys::space::gap::s())
}

fn ask() {
    Dialog::new("gallery.dialog", "Discard changes?")
        .message("Your changes to the gallery will be lost.")
        .button("Cancel", || {
            Gallery::write().answer = String::from("cancelled")
        })
        .main_button("Discard", || {
            Gallery::write().answer = String::from("discarded")
        })
        .open("gallery");
}

/// Three answers, which stack: the default on top, the destructive one
/// under it, and Cancel at the bottom.
fn ask_stacked() {
    Dialog::new("gallery.dialog.stacked", "Save changes to the gallery?")
        .message("If you don't save, your changes will be lost.")
        .button("Cancel", || {
            Gallery::write().answer = String::from("cancelled")
        })
        .destructive("Don't Save", || {
            Gallery::write().answer = String::from("not saved")
        })
        .main_button("Save", || Gallery::write().answer = String::from("saved"))
        .open("gallery");
}
