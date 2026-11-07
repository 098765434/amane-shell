//! A control for each setting the registry lists on a page, from the
//! snapshot's entry: a switch, a slider over a range, a dropdown of choices,
//! or a field with Set. A change goes to `managed/settings.json` and waits
//! for Apply; a setting another file sets is shown locked, with where.

use amane::{Row, Service, TextInput};
use amane_kit::app::{commit_on_blur, field_box, text_field};
use amane_kit::buttons::{PushButton, Switch};
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::layout::boxed;
use amane_kit::stepper::Stepper;
use amane_kit::style;
use serde_json::Value;

use super::adopt;
use super::nixos::{Control, Entry, Managed, Snapshot, Tier, show, show_as};

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

/// A `'static` id for a stepper's field, made once per setting.
fn stepper_id(id: &str) -> &'static str {
    thread_local! {
        static IDS: std::cell::RefCell<std::collections::HashMap<String, &'static str>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    IDS.with_borrow_mut(|ids| {
        *ids.entry(id.to_string())
            .or_insert_with(|| Box::leak(format!("settings.system.stepper.{id}").into_boxed_str()))
    })
}

/// What a row says under a setting's name: what Set refused, where else
/// it is set (locked), or what it was while a change waits; and the Move to
/// Settings… button when the setting is set by hand somewhere it can be
/// moved from.
pub fn summary(
    entry: &Entry,
    managed: &Managed,
    feedback: &Feedback,
) -> (String, Option<PushButton>) {
    let pending = managed.current.get(&entry.id) != managed.committed.get(&entry.id);
    let mut parts: Vec<String> = Vec::new();
    let mut button = None;

    if let Some((id, why)) = &feedback.error
        && *id == entry.id
    {
        parts.push(format!("Not set: {why}"));
    }
    if !entry.managed() {
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
        parts.push(format!(
            "Set in {}{}",
            places.join(", "),
            if movable {
                ""
            } else {
                "; edit it by hand to change it here"
            }
        ));
        if movable {
            let e = entry.clone();
            button = Some(
                PushButton::new(
                    format!("settings.system.move.{}", entry.id),
                    "Move to Settings…",
                )
                .on_press(move || adopt::confirm(&e)),
            );
        }
    } else if pending {
        let was = managed.committed.get(&entry.id).map_or_else(
            || show_as(&entry.control, &entry.default),
            |v| show_as(&entry.control, v),
        );
        parts.push(format!("Was {was} · waiting for Apply"));
    } else if entry.tier == Tier::Restart {
        // "after Apply" goes for every row, and the page says it once
        parts.push(entry.tier.note().to_string());
    }
    (parts.join(" · "), button)
}

/// The row for one registry setting: its control at the trailing edge,
/// what [`summary`] says under its name.
pub fn row(entry: &Entry, managed: &Managed, feedback: &Feedback) -> Option<FormRow> {
    let value = managed.value(&entry.id, &entry.value).clone();
    let locked = !entry.managed();
    let (subtitle, mover) = summary(entry, managed, feedback);
    let row = FormRow::new(
        format!("settings.system.row.{}", entry.id),
        entry.label.clone(),
    )
    .subtitle(subtitle);

    let row = match &entry.control {
        // the Apps page draws the list itself
        Control::Packages => return None,
        Control::Switch => row.trailing(switch(entry, managed).build()),
        Control::Int { min, max } => {
            let (min, max) = (*min, *max);
            let now = value.as_i64().unwrap_or(min).clamp(min, max);
            let e = entry.clone();
            row.trailing(
                Stepper::new(stepper_id(&entry.id), now as f64)
                    .range(min as f64, max as f64)
                    .width(80.0)
                    .disabled(locked)
                    .on_change(move |value| set(&e, Value::from(value.round() as i64)))
                    .build(),
            )
        }
        Control::Choice(choices) => {
            let labels = choices.iter().map(|c| c.label.clone()).collect();
            let selected = choices.iter().position(|c| c.value == value).unwrap_or(0);
            let e = entry.clone();
            let values: Vec<Value> = choices.iter().map(|c| c.value.clone()).collect();
            row.trailing(
                Dropdown::new(
                    format!("settings.system.choice.{}", entry.id),
                    labels,
                    selected,
                )
                .disabled(locked)
                .on_select(move |i| {
                    if let Some(value) = values.get(i) {
                        set(&e, value.clone());
                    }
                })
                .build(),
            )
        }
        Control::TimeZone | Control::Text => {
            let id = field_id(&entry.id);
            let shown = show(&value);
            if TextInput::text(id).is_empty() && !shown.is_empty() {
                TextInput::set_text(id, &shown);
            }
            // kept on Return, or when the field is left with something new
            let e = entry.clone();
            let kept = shown.clone();
            let keep = move |text: String| {
                let text = text.trim().to_string();
                if !locked && text != kept.trim() {
                    set(&e, Value::from(text));
                }
            };
            commit_on_blur(id, keep.clone());
            let field = text_field(id, &shown).on_submit(keep);
            row.trailing(field_box(id, field, 200.0))
        }
    };

    Some(match mover {
        Some(button) => row.detail(
            Row::new(vec![boxed(button.build())]),
            style::control_height(),
        ),
        None => row,
    })
}

/// A row for each setting the registry lists on `page`.
pub fn rows(page: &str) -> Vec<FormRow> {
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let feedback = Feedback::read();
    let rows: Vec<FormRow> = snapshot
        .for_page(page)
        .into_iter()
        .filter_map(|entry| row(entry, &managed, &feedback))
        .collect();
    if !rows.is_empty() {
        return rows;
    }
    // none yet: say why, so the page isn't blank
    if snapshot.loading {
        return vec![
            FormRow::new(
                format!("settings.system.{page}.loading"),
                "Reading the configuration…",
            )
            .loading(),
        ];
    }
    match &snapshot.error {
        Some(error) => vec![crate::problem_row(
            &format!("settings.system.{page}.error"),
            "The configuration couldn't be read",
            "Its settings show here once it can be. Refresh on About This Computer tries again.",
            error,
        )],
        None => rows,
    }
}
