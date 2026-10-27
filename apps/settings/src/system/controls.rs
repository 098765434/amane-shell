//! A control for each setting the registry lists on a page, from the
//! snapshot's entry: a switch, a slider over a range, a dropdown of choices,
//! or a field with Set. A change goes to `managed/settings.json` and waits
//! for Apply; a setting another file sets is shown locked, with where.

use amane::{Column, Row, Service, TextInput, Widget};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Style, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::layout::boxed;
use amane_kit::slider::Slider;
use amane_kit::tokens::sys;
use serde_json::Value;

use super::adopt;
use super::nixos::{Control, Entry, Managed, Snapshot, show, show_as};
use crate::{labelled, note, setting};

/// What the last Set said when it refused a value.
pub struct Feedback {
    pub error: Option<(String, String)>,
}

impl Service for Feedback {
    fn new() -> Self {
        Self { error: None }
    }

    fn listen() {}
}

/// Checks the value against its entry and writes it, or says why not.
pub fn set(entry: &Entry, value: Value) {
    // a keyboard variant is one of its layout's
    let layout = {
        let (snapshot, managed) = (Snapshot::read(), Managed::read());
        snapshot
            .entry("keyboardLayout")
            .map(|e| show(managed.value(&e.id, &e.value)))
            .unwrap_or_default()
    };
    match entry.check_with(&value, &layout) {
        Ok(()) => {
            Feedback::write().error = None;
            Managed::set(&entry.id, value);
        }
        Err(why) => Feedback::write().error = Some((entry.id.clone(), why)),
    }
}

/// The field's id, which has to live for the program: a page has few.
fn field_id(id: &str) -> &'static str {
    match id {
        "timeZone" => "settings.system.field.timeZone",
        "hostName" => "settings.system.field.hostName",
        "keyboardLayout" => "settings.system.field.keyboardLayout",
        "keyboardVariant" => "settings.system.field.keyboardVariant",
        _ => "settings.system.field.other",
    }
}

/// The lines under a control: what Set refused, where else the setting is
/// set (locked), what it was while a change waits, or when it takes effect.
pub fn notes(entry: &Entry, managed: &Managed, feedback: &Feedback) -> Vec<Box<dyn Widget>> {
    let mut lines: Vec<Box<dyn Widget>> = Vec::new();
    let pending = managed.current.get(&entry.id) != managed.committed.get(&entry.id);

    if let Some((id, why)) = &feedback.error
        && *id == entry.id
    {
        lines.push(boxed(
            note(why.clone()).color(sys::color::status::critical::text()),
        ));
    }
    if !entry.managed() {
        // where, to the line when nix could tell, and Move to Settings… when
        // the line is a plain `name = value;`
        let places: Vec<String> = match entry.set_at.as_slice() {
            [] => entry
                .set_elsewhere()
                .iter()
                .map(|f| f.to_string())
                .collect(),
            at => at
                .iter()
                .map(|(file, line)| format!("{file}:{line}"))
                .collect(),
        };
        let movable = adopt::plan(entry).is_ok();
        let e = entry.clone();
        let mut row: Vec<Box<dyn Widget>> = vec![boxed(note(format!(
            "Set in {}{}",
            places.join(", "),
            if movable {
                ""
            } else {
                "; edit it by hand to change it here"
            }
        )))];
        if movable {
            row.push(boxed(
                PushButton::new(
                    format!("settings.system.move.{}", entry.id),
                    "Move to Settings…",
                )
                .style(Style::Plain)
                .on_press(move || adopt::confirm(&e))
                .build(),
            ));
        }
        lines.push(boxed(
            Row::new(row).gap(sys::space::gap::m()).align(amane::Center),
        ));
    } else if pending {
        let was = managed.committed.get(&entry.id).map_or_else(
            || show_as(&entry.control, &entry.default),
            |v| show_as(&entry.control, v),
        );
        lines.push(boxed(note(format!(
            "Was {was} · waiting for Apply. {}",
            entry.tier.note()
        ))));
    } else {
        lines.push(boxed(note(entry.tier.note())));
    }
    lines
}

/// The switch for a boolean entry, by itself.
pub fn switch(entry: &Entry, managed: &Managed) -> Switch {
    let on = managed
        .value(&entry.id, &entry.value)
        .as_bool()
        .unwrap_or(false);
    let e = entry.clone();
    Switch::new(format!("settings.system.{}", entry.id), on)
        .disabled(!entry.managed())
        .on_toggle(move |on| set(&e, Value::Bool(on)))
}

pub fn for_page(page: &str) -> Vec<Box<dyn Widget>> {
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let feedback = Feedback::read();

    snapshot
        .for_page(page)
        .into_iter()
        .filter_map(|entry| {
            let value = managed.value(&entry.id, &entry.value).clone();
            let locked = !entry.managed();

            let mut lines: Vec<Box<dyn Widget>> = Vec::new();
            let mut heading = true;

            match &entry.control {
                // the Apps page draws the list itself
                Control::Packages => return None,
                Control::Switch => {
                    lines.push(boxed(labelled(
                        &entry.label,
                        switch(entry, &managed).build(),
                    )));
                    heading = false;
                }
                Control::Int { min, max } => {
                    let (min, max) = (*min, *max);
                    let now = value.as_i64().unwrap_or(min).clamp(min, max);
                    let span = (max - min).max(1);
                    let percent = ((now - min) * 100 / span) as u8;
                    let e = entry.clone();
                    lines.push(boxed(
                        Row::new(vec![
                            boxed(
                                Slider::new(percent, 240.0)
                                    .inert(locked)
                                    .wheel(false)
                                    .on_value_change(move |percent| {
                                        let value = min + (i64::from(percent) * span + 50) / 100;
                                        set(&e, Value::from(value.clamp(min, max)));
                                    })
                                    .build(),
                            ),
                            boxed(note(now.to_string())),
                        ])
                        .gap(sys::space::gap::m())
                        .align(amane::Center),
                    ));
                }
                Control::Choice(choices) => {
                    let labels = choices.iter().map(|c| c.label.clone()).collect();
                    let selected = choices.iter().position(|c| c.value == value).unwrap_or(0);
                    let e = entry.clone();
                    let values: Vec<Value> = choices.iter().map(|c| c.value.clone()).collect();
                    lines.push(boxed(
                        Dropdown::new(
                            format!("settings.system.choice.{}", entry.id),
                            labels,
                            selected,
                        )
                        .width(240.0)
                        .disabled(locked)
                        .on_select(move |i| {
                            if let Some(value) = values.get(i) {
                                set(&e, value.clone());
                            }
                        })
                        .build(),
                    ));
                }
                Control::TimeZone | Control::Text => {
                    let id = field_id(&entry.id);
                    let shown = show(&value);
                    if TextInput::text(id).is_empty() && !shown.is_empty() {
                        TextInput::set_text(id, &shown);
                    }
                    let e = entry.clone();
                    let e2 = entry.clone();
                    let field = text_field(id, &shown).on_submit(move |text| {
                        set(&e, Value::from(text.trim()));
                    });
                    lines.push(boxed(
                        Row::new(vec![
                            boxed(field_box(id, field, 240.0)),
                            boxed(
                                PushButton::new(format!("settings.system.set.{}", entry.id), "Set")
                                    .disabled(locked)
                                    .on_press(move || {
                                        set(&e2, Value::from(TextInput::text(id).trim()));
                                    })
                                    .build(),
                            ),
                        ])
                        .gap(sys::space::gap::s())
                        .align(amane::Center),
                    ));
                }
            }

            lines.extend(notes(entry, &managed, &feedback));

            let column = Column::new(lines).gap(sys::space::gap::xs());
            Some(if heading {
                boxed(setting(&entry.label, column))
            } else {
                boxed(column)
            })
        })
        .collect()
}
