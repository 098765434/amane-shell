//! A `flake.lock`'s root inputs, and what a fresh lock moves: what Check
//! for Updates shows and an update's commit says; and what the scheduled
//! check last found ([`Check`]), which Shell Settings and the shell's
//! Software Update tile both read.

use std::io;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::repo::short;

/// One of the root's inputs, as the lock pins it.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    /// Its name in the flake's `inputs`.
    pub name: String,
    /// The commit it's locked at, when the lock records one.
    pub rev: Option<String>,
    /// Where it comes from: "owner/repo", else its URL or type.
    pub source: Option<String>,
    /// The lock's `lastModified` for it.
    pub modified: Option<DateTime<Utc>>,
}

impl Input {
    /// "12 days ago", "3 h ago", "3 months ago".
    pub fn ago(&self) -> String {
        match self.modified {
            Some(when) => amane_base::format::ago((Utc::now() - when).to_std().unwrap_or_default()),
            None => String::from("unknown"),
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
    /// The input's name.
    pub name: String,
    /// Its commit in the old lock; none when it's new or unpinned.
    pub from: Option<String>,
    /// Its commit in the fresh lock.
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

/// What the scheduled check (`amane-update-check`, a user timer from the
/// configuration) last found, from `check.json` in the shell's state folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Check {
    /// When it ran.
    pub checked: Option<DateTime<Utc>>,
    /// The inputs with something newer.
    pub changes: Vec<InputChange>,
    /// Why it couldn't look, if it couldn't.
    pub error: Option<String>,
    /// Whether there's a check at all.
    pub present: bool,
}

/// Where the check is kept: `$XDG_STATE_HOME/amane-shell/nixos/check.json`.
pub fn check_path() -> PathBuf {
    amane_base::paths::state()
        .join(amane_base::app_ids::SHELL)
        .join("nixos")
        .join("check.json")
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

/// The check kept, or none.
pub fn read_check() -> Check {
    std::fs::read_to_string(check_path())
        .map(|json| parse_check(&json))
        .unwrap_or_default()
}

/// Says nothing waits, checked now: once an update is applied.
pub fn clear_check() -> io::Result<()> {
    let now = Utc::now().timestamp();
    let json = format!("{{\n  \"checked\": {now},\n  \"changes\": []\n}}\n");
    amane_base::fs::write_whole(&check_path(), json)
}

#[cfg(test)]
mod check_tests {
    use super::*;

    #[test]
    fn a_check_lists_what_moved() {
        let found = parse_check(
            r#"{"checked": 1761500000, "changes": [{"name": "nixpkgs", "from": "a", "to": "b"}, {"name": "amane-shell"}]}"#,
        );
        let names: Vec<&str> = found.changes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["nixpkgs", "amane-shell"]);
        assert_eq!(found.changes[0].to.as_deref(), Some("b"));
        let moved = parse_check(
            r#"{"checked": 1, "changes": [{"name": "nixpkgs", "from": "1111111aaaaaaa", "to": "2222222bbbbbbb"}]}"#,
        );
        assert_eq!(moved.changes[0].text(), "nixpkgs 1111111 → 2222222");
        assert_eq!(found.checked.map(|at| at.timestamp()), Some(1761500000));
        assert!(found.present);
        assert!(!parse_check("not json").present);
        assert_eq!(
            parse_check(r#"{"checked": 1, "changes": [], "error": "no network"}"#)
                .error
                .as_deref(),
            Some("no network")
        );
    }
}
