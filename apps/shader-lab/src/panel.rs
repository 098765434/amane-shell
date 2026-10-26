//! The controls beside the preview: what to show, then one row per
//! parameter the effect lists, then what went wrong, if anything did.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;

use amane::{Clipboard, Color, Column, Row, Service, Size, Text, TextInput, Widget};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::kinetic::Axis;
use amane_kit::layout::{boxed, text_height};
use amane_kit::material::{self, Level};
use amane_kit::scroll_view::ScrollView;
use amane_kit::slider::Slider;
use amane_kit::tokens::{Styled, sys};

use crate::effects::{Effects, Manifest, Param};
use crate::{BACKDROPS, Lab, SUBJECTS};

const WIDTH: f32 = 380.0;
const LABEL_WIDTH: f32 = 92.0;
const SLIDER_WIDTH: f32 = 170.0;
const FIELD_WIDTH: f32 = 72.0;

/// The controls, scrolled when they're taller than the window.
pub fn panel(effects: &Effects, lab: &Lab) -> amane::Rectangle {
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();

    // how tall the rows are, added up as they go in, for the scroll view
    let control = sys::size::control::height();
    let captioned = text_height(sys::r#type::caption()) + sys::space::gap::xs() + control;
    let mut length = captioned * 2.0 + control;

    let names: Vec<String> = effects.list.iter().map(|entry| entry.title()).collect();

    rows.push(boxed(setting(
        "Effect",
        Dropdown::new("lab.effect", names, lab.effect)
            .width(WIDTH)
            .on_select(|index| Lab::write().effect = index)
            .build(),
    )));

    rows.push(boxed(
        Row::new(vec![
            boxed(setting(
                "Subject",
                Dropdown::new("lab.subject", to_strings(&SUBJECTS), lab.subject)
                    .width(WIDTH / 2.0 - 6.0)
                    .on_select(|index| Lab::write().subject = index)
                    .build(),
            )),
            boxed(setting(
                "Backdrop",
                Dropdown::new("lab.backdrop", to_strings(&BACKDROPS), lab.backdrop)
                    .width(WIDTH / 2.0 - 6.0)
                    .on_select(|index| Lab::write().backdrop = index)
                    .build(),
            )),
        ])
        .gap(12.0),
    ));

    // a subject filling the preview has no size of its own
    if lab.subject != 5 {
        rows.push(boxed(size_row(lab)));
    } else {
        length -= control;
    }

    match lab.subject {
        3 => rows.push(boxed(text_row("lab.text", "Text", &lab.text, |text| {
            Lab::write().text = text;
        }))),
        4 => rows.push(boxed(text_row("lab.image", "Image", &lab.image, |path| {
            Lab::write().image = path;
        }))),
        _ => {}
    }

    if matches!(lab.subject, 3 | 4) {
        length += control;
    }

    let entry = effects.list.get(lab.effect);

    if let Some(Ok(manifest)) = entry.map(|entry| &entry.manifest) {
        if !manifest.presets.is_empty() {
            rows.push(boxed(setting("Preset", presets(manifest, lab))));

            length += captioned;
        }

        if let Some(gradient) = &manifest.gradient {
            let presets = gradient
                .presets
                .iter()
                .map(|preset| preset.name.clone())
                .collect();
            let name = manifest.name.clone();

            rows.push(boxed(setting(
                &gradient.label,
                Dropdown::new("lab.preset", presets, lab.preset(manifest))
                    .width(WIDTH)
                    .on_select(move |index| {
                        Lab::write().presets.insert(name.clone(), index);
                    })
                    .build(),
            )));

            length += captioned;
        }

        // the params, then the buttons
        length += control * (manifest.params.len() + 1) as f32;

        for param in &manifest.params {
            rows.push(boxed(param_row(
                manifest,
                param,
                lab.value(manifest, param),
            )));
        }

        rows.push(boxed(actions(
            manifest,
            lab,
            &entry.map(|e| e.dir.clone()).unwrap_or_default(),
        )));
    }

    if let Some((errors, lines)) = errors(entry.and_then(|entry| entry.manifest.as_ref().err())) {
        rows.push(boxed(errors));

        length += lines as f32 * (ERROR_LINE + ERROR_GAP);
    }

    let gap = sys::space::gap::m();

    length += gap * rows.len().saturating_sub(1) as f32;

    // the window's padding is above and below the panel
    let viewport = (amane::window_size().1 - sys::space::inset::l() * 2.0).max(1.0);
    let furthest = (length - viewport).max(0.0);

    let column = Column::new(rows).gap(gap).width(Size::Fixed(WIDTH));

    ScrollView::new(Axis::Vertical, WIDTH, viewport)
        .content(length)
        .offset(lab.scroll.clamp(0.0, furthest))
        .scrollbar(if furthest > 0.0 { 1.0 } else { 0.0 })
        .on_scroll(move |scroll| {
            let mut lab = Lab::write();
            lab.scroll = (lab.scroll.clamp(0.0, furthest) + scroll.y).clamp(0.0, furthest);
        })
        .build(column)
}

/// The manifest's named sets of values; "Custom" while the values match none.
fn presets(manifest: &Manifest, lab: &Lab) -> amane::Rectangle {
    let matches = |preset: &crate::effects::ParamPreset| {
        preset.values.iter().all(|(key, value)| {
            let param = manifest.params.iter().find(|param| &param.key == key);

            param.is_some_and(|param| (lab.value(manifest, param) - value).abs() < 0.01)
        })
    };

    let chosen = manifest
        .presets
        .iter()
        .position(matches)
        .map_or(0, |index| index + 1);

    let names = std::iter::once("Custom".to_owned())
        .chain(manifest.presets.iter().map(|preset| preset.name.clone()))
        .collect();

    let (name, presets) = (manifest.name.clone(), manifest.presets.clone());

    Dropdown::new("lab.param-preset", names, chosen)
        .width(WIDTH)
        .on_select(move |index| {
            let Some(preset) = index.checked_sub(1).and_then(|index| presets.get(index)) else {
                return;
            };

            let mut lab = Lab::write();

            for (key, value) in &preset.values {
                lab.set(&name, key, *value);
            }
        })
        .build()
}

/// A setting's name over its control.
fn setting(name: &str, control: impl Widget + 'static) -> Column {
    Column::new(vec![boxed(caption(name)), boxed(control)]).gap(sys::space::gap::xs())
}

fn caption(text: &str) -> Text {
    Text::new(text)
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

/// Lined up with the controls beside it.
fn centered(text: Text) -> amane::Rectangle {
    amane::Rectangle::new()
        .width(56.0)
        .height(Size::Fixed(sys::size::control::height()))
        .align_child(amane::Start, amane::Center)
        .child(text)
}

fn label(text: &str) -> amane::Rectangle {
    amane::Rectangle::new()
        .width(LABEL_WIDTH)
        .height(Size::Fixed(sys::size::control::height()))
        .align_child(amane::Start, amane::Center)
        .child(
            Text::new(text)
                .style(sys::r#type::label())
                .color(material::label(Level::Primary)),
        )
}

fn size_row(lab: &Lab) -> Row {
    let (min, max) = (40.0, 300.0);
    let position = ((lab.size - min) / (max - min) * 100.0).round() as u8;

    Row::new(vec![
        boxed(label("Size")),
        boxed(
            Slider::new(position, SLIDER_WIDTH)
                .on_value_change(move |position| {
                    Lab::write().size = min + f32::from(position) / 100.0 * (max - min);
                })
                .build(),
        ),
        boxed(centered(caption(&format!("{:.0}", lab.size)))),
    ])
    .gap(12.0)
}

fn text_row(id: &'static str, name: &str, now: &str, set: impl Fn(String) + 'static) -> Row {
    show(id, now);

    Row::new(vec![
        boxed(label(name)),
        boxed(field_box(
            id,
            text_field(id, "").on_submit(set),
            WIDTH - LABEL_WIDTH - 12.0,
        )),
    ])
    .gap(12.0)
}

/// A slider over the parameter's range, and a field that takes an exact number.
fn param_row(manifest: &Manifest, param: &Param, value: f32) -> Row {
    let span = (param.max - param.min).max(f32::EPSILON);
    let position = ((value - param.min) / span * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8;

    let id = static_id(format!("lab.{}.{}", manifest.name, param.key));

    show(id, &format_value(value, param));

    let slid = {
        let (name, param) = (manifest.name.clone(), param.clone());

        move |position: u8| {
            let value = param.snap(param.min + f32::from(position) / 100.0 * span);
            Lab::write().set(&name, &param.key, value);
        }
    };

    let typed = {
        let (name, param) = (manifest.name.clone(), param.clone());

        move |text: String| {
            let number = text.trim().trim_end_matches(param.unit.as_str()).trim();

            if let Ok(value) = number.parse::<f32>() {
                Lab::write().set(&name, &param.key, value.clamp(param.min, param.max));
            }
        }
    };

    Row::new(vec![
        boxed(label(&param.label)),
        boxed(
            Slider::new(position, SLIDER_WIDTH)
                .on_value_change(slid)
                .build(),
        ),
        boxed(field_box(
            id,
            text_field(id, "").on_submit(typed),
            FIELD_WIDTH,
        )),
    ])
    .gap(12.0)
}

fn actions(manifest: &Manifest, lab: &Lab, dir: &std::path::Path) -> Row {
    let mut buttons: Vec<Box<dyn Widget>> = Vec::new();

    if manifest.animate.is_some() {
        buttons.push(boxed(
            Row::new(vec![
                boxed(
                    Switch::new("lab.animate", lab.animate)
                        .on_toggle(|on| {
                            let mut lab = Lab::write();
                            lab.animate = on;
                            lab.started = std::time::Instant::now();
                        })
                        .build(),
                ),
                boxed(centered(caption("Animate"))),
            ])
            .gap(8.0),
        ));
    }

    let name = manifest.name.clone();

    buttons.push(boxed(
        PushButton::new("lab.reset", "Reset")
            .on_press(move || {
                let mut lab = Lab::write();
                lab.values.remove(&name);
                lab.presets.remove(&name);
            })
            .build(),
    ));

    let snippet = rust_snippet(manifest, lab, dir);

    buttons.push(boxed(
        PushButton::new("lab.copy", "Copy as Rust")
            .on_press(move || Clipboard::copy(snippet.clone()))
            .build(),
    ));

    Row::new(buttons).gap(12.0)
}

/// The effect as it's set now, to paste into the shell.
fn rust_snippet(manifest: &Manifest, lab: &Lab, dir: &std::path::Path) -> String {
    let presets = manifest.gradient.as_ref().map(|gradient| &gradient.presets);
    let preset = presets.and_then(|presets| presets.get(lab.preset(manifest)));
    let values = manifest.pack(|param| lab.value(manifest, param), preset);

    let mut text = String::from("Effect::new()\n");

    for pass in &manifest.passes {
        let inputs: Vec<String> = pass.inputs.iter().map(|input| input.rust()).collect();

        let _ = writeln!(
            text,
            "    .pass({:?}, [{}])",
            dir.join(&pass.shader).display().to_string(),
            inputs.join(", ")
        );
    }

    text.push_str("    .values(vec![\n");

    for row in values {
        let _ = writeln!(
            text,
            "        [{:?}, {:?}, {:?}, {:?}],",
            row[0], row[1], row[2], row[3]
        );
    }

    let _ = write!(text, "    ])\n    .margin({:?})", manifest.margin);

    text
}

const ERROR_LINE: f32 = 14.0;
const ERROR_GAP: f32 = 2.0;

/// The manifest's error and each shader that failed, in red, and how many lines that is.
fn errors(manifest: Option<&String>) -> Option<(Column, usize)> {
    let mut messages: Vec<String> = manifest.cloned().into_iter().collect();

    for (_, error) in amane::shader_errors() {
        messages.push(error);
    }

    if messages.is_empty() {
        return None;
    }

    let lines: Vec<Box<dyn Widget>> = messages
        .iter()
        .flat_map(|message| message.lines().take(12))
        .map(|line| {
            boxed(
                Text::new(line.to_owned())
                    .font("monospace")
                    .size(11.0)
                    .color(Color::rgba(255, 107, 107, 255)),
            )
        })
        .collect();

    let count = lines.len();

    Some((
        Column::new(lines).gap(ERROR_GAP).width(Size::Fixed(WIDTH)),
        count,
    ))
}

/// Puts the value in the field, unless it's being typed into.
fn show(id: &'static str, value: &str) {
    if !TextInput::has_focus(id) && TextInput::text(id) != value {
        TextInput::set_text(id, value);
    }
}

fn format_value(value: f32, param: &Param) -> String {
    let number = if param.step >= 1.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };

    format!("{number}{}", param.unit)
}

fn to_strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// A text field's id has to be `'static`; there's one per parameter, made once.
fn static_id(id: String) -> &'static str {
    thread_local! {
        static IDS: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }

    IDS.with_borrow_mut(|ids| {
        *ids.entry(id.clone())
            .or_insert_with(|| Box::leak(id.into_boxed_str()))
    })
}
