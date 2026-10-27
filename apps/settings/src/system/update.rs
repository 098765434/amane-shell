//! Software Update: the flake's inputs from `flake.lock`, each with when it
//! was last updated; Check for Updates (all, or one input) builds the
//! system from a fresh lock and shows what changes before anything is
//! written. Applying it puts the lock in the repo and commits it.

use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use amane::{Center, Column, Row, Service, Size, Widget, watch_file};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::card::Card;
use amane_kit::icons::{ICON_CHECK, ICON_UPDATE};
use amane_kit::layout::{boxed, icon};
use amane_kit::list::{List, ListItem, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};
use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use super::apply_bar::Overlay;
use super::job::{BuildJob, Phase, Target};
use super::nixos::repo;
use super::repo::short;
use super::{controls, state_dir};
use crate::{Navigation, PAGE_WIDTH, note};

#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub name: String,
    pub rev: Option<String>,
    pub source: Option<String>,
    pub modified: Option<DateTime<Utc>>,
}

impl Input {
    /// "12 days ago", "today", "3 months ago".
    pub fn ago(&self) -> String {
        let Some(when) = self.modified else {
            return String::from("unknown");
        };
        let days = (Local::now().with_timezone(&Utc) - when).num_days();
        match days {
            i64::MIN..=0 => String::from("today"),
            1 => String::from("yesterday"),
            2..=13 => format!("{days} days ago"),
            14..=60 => format!("{} weeks ago", days / 7),
            61..=730 => format!("{} months ago", days / 30),
            _ => format!("{} years ago", days / 365),
        }
    }
}

/// The root's inputs from a `flake.lock`.
pub fn parse_lock(json: &str) -> Result<Vec<Input>, String> {
    let lock: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let nodes = lock
        .get("nodes")
        .and_then(Value::as_object)
        .ok_or("no nodes")?;
    let root = lock.get("root").and_then(Value::as_str).unwrap_or("root");
    let inputs = nodes
        .get(root)
        .and_then(|node| node.get("inputs"))
        .and_then(Value::as_object)
        .ok_or("no root inputs")?;

    let mut list: Vec<Input> = inputs
        .iter()
        .map(|(name, key)| {
            // a follows is a path of names; the last is the node
            let key = match key {
                Value::String(key) => Some(key.clone()),
                Value::Array(path) => path.last().and_then(Value::as_str).map(String::from),
                _ => None,
            };
            let locked = key
                .as_deref()
                .and_then(|key| nodes.get(key))
                .and_then(|node| node.get("locked"));
            let text = |field: &str| {
                locked
                    .and_then(|l| l.get(field))
                    .and_then(Value::as_str)
                    .map(String::from)
            };
            let source = match (text("owner"), text("repo")) {
                (Some(owner), Some(repo)) => Some(format!("{owner}/{repo}")),
                _ => text("url").or_else(|| text("type")),
            };
            Input {
                name: name.clone(),
                rev: text("rev"),
                source,
                modified: locked
                    .and_then(|l| l.get("lastModified"))
                    .and_then(Value::as_i64)
                    .and_then(|secs| DateTime::from_timestamp(secs, 0)),
            }
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

/// An input a fresh lock moves.
#[derive(Clone, Debug, PartialEq)]
pub struct InputChange {
    pub name: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl InputChange {
    /// "nixpkgs 26f4107 → 1a2b3c4".
    pub fn text(&self) -> String {
        let rev = |rev: &Option<String>| rev.as_deref().map_or(String::from("∅"), short);
        format!("{} {} → {}", self.name, rev(&self.from), rev(&self.to))
    }
}

/// The root inputs whose commit differs between two locks.
pub fn diff_locks(old: &str, new: &str) -> Result<Vec<InputChange>, String> {
    let old = parse_lock(old).unwrap_or_default();
    let new = parse_lock(new)?;
    Ok(new
        .iter()
        .filter_map(|input| {
            let before = old.iter().find(|o| o.name == input.name);
            let from = before.and_then(|o| o.rev.clone());
            (from != input.rev).then(|| InputChange {
                name: input.name.clone(),
                from,
                to: input.rev.clone(),
            })
        })
        .collect())
}

/// The commit's subject for an update.
pub fn message(changes: &[InputChange]) -> String {
    format!(
        "update: {}",
        changes
            .iter()
            .map(InputChange::text)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub struct Lock {
    pub inputs: Vec<Input>,
    pub error: Option<String>,
}

/// What the scheduled check (`amane-update-check`, a user timer from the
/// configuration) last found, from `check.json` in the state dir.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Check {
    pub checked: Option<DateTime<Utc>>,
    pub changes: Vec<InputChange>,
    pub error: Option<String>,
    pub present: bool,
}

fn check_path() -> PathBuf {
    state_dir().join("check.json")
}

/// `check.json` as the checker writes it.
pub fn parse_check(json: &str) -> Check {
    let value: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let text = |change: &Value, key: &str| change.get(key)?.as_str().map(String::from);
    Check {
        checked: value
            .get("checked")
            .and_then(Value::as_i64)
            .and_then(|secs| DateTime::from_timestamp(secs, 0)),
        changes: value
            .get("changes")
            .and_then(Value::as_array)
            .map(|changes| {
                changes
                    .iter()
                    .filter_map(|change| {
                        Some(InputChange {
                            name: text(change, "name")?,
                            from: text(change, "from"),
                            to: text(change, "to"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        error: value.get("error").and_then(Value::as_str).map(String::from),
        present: value.is_object(),
    }
}

fn look_check() -> Check {
    fs::read_to_string(check_path())
        .map(|json| parse_check(&json))
        .unwrap_or_default()
}

impl Service for Check {
    fn new() -> Self {
        look_check()
    }

    fn listen() {
        loop {
            let path = check_path();
            if !path.parent().is_some_and(|folder| folder.is_dir()) {
                thread::sleep(Duration::from_secs(30));
                continue;
            }
            for _ in watch_file(&path.to_string_lossy()) {
                let fresh = look_check();
                let mut now = Self::write();
                if fresh == *now {
                    now.quiet();
                } else {
                    *now = fresh;
                }
            }
        }
    }
}

impl Check {
    /// Records that nothing waits any more (an update was applied, or a
    /// check found everything current), so the tile and the card clear
    /// before the timer's next run.
    pub fn clear() {
        let path = check_path();
        let now = Utc::now().timestamp();
        let json = format!("{{\n  \"checked\": {now},\n  \"changes\": []\n}}\n");
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        let _ = fs::write(path, json);
    }

    /// "2 hours ago", "yesterday".
    fn ago(&self) -> String {
        let Some(when) = self.checked else {
            return String::from("never");
        };
        let minutes = (Utc::now() - when).num_minutes();
        match minutes {
            i64::MIN..=1 => String::from("just now"),
            2..=59 => format!("{minutes} minutes ago"),
            60..=119 => String::from("an hour ago"),
            120..=1439 => format!("{} hours ago", minutes / 60),
            1440..=2879 => String::from("yesterday"),
            _ => format!("{} days ago", minutes / 1440),
        }
    }
}

fn lock_path() -> PathBuf {
    repo().join("flake.lock")
}

fn look() -> Lock {
    match fs::read_to_string(lock_path()) {
        Ok(json) => match parse_lock(&json) {
            Ok(inputs) => Lock {
                inputs,
                error: None,
            },
            Err(error) => Lock {
                inputs: Vec::new(),
                error: Some(format!("flake.lock: {error}")),
            },
        },
        Err(error) => Lock {
            inputs: Vec::new(),
            error: Some(format!("flake.lock: {error}")),
        },
    }
}

impl Service for Lock {
    fn new() -> Self {
        look()
    }

    fn listen() {
        loop {
            let path = lock_path();
            if !path.parent().is_some_and(|folder| folder.is_dir()) {
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            for _ in watch_file(&path.to_string_lossy()) {
                *Self::write() = look();
            }
        }
    }
}

/// What a check found, while its job is around; else what the scheduled
/// check last found.
fn result_card(job: &BuildJob, check: &Check) -> Option<Box<dyn Widget>> {
    let (glyph, text, button) = match (&job.target, &job.phase) {
        (Target::Update { .. }, Phase::UpToDate) => {
            (ICON_CHECK, String::from("Everything is up to date."), None)
        }
        (Target::Update { changes, .. }, Phase::Built(built)) => (
            ICON_UPDATE,
            format!(
                "Update ready: {} change{} · {} package{} change{}",
                changes.len(),
                if changes.len() == 1 { "" } else { "s" },
                built.diff.len(),
                if built.diff.len() == 1 { "" } else { "s" },
                if built.needs_restart {
                    " · needs a restart"
                } else {
                    ""
                }
            ),
            Some(("Show Changes", Button::Changes)),
        ),
        (Target::Update { .. }, _) if job.busy() => return None,
        _ if !check.present => return None,
        _ if !check.changes.is_empty() => (
            ICON_UPDATE,
            format!(
                "Checked {} · updates for {}",
                check.ago(),
                check
                    .changes
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some(("Update All", Button::UpdateAll)),
        ),
        _ => (
            ICON_CHECK,
            match &check.error {
                Some(error) => format!("Checked {} · couldn't: {error}", check.ago()),
                None => format!("Checked {} · up to date", check.ago()),
            },
            None,
        ),
    };

    let mut row: Vec<Box<dyn Widget>> = vec![
        boxed(icon(
            glyph,
            sys::size::icon::s(),
            material::label(Level::Secondary),
        )),
        boxed(
            amane::Text::new(text)
                .style(sys::r#type::label())
                .color(material::label(Level::Primary))
                .elide(),
        ),
    ];
    if let Some((label, what)) = button {
        let running = job.running();
        row.push(boxed(
            PushButton::new("settings.update.card", label)
                .style(Style::Plain)
                .disabled(running && what == Button::UpdateAll)
                .on_press(move || match what {
                    Button::Changes => Navigation::write().overlay = Some(Overlay::Changes),
                    Button::UpdateAll => BuildJob::start_update(Vec::new()),
                })
                .build(),
        ));
    }

    // a card fills its parent: one row tall
    Some(boxed(
        amane::Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(
                sys::size::row::height() + 2.0 * sys::space::inset::m(),
            ))
            .fill(amane::Color::TRANSPARENT)
            .child(
                Card::new(vec![boxed(
                    Row::new(row).gap(sys::space::gap::m()).align(Center),
                )])
                .padding(sys::space::inset::m())
                .build(),
            ),
    ))
}

/// What the card's button does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    Changes,
    UpdateAll,
}

pub fn page() -> Column {
    let lock = Lock::read();
    let job = BuildJob::read();
    let check = Check::read();
    let running = job.running();
    // the inputs a job moved, else what the scheduled check saw move
    let moved: Vec<&InputChange> = match &job.target {
        Target::Update { changes, .. } if job.busy() => changes.iter().collect(),
        _ => check.changes.iter().collect(),
    };

    let list = with_row_width(PAGE_WIDTH, || {
        let items: Vec<ListItem> = lock
            .inputs
            .iter()
            .map(|input| {
                let detail = match moved.iter().find(|c| c.name == input.name) {
                    Some(change) => format!(
                        "{} → {}",
                        change.from.as_deref().map_or(String::from("∅"), short),
                        change.to.as_deref().map_or(String::from("∅"), short)
                    ),
                    None => format!(
                        "{}{}",
                        input.ago(),
                        input
                            .rev
                            .as_deref()
                            .map_or(String::new(), |rev| format!(" · {}", short(rev)))
                    ),
                };
                let name = input.name.clone();
                ListItem::new(
                    format!("settings.update.{}", input.name),
                    format!("{} · {detail}", input.name),
                )
                .bare(true)
                .trailing(
                    PushButton::new(format!("settings.update.one.{}", input.name), "Update")
                        .style(Style::Plain)
                        .disabled(running)
                        .on_press(move || BuildJob::start_update(vec![name.clone()]))
                        .build(),
                )
            })
            .collect();
        List::new(items)
            .header("Inputs")
            .empty(
                lock.error
                    .clone()
                    .unwrap_or_else(|| String::from("No inputs.")),
            )
            .build()
    });

    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(
        Row::new(vec![
            boxed(
                PushButton::new("settings.update.check", "Check for Updates")
                    .style(Style::Filled)
                    .disabled(running)
                    .on_press(|| BuildJob::start_update(Vec::new()))
                    .build(),
            ),
            boxed(note(
                "Builds the updated system first; nothing is written until you apply it.",
            )),
        ])
        .gap(sys::space::gap::m())
        .align(Center),
    )];
    parts.extend(result_card(&job, &check));
    parts.push(boxed(list));
    parts.push(boxed(note(
        "Update on one input bumps just that one. Applying writes flake.lock and commits it with the settings.",
    )));
    parts.extend(controls::for_page("update"));
    parts.push(boxed(note(
        "The automatic check relocks beside the repository and notifies; it never builds or applies.",
    )));

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_lists_the_roots_inputs_with_their_commits() {
        let json = r#"{"nodes":{
            "nixpkgs":{"locked":{"lastModified":1760000000,"owner":"NixOS","repo":"nixpkgs","rev":"26f4107e47e3ad0ffd0db220469ba83b6f6d96e8","type":"github"}},
            "home-manager":{"inputs":{"nixpkgs":["nixpkgs"]},"locked":{"lastModified":1760100000,"owner":"nix-community","repo":"home-manager","rev":"abcdef1234567","type":"github"}},
            "root":{"inputs":{"nixpkgs":"nixpkgs","home-manager":"home-manager","hm2":["home-manager"]}}
        },"root":"root","version":7}"#;
        let inputs = parse_lock(json).unwrap();
        assert_eq!(inputs.len(), 3);
        assert_eq!(inputs[0].name, "hm2");
        assert_eq!(inputs[0].rev.as_deref(), Some("abcdef1234567"));
        assert_eq!(inputs[2].source.as_deref(), Some("NixOS/nixpkgs"));
        assert!(inputs[2].modified.is_some());
    }

    #[test]
    fn a_scheduled_check_reads_back() {
        let check = parse_check(
            r#"{"checked": 1761500000, "changes": [{"name": "nixpkgs", "from": "1111111aaaaaaa", "to": "2222222bbbbbbb"}]}"#,
        );
        assert!(check.present);
        assert_eq!(check.changes.len(), 1);
        assert_eq!(check.changes[0].text(), "nixpkgs 1111111 → 2222222");
        assert!(check.checked.is_some());
        let none = parse_check("");
        assert!(!none.present && none.changes.is_empty());
        assert_eq!(
            parse_check(r#"{"checked": 1, "changes": [], "error": "offline"}"#)
                .error
                .as_deref(),
            Some("offline")
        );
    }

    #[test]
    fn two_locks_differ_by_input_and_read_as_a_commit_subject() {
        let old = r#"{"nodes":{"a":{"locked":{"rev":"1111111aaaaaaa"}},"b":{"locked":{"rev":"2222222bbbbbbb"}},
            "root":{"inputs":{"a":"a","b":"b"}}},"root":"root","version":7}"#;
        let new = r#"{"nodes":{"a":{"locked":{"rev":"3333333aaaaaaa"}},"b":{"locked":{"rev":"2222222bbbbbbb"}},
            "root":{"inputs":{"a":"a","b":"b"}}},"root":"root","version":7}"#;
        let changes = diff_locks(old, new).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text(), "a 1111111 → 3333333");
        assert_eq!(message(&changes), "update: a 1111111 → 3333333");
        assert!(diff_locks(new, new).unwrap().is_empty());
        assert!(diff_locks("", new).unwrap().len() == 2);
    }
}
