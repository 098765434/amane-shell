//! The controls beside the preview: saved looks, what to show, time, then
//! the effect's presets, gradient, params and colours, the shadows, and
//! what went wrong, if anything did.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;

use amane::{Clipboard, Color, Column, Rectangle, Row, Service, Size, Text, TextInput, Widget};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Switch};
use amane_kit::color_picker::{ColorPicker, parse_hex, to_color, to_hex};
use amane_kit::dialog::Dialog;
use amane_kit::dropdown::Dropdown;
use amane_kit::kinetic::Axis;
use amane_kit::lab::config::{Backdrop, Config, Kind};
use amane_kit::lab::manifest::{ColorParam, Manifest, Param};
use amane_kit::layout::{boxed, pad, text_height};
use amane_kit::material::{self, Level};
use amane_kit::scroll_view::ScrollView;
use amane_kit::slider::Slider;
use amane_kit::tokens::{Styled, sys};

use crate::configs::{self, Configs};
use crate::effects::Effects;
use crate::{Lab, Naming, change, gradient, load, pick_effect, preview, redo, shadows, undo};

pub const WIDTH: f32 = 380.0;
const LABEL_WIDTH: f32 = 92.0;
const SLIDER_WIDTH: f32 = 170.0;
const FIELD_WIDTH: f32 = 72.0;

const ERROR_LINE: f32 = 14.0;
const ERROR_GAP: f32 = 2.0;

/// The rows so far, and how tall they are together, for the scroll view.
struct Rows {
    widgets: Vec<Box<dyn Widget>>,
    length: f32,
}

impl Rows {
    fn push(&mut self, widget: impl Widget + 'static, height: f32) {
        self.widgets.push(boxed(widget));
        self.length += height;
    }
}

/// The controls, scrolled when they're taller than the window.
pub fn panel(effects: &Effects, lab: &Lab) -> Rectangle {
    let control = sys::size::control::height();
    let captioned = caption_height() + sys::space::gap::xs() + control;

    let mut rows = Rows {
        widgets: Vec::new(),
        length: 0.0,
    };

    let (looks, looks_height) = saved_looks(lab);
    rows.push(looks, looks_height);

    let entry = preview::current(effects, lab);
    let manifest = entry.and_then(|entry| entry.manifest.as_ref().ok());

    let names: Vec<String> = effects.list.iter().map(|entry| entry.title()).collect();
    let chosen = effects.position(&lab.state.effect).unwrap_or(0);

    rows.push(
        setting(
            "Effect",
            Dropdown::new("lab.effect", names, chosen)
                .width(WIDTH)
                .on_select(pick_effect)
                .build(),
        ),
        captioned,
    );

    rows.push(subject_and_backdrop(lab), captioned);

    let subject = &lab.state.subject;

    // a subject filling the preview has no size of its own
    if subject.kind != Kind::Fill {
        let number = Number {
            id: "lab.size".into(),
            label: "Size".into(),
            value: subject.size,
            min: 40.0,
            max: 300.0,
            step: 1.0,
            unit: String::new(),
        };

        rows.push(
            number_row(number, |size| {
                change("size", |state| state.subject.size = size)
            }),
            control,
        );

        rows.push(number_pair("Position", subject.x, subject.y), control);
    }

    let field: Option<(&'static str, &str, &str)> = match subject.kind {
        Kind::Text => Some(("lab.text", "Text", &subject.text)),
        Kind::Image => Some(("lab.image", "Image", &subject.image)),
        Kind::Photo => Some(("lab.photo", "Photo", &subject.photo)),
        _ => None,
    };

    if let Some((id, name, now)) = field {
        let kind = subject.kind;

        rows.push(
            text_row(id, name, now, move |text| {
                change(id, |state| match kind {
                    Kind::Text => state.subject.text = text,
                    Kind::Image => state.subject.image = text,
                    _ => state.subject.photo = text,
                });
            }),
            control,
        );
    }

    let (time, time_height) = time_section(lab, manifest);
    rows.push(time, time_height);

    if let (Some(entry), Some(manifest)) = (entry, manifest) {
        if !manifest.presets.is_empty() {
            rows.push(setting("Preset", presets(manifest, lab)), captioned);
        }

        if let Some((editor, height)) = gradient::editor(manifest, lab) {
            rows.push(editor, height);
        }

        for param in &manifest.params {
            let value = lab.value(manifest, param);

            if param.options.is_empty() {
                rows.push(param_row(manifest, param, value), control);
            } else {
                rows.push(options_row(manifest, param, value), control);
            }
        }

        for param in &manifest.colors {
            let (row, height) = color_row(manifest, param, lab);

            rows.push(row, height);
        }

        let (shadows, shadows_height) = shadows::section(lab);
        rows.push(shadows, shadows_height);

        rows.push(actions(manifest, lab, &entry.dir), control);
    }

    if let Some((errors, lines)) = errors(entry.and_then(|entry| entry.manifest.as_ref().err())) {
        rows.push(errors, lines as f32 * (ERROR_LINE + ERROR_GAP));
    }

    let gap = sys::space::gap::m();

    let length = rows.length + gap * rows.widgets.len().saturating_sub(1) as f32;

    // the window's padding is above and below the panel
    let viewport = (amane::window_size().1 - sys::space::inset::l() * 2.0).max(1.0);
    let furthest = (length - viewport).max(0.0);

    let column = Column::new(rows.widgets).gap(gap).width(Size::Fixed(WIDTH));

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

/// The saved looks, the buttons that save, rename and delete them, and the
/// name being typed.
fn saved_looks(lab: &Lab) -> (Column, f32) {
    let configs = Configs::read();
    let control = sys::size::control::height();
    let gap = sys::space::gap::xs();

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(caption("Saved looks"))];
    let mut height = caption_height();

    if !configs::reaches_the_shell() {
        rows.push(boxed(note(&format!(
            "Saved in {}; run with --effects apps/shader-lab/effects to save where the shell finds them.",
            configs::folder().display()
        ))));
        height += control * 1.5 + gap;
    }

    if configs.list.is_empty() {
        rows.push(boxed(note("Nothing saved yet.")));
        height += control + gap;
    }

    for saved in &configs.list {
        let chosen = lab.saved_as.as_deref() == Some(saved.slug.as_str());

        let effect = match &saved.config {
            Ok(config) => config.effect.clone(),
            Err(error) => error.clone(),
        };

        let slug = saved.slug.clone();
        let config = saved.config.clone().ok();

        let row = Rectangle::new()
            .width(WIDTH)
            .height(Size::Fixed(control))
            .radius(sys::radius::raised())
            .fill(if chosen {
                sys::color::state::selected()
            } else {
                Color::TRANSPARENT
            })
            .padding(pad(sys::space::inset::s(), 0.0))
            .cursor(amane::Pointer)
            .on_click(move |_| {
                if let Some(config) = &config {
                    load(&slug, config);
                }
            })
            .child(
                Row::new(vec![
                    boxed(label_width(&saved.title(), WIDTH * 0.6)),
                    boxed(
                        Rectangle::new()
                            .width(
                                WIDTH * 0.4 - sys::space::inset::s() * 2.0 - sys::space::gap::s(),
                            )
                            .height(Size::Fixed(control))
                            .align_child(amane::End, amane::Center)
                            .child(caption(&effect).elide()),
                    ),
                ])
                .gap(sys::space::gap::s()),
            );

        rows.push(boxed(row));
        height += control + gap;
    }

    if let Some(naming) = &lab.naming {
        rows.push(boxed(naming_row(naming, &lab.state)));
        height += control + gap;
    }

    let has_file = lab.saved_as.is_some();

    let buttons = Row::new(vec![
        boxed(button("lab.save-as", "Save as…", true, || {
            Lab::write().naming = Some(Naming::SaveAs);
        })),
        boxed(button("lab.update", "Update", has_file, update_saved)),
        boxed(button("lab.rename", "Rename", has_file, || {
            let mut lab = Lab::write();

            if let Some(slug) = lab.saved_as.clone() {
                lab.naming = Some(Naming::Rename(slug));
            }
        })),
        boxed(button("lab.delete", "Delete", has_file, ask_to_delete)),
    ])
    .gap(sys::space::gap::xs());

    rows.push(boxed(buttons));
    height += control + gap;

    let loader = button(
        "lab.copy-loader",
        "Copy loader line",
        has_file,
        copy_loader_line,
    );

    rows.push(boxed(loader));
    height += control + gap;

    if let Some(message) = &lab.message {
        rows.push(boxed(note(message)));
        height += control + gap;
    }

    (Column::new(rows).gap(gap), height)
}

/// The name being typed for Save as or Rename; Enter saves it.
fn naming_row(naming: &Naming, state: &Config) -> Row {
    let id = "lab.naming";

    if !TextInput::has_focus(id) && TextInput::text(id).is_empty() {
        TextInput::set_text(id, &state.name);
    }

    let naming = naming.clone();

    let submit = move |name: String| {
        let name = name.trim().to_owned();

        if name.is_empty() {
            return;
        }

        match &naming {
            Naming::SaveAs => save_as(&name),
            Naming::Rename(old) => rename(old, &name),
        }

        TextInput::set_text(id, "");
    };

    Row::new(vec![
        boxed(field_box(
            id,
            text_field(id, "Name, then Enter")
                .focused()
                .on_submit(submit),
            WIDTH - 90.0,
        )),
        boxed(button("lab.naming.cancel", "Cancel", true, || {
            Lab::write().naming = None;
            TextInput::set_text("lab.naming", "");
        })),
    ])
    .gap(sys::space::gap::xs())
}

/// The look as it is, every value written out, under its name.
fn completed(name: &str) -> Config {
    let effects = Effects::read();
    let lab = Lab::read();

    let mut config = lab.state.clone();
    config.name = name.to_owned();

    if let Some(manifest) =
        preview::current(&effects, &lab).and_then(|entry| entry.manifest.as_ref().ok())
    {
        config.complete(manifest);
    }

    config
}

fn save_as(name: &str) {
    let config = completed(name);
    let result = configs::save(&config);

    finish_saving(result, config);
}

/// Saves the look over the file it came from.
pub fn update_saved() {
    let name = {
        let lab = Lab::read();

        if lab.saved_as.is_none() {
            return;
        }

        lab.state.name.clone()
    };

    let config = completed(&name);
    let result = configs::save(&config);

    finish_saving(result, config);
}

fn rename(old: &str, name: &str) {
    let config = completed(name);
    let result = configs::rename(old, &config);

    finish_saving(result, config);
}

fn finish_saving(result: Result<String, String>, config: Config) {
    let mut lab = Lab::write();

    match result {
        Ok(slug) => {
            lab.state.name = config.name;
            lab.message = Some(format!(
                "Saved as {}",
                configs::folder().join(format!("{slug}.json")).display()
            ));
            lab.saved_as = Some(slug);
            lab.naming = None;
        }

        Err(error) => lab.message = Some(format!("Couldn't save: {error}")),
    }
}

fn ask_to_delete() {
    let Some(slug) = Lab::read().saved_as.clone() else {
        return;
    };

    let name = Lab::read().state.name.clone();

    Dialog::new("lab.delete-look", format!("Delete “{name}”?"))
        .message(format!("{slug}.json goes from the configs folder. The look stays in the lab until you change it."))
        .button("Cancel", || {})
        .destructive("Delete", move || {
            let result = configs::delete(&slug);
            let mut lab = Lab::write();

            match result {
                Ok(()) => {
                    lab.saved_as = None;
                    lab.message = Some(format!("Deleted {slug}.json"));
                }

                Err(error) => lab.message = Some(format!("Couldn't delete: {error}")),
            }
        })
        .open("main");
}

/// The line that puts this look on a rectangle in the shell.
fn copy_loader_line() {
    let effects = Effects::read();
    let lab = Lab::read();

    let Some(slug) = &lab.saved_as else {
        return;
    };

    let manifest = preview::current(&effects, &lab).and_then(|entry| entry.manifest.as_ref().ok());
    let (width, height) = preview::size_of(&lab.state.subject);
    let radius = preview::radius_of(&lab, manifest);

    Clipboard::copy(format!(
        "amane_kit::lab::look({slug:?}, {width:.1}, {height:.1}, {radius:.1})"
    ));
}

fn subject_and_backdrop(lab: &Lab) -> Row {
    let subjects = Kind::ALL
        .iter()
        .map(|kind| kind.label().to_owned())
        .collect();
    let backdrops = Backdrop::ALL
        .iter()
        .map(|backdrop| backdrop.label().to_owned())
        .collect();

    Row::new(vec![
        boxed(setting(
            "Subject",
            Dropdown::new("lab.subject", subjects, lab.state.subject.kind.index())
                .width(WIDTH / 2.0 - 6.0)
                .on_select(|index| {
                    change("subject", |state| {
                        state.subject.kind = Kind::from_index(index)
                    })
                })
                .build(),
        )),
        boxed(setting(
            "Backdrop",
            Dropdown::new("lab.backdrop", backdrops, lab.state.backdrop.index())
                .width(WIDTH / 2.0 - 6.0)
                .on_select(|index| {
                    change("backdrop", |state| {
                        state.backdrop = Backdrop::from_index(index)
                    })
                })
                .build(),
        )),
    ])
    .gap(12.0)
}

/// Where the subject is, with a button to put it back in the middle.
fn number_pair(name: &str, x: f32, y: f32) -> Row {
    Row::new(vec![
        boxed(label(name)),
        boxed(
            Rectangle::new()
                .width(SLIDER_WIDTH)
                .height(Size::Fixed(sys::size::control::height()))
                .align_child(amane::Start, amane::Center)
                .child(caption(&format!("{x:.0}, {y:.0} px"))),
        ),
        boxed(button("lab.center", "Centre", x != 0.0 || y != 0.0, || {
            change("move", |state| {
                state.subject.x = 0.0;
                state.subject.y = 0.0;
            });
        })),
    ])
    .gap(12.0)
}

/// Holding the shader clock and scrubbing it, and the effect's own sweep.
fn time_section(lab: &Lab, manifest: Option<&Manifest>) -> (Column, f32) {
    let control = sys::size::control::height();
    let gap = sys::space::gap::xs();
    let paused = amane::shader_time_paused();

    let toggle = button(
        "lab.time",
        if paused {
            "▶ Run time"
        } else {
            "⏸ Hold time"
        },
        true,
        preview::toggle_time,
    );

    let mut first: Vec<Box<dyn Widget>> = vec![boxed(label("Time")), boxed(toggle)];

    if manifest.is_some_and(|manifest| manifest.animate.is_some()) {
        first.push(boxed(
            Switch::new("lab.animate", lab.animate)
                .on_toggle(|on| {
                    let mut lab = Lab::write();
                    lab.animate = on;
                    lab.started = std::time::Instant::now();
                })
                .build(),
        ));
        first.push(boxed(centered(caption("Sweep"))));
    }

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(Row::new(first).gap(12.0))];
    let mut height = control;

    if paused {
        let number = Number {
            id: "lab.time.at".into(),
            label: "At".into(),
            value: amane::shader_time(),
            min: 0.0,
            max: 120.0,
            step: 0.05,
            unit: "s".into(),
        };

        rows.push(boxed(number_row(number, |at| {
            amane::pause_shader_time(Some(at))
        })));
        height += control + gap;
    }

    (Column::new(rows).gap(gap), height)
}

/// The manifest's named sets of values; "Custom" while the values match none.
fn presets(manifest: &Manifest, lab: &Lab) -> Rectangle {
    let matches = |preset: &amane_kit::lab::manifest::ParamPreset| {
        let values = preset.values.iter().all(|(key, value)| {
            let param = manifest.params.iter().find(|param| &param.key == key);

            param.is_some_and(|param| (lab.value(manifest, param) - value).abs() < 0.01)
        });

        let colors = preset.colors.iter().all(|(key, hex)| {
            let param = manifest.colors.iter().find(|param| &param.key == key);

            param.is_some_and(|param| to_hex(lab.state.color(param)) == hex.to_lowercase())
        });

        values && colors
    };

    let chosen = manifest
        .presets
        .iter()
        .position(matches)
        .map_or(0, |index| index + 1);

    let names = std::iter::once("Custom".to_owned())
        .chain(manifest.presets.iter().map(|preset| preset.name.clone()))
        .collect();

    let presets = manifest.presets.clone();

    Dropdown::new("lab.param-preset", names, chosen)
        .width(WIDTH)
        .on_select(move |index| {
            let Some(preset) = index.checked_sub(1).and_then(|index| presets.get(index)) else {
                return;
            };

            change("preset", |state| {
                for (key, value) in &preset.values {
                    state.values.insert(key.clone(), *value);
                }

                for (key, hex) in &preset.colors {
                    state.colors.insert(key.clone(), hex.to_lowercase());
                }
            });
        })
        .build()
}

/// A parameter whose values have names, as a dropdown.
fn options_row(manifest: &Manifest, param: &Param, value: f32) -> Row {
    let chosen = (value.round().max(0.0) as usize).min(param.options.len() - 1);
    let key = param.key.clone();

    let id = format!("lab.{}.{}", manifest.name, param.key);

    Row::new(vec![
        boxed(label(&param.label)),
        boxed(
            Dropdown::new(id, param.options.clone(), chosen)
                .width(WIDTH - LABEL_WIDTH - 12.0)
                .on_select(move |index| {
                    change(&key, |state| {
                        state.values.insert(key.clone(), index as f32);
                    });
                })
                .build(),
        ),
    ])
    .gap(12.0)
}

/// A colour's swatch, which opens the picker, and its hex; the picker under
/// it while it's open.
fn color_row(manifest: &Manifest, param: &ColorParam, lab: &Lab) -> (Column, f32) {
    let color = lab.state.color(param);
    let id = format!("color.{}", param.key);
    let open = lab.picking.as_deref() == Some(id.as_str());

    let field_id = static_id(format!("lab.{}.{}", manifest.name, param.key));

    show(field_id, &to_hex(color));

    let key = param.key.clone();

    let typed = move |text: String| {
        if let Some(color) = parse_hex(&text) {
            change(&key, |state| {
                state.colors.insert(key.clone(), to_hex(color));
            });
        }
    };

    let row = Row::new(vec![
        boxed(label(&param.label)),
        boxed(swatch(&id, color, open)),
        boxed(field_box(
            field_id,
            text_field(field_id, "").on_submit(typed),
            FIELD_WIDTH + 12.0,
        )),
    ])
    .gap(12.0);

    let mut rows: Vec<Box<dyn Widget>> = vec![boxed(row)];
    let mut height = sys::size::control::height();

    if open {
        let key = param.key.clone();

        let picker = ColorPicker::new(format!("lab.picker.{}.{}", manifest.name, param.key), color)
            .width(WIDTH)
            .on_change(move |color| {
                change(&key, |state| {
                    state.colors.insert(key.clone(), to_hex(color));
                });
            })
            .build();

        rows.push(boxed(picker));
        height += picker_height(WIDTH) + sys::space::gap::xs();
    }

    (Column::new(rows).gap(sys::space::gap::xs()), height)
}

/// A colour to click: opens the picker `id` names, or closes it.
pub fn swatch(id: &str, color: [f32; 4], open: bool) -> Rectangle {
    let id = id.to_owned();
    let height = sys::size::control::height();

    let chip = Rectangle::new()
        .width(SLIDER_WIDTH - 40.0)
        .height(Size::Fixed(height * 0.7))
        .radius(sys::radius::raised())
        .fill(to_color(color))
        .border(
            if open { 2.0 } else { sys::border::hairline() },
            if open {
                sys::color::accent::bg()
            } else {
                material::separator()
            },
        );

    Rectangle::new()
        .width(SLIDER_WIDTH - 40.0)
        .height(Size::Fixed(height))
        .align_child(amane::Start, amane::Center)
        .cursor(amane::Pointer)
        .on_click(move |_| {
            let mut lab = Lab::write();

            lab.picking = if lab.picking.as_deref() == Some(id.as_str()) {
                None
            } else {
                Some(id.clone())
            };
        })
        .child(chip)
}

/// How tall kit's colour picker is at `width`.
pub fn picker_height(width: f32) -> f32 {
    let gap = sys::space::gap::s();
    let square = (width * 0.62).round();
    let strip = 14.0 + 4.0;
    let recent = 22.0;

    square + strip * 2.0 + sys::size::control::height() + recent + gap * 4.0
}

/// A number control's name, place and range.
pub struct Number {
    pub id: String,
    pub label: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub unit: String,
}

/// A slider over the range and a field that takes an exact number; the
/// wheel scrolls the panel, not the slider.
pub fn number_row(number: Number, set: impl Fn(f32) + Clone + 'static) -> Row {
    let Number {
        id,
        label: name,
        value,
        min,
        max,
        step,
        unit,
    } = number;

    let span = (max - min).max(f32::EPSILON);
    let position = ((value - min) / span * 100.0).round().clamp(0.0, 100.0) as u8;

    let field_id = static_id(id);

    show(field_id, &format_value(value, step, &unit));

    let snap = move |value: f32| {
        let snapped = if step > 0.0 {
            min + ((value - min) / step).round() * step
        } else {
            value
        };

        snapped.clamp(min, max)
    };

    let slid = {
        let set = set.clone();

        move |position: u8| set(snap(min + f32::from(position) / 100.0 * span))
    };

    let typed = move |text: String| {
        let number = text.trim().trim_end_matches(unit.as_str()).trim();

        if let Ok(value) = number.parse::<f32>() {
            set(value.clamp(min, max));
        }
    };

    Row::new(vec![
        boxed(label(&name)),
        boxed(
            Slider::new(position, SLIDER_WIDTH)
                .wheel(false)
                .on_value_change(slid)
                .build(),
        ),
        boxed(field_box(
            field_id,
            text_field(field_id, "").on_submit(typed),
            FIELD_WIDTH,
        )),
    ])
    .gap(12.0)
}

/// An effect param as a number row, each change one undo step per param.
fn param_row(manifest: &Manifest, param: &Param, value: f32) -> Row {
    let number = Number {
        id: format!("lab.{}.{}", manifest.name, param.key),
        label: param.label.clone(),
        value,
        min: param.min,
        max: param.max,
        step: param.step,
        unit: param.unit.clone(),
    };

    let key = param.key.clone();

    number_row(number, move |value| {
        change(&key, |state| {
            state.values.insert(key.clone(), value);
        });
    })
}

fn actions(manifest: &Manifest, lab: &Lab, dir: &std::path::Path) -> Row {
    let snippet = rust_snippet(manifest, lab, dir);

    Row::new(vec![
        boxed(button("lab.undo", "Undo", lab.history.can_undo(), undo)),
        boxed(button("lab.redo", "Redo", lab.history.can_redo(), redo)),
        boxed(button("lab.reset", "Reset", true, || {
            change("reset", |state| {
                state.values.clear();
                state.colors.clear();
                state.gradient = Default::default();
            });
        })),
        boxed(button("lab.copy", "Copy as Rust", true, move || {
            Clipboard::copy(snippet.clone())
        })),
    ])
    .gap(sys::space::gap::xs())
}

/// The effect as it's set now, to paste into the shell.
fn rust_snippet(manifest: &Manifest, lab: &Lab, dir: &std::path::Path) -> String {
    let values = lab.rows(manifest);

    // a rectangle shader, as the shell's glass is drawn
    if let Some(shader) = &manifest.shader {
        let mut text = format!(
            ".shader({:?})\n.shader_values(vec![\n",
            dir.join(shader).display().to_string()
        );

        for row in values {
            let _ = writeln!(
                text,
                "    [{:?}, {:?}, {:?}, {:?}],",
                row[0], row[1], row[2], row[3]
            );
        }

        text.push_str("])");

        return text;
    }

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

fn button(id: &str, text: &str, enabled: bool, press: impl Fn() + 'static) -> Rectangle {
    let button = PushButton::new(id.to_owned(), text.to_owned());

    if enabled {
        button.on_press(press).build()
    } else {
        button.build().opacity(0.4)
    }
}

/// A setting's name over its control.
fn setting(name: &str, control: impl Widget + 'static) -> Column {
    Column::new(vec![boxed(caption(name)), boxed(control)]).gap(sys::space::gap::xs())
}

pub fn caption(text: &str) -> Text {
    Text::new(text)
        .style(sys::r#type::caption())
        .color(material::label(Level::Secondary))
}

pub fn caption_height() -> f32 {
    text_height(sys::r#type::caption())
}

/// A caption across the panel, wrapped.
pub fn note(text: &str) -> Rectangle {
    Rectangle::new()
        .width(WIDTH)
        .height(Size::Fixed(sys::size::control::height() * 1.5))
        .align_child(amane::Start, amane::Center)
        .child(caption(text).wrap().max_lines(2).elide())
}

/// Lined up with the controls beside it.
fn centered(text: Text) -> Rectangle {
    Rectangle::new()
        .width(56.0)
        .height(Size::Fixed(sys::size::control::height()))
        .align_child(amane::Start, amane::Center)
        .child(text)
}

pub fn label(text: &str) -> Rectangle {
    label_width(text, LABEL_WIDTH)
}

fn label_width(text: &str, width: f32) -> Rectangle {
    Rectangle::new()
        .width(width)
        .height(Size::Fixed(sys::size::control::height()))
        .align_child(amane::Start, amane::Center)
        .child(
            Text::new(text)
                .style(sys::r#type::label())
                .color(material::label(Level::Primary))
                .elide(),
        )
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

/// Puts the value in the field, unless it's being typed into.
fn show(id: &'static str, value: &str) {
    if !TextInput::has_focus(id) && TextInput::text(id) != value {
        TextInput::set_text(id, value);
    }
}

/// As many decimals as the step has, so 0.075 doesn't show as 0.1.
fn format_value(value: f32, step: f32, unit: &str) -> String {
    let decimals = if step <= 0.0 {
        2
    } else if step >= 1.0 {
        0
    } else {
        (-step.log10()).ceil().clamp(1.0, 4.0) as usize
    };

    format!("{value:.decimals$}{unit}")
}

/// A text field's id has to be `'static`; there's one per control, made once.
fn static_id(id: String) -> &'static str {
    thread_local! {
        static IDS: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
    }

    IDS.with_borrow_mut(|ids| {
        *ids.entry(id.clone())
            .or_insert_with(|| Box::leak(id.into_boxed_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_as_many_decimals_as_the_step() {
        assert_eq!(format_value(0.075, 0.005, ""), "0.075");
        assert_eq!(format_value(1.55, 0.05, ""), "1.55");
        assert_eq!(format_value(7.5, 0.5, ""), "7.5");
        assert_eq!(format_value(100.0, 1.0, "%"), "100%");
    }
}
