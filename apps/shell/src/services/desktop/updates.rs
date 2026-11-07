//! What the last NixOS update check found: `amane-update-check` (a user
//! timer from the NixOS configuration) relocks the flake beside the repo
//! and writes `nixos/check.json` in the shell's state dir; Shell Settings
//! clears it once an update is applied. The Software Update tile reads it.

use amane::{Service, watch_file};
use serde_json::Value;

use crate::services::state_file;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Updates {
    /// The inputs with something newer, by name.
    pub changed: Vec<String>,
    /// When the check ran, as seconds since the epoch.
    pub checked: Option<u64>,
    pub error: Option<String>,
}

/// `check.json` as the checker writes it.
pub fn parse(json: &str) -> Updates {
    let value: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    Updates {
        changed: value
            .get("changes")
            .and_then(Value::as_array)
            .map(|changes| {
                changes
                    .iter()
                    .filter_map(|change| change.get("name")?.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        checked: value.get("checked").and_then(Value::as_u64),
        error: value.get("error").and_then(Value::as_str).map(String::from),
    }
}

fn look() -> Updates {
    std::fs::read_to_string(state_file("nixos/check.json"))
        .map(|json| parse(&json))
        .unwrap_or_default()
}

impl Service for Updates {
    fn new() -> Self {
        look()
    }

    fn listen() {
        let path = state_file("nixos/check.json");
        loop {
            if !path.parent().is_some_and(|folder| folder.is_dir()) {
                std::thread::sleep(std::time::Duration::from_secs(60));
                continue;
            }
            for _ in watch_file(&path.to_string_lossy()) {
                let fresh = look();
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

impl Updates {
    /// "nixpkgs, amane-shell": what has updates, for a subtitle.
    pub fn names(&self) -> String {
        self.changed.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_lists_what_moved() {
        let found = parse(
            r#"{"checked": 1761500000, "changes": [{"name": "nixpkgs", "from": "a", "to": "b"}, {"name": "amane-shell"}]}"#,
        );
        assert_eq!(found.names(), "nixpkgs, amane-shell");
        assert_eq!(found.checked, Some(1761500000));
        assert!(parse("{}").changed.is_empty());
        assert_eq!(
            parse(r#"{"checked": 1, "changes": [], "error": "no network"}"#)
                .error
                .as_deref(),
            Some("no network")
        );
        assert!(parse("not json").changed.is_empty());
    }
}
