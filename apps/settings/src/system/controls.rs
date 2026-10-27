//! A control for each setting the registry lists on a page, from the
//! snapshot's entry: a switch, a slider over a range, or a field with Set.
//! A change goes to `managed/settings.json` and waits for Apply; a setting
//! another file sets is shown locked, with where.

use amane::{Column, Row, Service, TextInput, Widget};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Switch};
use amane_kit::layout::boxed;
use amane_kit::slider::Slider;
use amane_kit::tokens::sys;
use serde_json::Value;

use super::nixos::{Control, Entry, Managed, Snapshot, show};
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
fn set(entry: &Entry, value: Value) {
    match entry.check(&value) {
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
        _ => "settings.system.field.other",
    }
}

pub fn for_page(page: &str) -> Vec<Box<dyn Widget>> {
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let feedback = Feedback::read();

    snapshot
        .for_page(page)
        .into_iter()
        .map(|entry| {
            let value = managed.value(&entry.id, &entry.value).clone();
            let locked = !entry.managed();
            let pending = managed.current.get(&entry.id) != managed.committed.get(&entry.id);

            let mut lines: Vec<Box<dyn Widget>> = Vec::new();
            let mut heading = true;

            match &entry.control {
                Control::Switch => {
                    let on = value.as_bool().unwrap_or(false);
                    let e = entry.clone();
                    lines.push(boxed(labelled(
                        &entry.label,
                        Switch::new(format!("settings.system.{}", entry.id), on)
                            .disabled(locked)
                            .on_toggle(move |on| set(&e, Value::Bool(on)))
                            .build(),
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

            if let Some((id, why)) = &feedback.error
                && *id == entry.id
            {
                lines.push(boxed(
                    note(why.clone()).color(sys::color::status::critical::text()),
                ));
            }
            if locked {
                lines.push(boxed(note(format!(
                    "Set in {}: move it to managed/settings.json to change it here.",
                    entry.set_elsewhere().join(", ")
                ))));
            } else if pending {
                let was = managed
                    .committed
                    .get(&entry.id)
                    .map_or_else(|| show(&entry.default), show);
                lines.push(boxed(note(format!(
                    "Was {was} · waiting for Apply. {}",
                    entry.tier.note()
                ))));
            } else {
                lines.push(boxed(note(entry.tier.note())));
            }

            let column = Column::new(lines).gap(sys::space::gap::xs());
            if heading {
                boxed(setting(&entry.label, column))
            } else {
                boxed(column)
            }
        })
        .collect()
}
