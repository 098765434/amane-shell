//! A `flake.lock`'s root inputs, and what a fresh lock moves: what Check
//! for Updates shows and an update's commit says.

use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use crate::repo::short;

/// One of the root's inputs, as the lock pins it.
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
