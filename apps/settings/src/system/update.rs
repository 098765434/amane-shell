//! Software Update: the flake's inputs from `flake.lock`, each with when it
//! was last updated; Check for Updates (all, or one input) builds the
//! system from a fresh lock and shows what changes before anything is
//! written. Applying it puts the lock in the repo and commits it.

use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use amane::{Column, Service, watch_file};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::color::Accent;
use amane_kit::form::FormRow;
use amane_kit::icons::{ICON_CHECK, ICON_UPDATE};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::job::{BuildJob, Phase, Target};
use super::nixos::repo;
use super::repo::short;
use super::{controls, state_dir};
use crate::View;
use crate::{group, groups};

pub use amane_nixos::lock::{Input, InputChange, message, parse_lock};

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
fn result_card(job: &BuildJob, check: &Check) -> Option<FormRow> {
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

    let accent = if glyph == ICON_CHECK {
        Accent::Green
    } else {
        Accent::Blue
    };
    let mut row = FormRow::new("settings.update.result", text).icon(glyph, accent);
    if let Some((label, what)) = button {
        let running = job.running();
        row = row.trailing(
            PushButton::new("settings.update.card", label)
                .disabled(running && what == Button::UpdateAll)
                .on_press(move || match what {
                    Button::Changes => crate::open(View::Changes),
                    Button::UpdateAll => BuildJob::start_update(Vec::new()),
                })
                .build(),
        );
    }
    Some(row)
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

    let inputs = lock.inputs.iter().map(|input| {
        let detail = match moved.iter().find(|c| c.name == input.name) {
            Some(change) => format!(
                "{} → {}",
                change
                    .from
                    .as_deref()
                    .map_or(String::from("not pinned"), short),
                change
                    .to
                    .as_deref()
                    .map_or(String::from("not pinned"), short)
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
        let (title, detail) = match friendly(&input.name) {
            Some(friendly) => (friendly.to_string(), format!("{} · {detail}", input.name)),
            None => (input.name.clone(), detail),
        };
        FormRow::new(format!("settings.update.{}", input.name), title)
            .subtitle(detail)
            .trailing(
                PushButton::new(format!("settings.update.one.{}", input.name), "Update")
                    .disabled(running)
                    .on_press(move || BuildJob::start_update(vec![name.clone()]))
                    .build(),
            )
    });
    let mut inputs = group()
        .header("Components")
        .rows(inputs)
        .footer("Update on one component brings just that one up to date. Nothing changes until you apply it.");
    if !inputs.has_rows() {
        inputs = inputs.row(match &lock.error {
            Some(error) => crate::problem_row(
                "settings.update.no-inputs",
                "The components couldn't be read",
                "The configuration's list of versions (flake.lock) didn't read.",
                error,
            ),
            None => FormRow::new("settings.update.no-inputs", "No components"),
        });
    }

    let mut top = group().row(
        FormRow::new("settings.update.check-row", "Check for updates")
            .subtitle("Builds the updated system first; nothing is written until you apply it")
            .trailing(
                PushButton::new("settings.update.check", "Check Now")
                    .style(Style::Filled)
                    .disabled(running)
                    .on_press(|| BuildJob::start_update(Vec::new()))
                    .build(),
            ),
    );
    if let Some(result) = result_card(&job, &check) {
        top = top.row(result);
    }

    groups(vec![
        top,
        inputs,
        group().rows(controls::rows("update")).footer(
            "The automatic check looks for newer versions and lets you know; it never builds or applies anything.",
        ),
    ])
}

/// A plain name for the components most configurations have.
fn friendly(input: &str) -> Option<&'static str> {
    Some(match input {
        "nixpkgs" => "NixOS packages",
        "home-manager" => "Home Manager",
        "niri" | "niri-fork" => "Compositor",
        "amane-shell" => "Shell",
        "amane" => "Shell toolkit",
        "lanzaboote" => "Secure Boot",
        "disko" => "Disk layout",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
