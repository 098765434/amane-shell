//! Software Update: the flake's inputs from `flake.lock`, each with when it
//! was last updated. Checking for and taking updates is for later.

use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use amane::{Column, Service, Size, watch_file};
use amane_kit::layout::boxed;
use amane_kit::list::{List, ListItem, trailing_text, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::tokens::sys;
use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use super::nixos::repo;
use super::repo::short;
use crate::{PAGE_WIDTH, note};

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

pub struct Lock {
    pub inputs: Vec<Input>,
    pub error: Option<String>,
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

pub fn page() -> Column {
    let lock = Lock::read();

    let list = with_row_width(PAGE_WIDTH, || {
        let items: Vec<ListItem> = lock
            .inputs
            .iter()
            .map(|input| {
                let detail = format!(
                    "{}{}",
                    input.ago(),
                    input
                        .rev
                        .as_deref()
                        .map_or(String::new(), |rev| format!(" · {}", short(rev)))
                );
                ListItem::new(format!("settings.update.{}", input.name), &input.name)
                    .bare(true)
                    .trailing(trailing_text(detail, material::label(Level::Secondary)))
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

    Column::new(vec![
        boxed(note(
            "What the configuration is pinned to. Update with `nix flake update` (or one input: `nix flake update niri-fork`), then Apply.",
        )),
        boxed(list),
    ])
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
}
