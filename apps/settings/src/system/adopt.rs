//! Move to Settings: a setting another file sets by hand (locked here)
//! moves into `managed/settings.json`. nix told the snapshot which line;
//! this reads it, shows it, and with the user's OK turns it into a comment
//! and writes the same value to the settings file, committing both as one
//! commit of their own (`move: Keyboard layout from configuration.nix`), so
//! it can be reverted on its own. The only place Settings touches a `.nix`
//! file, and only a single `name = value;` line.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::thread;

use amane::Service;
use amane_kit::app::Dialog;
use serde_json::Value;

use super::controls::Feedback;
use super::nixos::{Entry, Managed, SETTINGS_FILE, Snapshot, repo, run, save};
use super::repo::Repo;

/// A line ready to move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub file: String,
    pub line: u64,
    /// The line as it is.
    pub text: String,
    /// The comment that takes its place.
    pub replacement: String,
}

/// The value part of `name = value;` on one line, when the line is that
/// and nothing cleverer.
pub fn parse_line(line: &str, attr: &str) -> Result<String, String> {
    let trimmed = line.trim();
    let Some((name, rest)) = trimmed.split_once('=') else {
        return Err(String::from("isn't a `name = value;` line"));
    };
    // `hostName = …` inside a block, or `networking.hostName = …`
    let name = name.trim().trim_matches('"');
    if name.rsplit('.').next() != Some(attr) {
        return Err(format!("sets {name}, not {attr}"));
    }
    let rest = rest.trim();
    // a comment after the value is fine; a value that spans lines isn't
    let value = match rest.find(';') {
        Some(end)
            if rest[end + 1..].trim().is_empty() || rest[end + 1..].trim().starts_with('#') =>
        {
            rest[..end].trim()
        }
        _ => return Err(String::from("doesn't end on this line")),
    };
    if value.is_empty() || value.contains("mk") || value.contains("lib.") || value.starts_with('{')
    {
        return Err(String::from("is more than a plain value"));
    }
    Ok(value.to_string())
}

/// What moving `entry` would do, or why it can't be done here.
pub fn plan(entry: &Entry) -> Result<Move, String> {
    let [(file, line)] = entry.set_at.as_slice() else {
        return Err(match entry.set_elsewhere().as_slice() {
            [] => String::from("nothing to move"),
            [file] => format!("Set in {file}; edit it by hand to move it"),
            files => format!("Set in {}; edit them by hand", files.join(", ")),
        });
    };
    let path = repo().join(file);
    let content = fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
    let text = content
        .lines()
        .nth(*line as usize - 1)
        .ok_or_else(|| format!("{file} has no line {line}"))?
        .to_string();
    let attr = entry.path.last().map_or("", String::as_str);
    parse_line(&text, attr).map_err(|why| format!("{file}:{line} {why}; edit it by hand"))?;
    let indent: String = text.chars().take_while(|c| c.is_whitespace()).collect();
    Ok(Move {
        file: file.clone(),
        line: *line,
        replacement: format!(
            "{indent}# {attr} is `{}` in managed/settings.json (Shell Settings).",
            entry.id
        ),
        text,
    })
}

/// Asks, then moves.
pub fn confirm(entry: &Entry) {
    let planned = match plan(entry) {
        Ok(planned) => planned,
        Err(why) => {
            Feedback::write().error = Some((entry.id.clone(), why));
            return;
        }
    };
    let entry = entry.clone();
    let text = planned.text.trim().to_string();
    Dialog::new(
        "settings.system.move",
        format!("Move {} to Settings?", entry.label),
    )
    .message(format!(
        "{}:{} becomes a comment and the value goes to managed/settings.json, \
             as a commit of its own.\n\n{text}",
        planned.file, planned.line
    ))
    .button("Cancel", || {})
    .main_button("Move", move || {
        let (entry, planned) = (entry.clone(), planned.clone());
        thread::spawn(move || {
            if let Err(why) = apply(&entry, &planned) {
                Feedback::write().error = Some((entry.id.clone(), why));
            }
            Managed::reload();
            Repo::refresh();
            Snapshot::refresh();
        });
    })
    .open("main");
}

/// Rewrites the line, writes the value, commits both.
fn apply(entry: &Entry, planned: &Move) -> Result<(), String> {
    let dirty = Repo::read().dirty.clone();
    if dirty.contains(&planned.file) {
        return Err(format!(
            "{} has uncommitted changes; commit or discard them first",
            planned.file
        ));
    }
    let current = Managed::read().current.clone();
    apply_in(&repo(), entry, planned, current)
}

/// [`apply`] in the repo at `root`, with the settings as they are.
fn apply_in(
    root: &Path,
    entry: &Entry,
    planned: &Move,
    mut map: BTreeMap<String, Value>,
) -> Result<(), String> {
    let path = root.join(&planned.file);
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut lines: Vec<&str> = content.lines().collect();
    let index = planned.line as usize - 1;
    if lines.get(index) != Some(&planned.text.as_str()) {
        return Err(format!(
            "{}:{} changed meanwhile",
            planned.file, planned.line
        ));
    }
    lines[index] = &planned.replacement;
    let mut rewritten = lines.join("\n");
    if content.ends_with('\n') {
        rewritten.push('\n');
    }
    fs::write(&path, rewritten).map_err(|e| e.to_string())?;

    // the value as the configuration has it, so nothing changes
    map.insert(entry.id.clone(), entry.value.clone());
    save(&root.join(SETTINGS_FILE), &map)?;

    let git = |args: &[&str]| run("git", args, Some(root));
    git(&["add", "--", &planned.file, SETTINGS_FILE])?;
    git(&[
        "commit",
        "-q",
        "-m",
        &format!("move: {} from {}", entry.label, planned.file),
        "--",
        &planned.file,
        SETTINGS_FILE,
    ])
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::nixos::{Control, Tier};

    /// A throwaway repo with one configuration file and no settings yet.
    fn temp_repo() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "amane-adopt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("managed")).unwrap();
        fs::write(
            root.join("configuration.nix"),
            "{\n  networking.hostName = \"nixos\";\n  services.xserver.xkb = {\n    layout = \"no\";\n  };\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("managed/settings.json"),
            "{\n  \"timeZone\": \"Europe/Oslo\"\n}\n",
        )
        .unwrap();
        let git = |args: &[&str]| run("git", args, Some(&root)).unwrap();
        git(&["init", "-q"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "add", "."]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "start",
        ]);
        root
    }

    #[test]
    fn a_line_moves_into_the_settings_file_as_one_commit() {
        let root = temp_repo();
        let entry = Entry {
            id: String::from("keyboardLayout"),
            path: vec![
                "services".into(),
                "xserver".into(),
                "xkb".into(),
                "layout".into(),
            ],
            page: String::from("general"),
            label: String::from("Keyboard layout"),
            control: Control::Text,
            tier: Tier::Rebuild,
            value: Value::from("no"),
            default: Value::Null,
            description: String::new(),
            set_in: vec![String::from("configuration.nix")],
            set_at: vec![(String::from("configuration.nix"), 4)],
            check_kind: None,
            unit: None,
            user_scope: false,
            on_demand: None,
        };
        let planned = Move {
            file: String::from("configuration.nix"),
            line: 4,
            text: String::from("    layout = \"no\";"),
            replacement: String::from(
                "    # layout is `keyboardLayout` in managed/settings.json (Shell Settings).",
            ),
        };
        let mut map = BTreeMap::new();
        map.insert(String::from("timeZone"), Value::from("Europe/Oslo"));

        // git needs an identity to commit
        let git = |args: &[&str]| run("git", args, Some(&root));
        git(&["config", "user.name", "t"]).unwrap();
        git(&["config", "user.email", "t@t"]).unwrap();

        apply_in(&root, &entry, &planned, map).unwrap();

        let nix = fs::read_to_string(root.join("configuration.nix")).unwrap();
        assert!(nix.contains("    # layout is `keyboardLayout` in managed/settings.json"));
        assert!(!nix.contains("layout = \"no\""));
        let json = fs::read_to_string(root.join("managed/settings.json")).unwrap();
        assert!(json.contains("\"keyboardLayout\": \"no\""));
        assert!(json.contains("Europe/Oslo"));
        let log = git(&["log", "--format=%s", "-1"]).unwrap();
        assert_eq!(log.trim(), "move: Keyboard layout from configuration.nix");
        assert_eq!(git(&["status", "--porcelain"]).unwrap().trim(), "");
        assert_eq!(git(&["rev-list", "--count", "HEAD"]).unwrap().trim(), "2");

        // the line moved on: nothing is touched
        let stale = Move {
            text: String::from("    layout = \"us\";"),
            ..planned
        };
        assert!(apply_in(&root, &entry, &stale, BTreeMap::new()).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn only_a_plain_line_moves() {
        assert_eq!(
            parse_line("  layout = \"no\";", "layout").unwrap(),
            "\"no\""
        );
        assert_eq!(
            parse_line("  networking.hostName = \"nixos\"; # the name", "hostName").unwrap(),
            "\"nixos\""
        );
        assert!(parse_line("  variant = \"\";", "layout").is_err());
        assert!(parse_line("  layout = lib.mkForce \"no\";", "layout").is_err());
        assert!(parse_line("  layout = {", "layout").is_err());
        assert!(parse_line("  listener = [", "listener").is_err());
        assert!(parse_line("  # layout = \"no\";", "layout").is_err());
    }
}
