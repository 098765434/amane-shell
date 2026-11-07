//! The keyboard layouts niri has and the one in use, for the bar's input
//! menu. Followed from `niri msg --json event-stream`, which
//! says the layouts as it starts and each switch after.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use amane::Service;
use serde_json::Value;

#[derive(Default, PartialEq)]
pub struct KeyboardLayout {
    names: Vec<String>,
    current: usize,
}

impl Service for KeyboardLayout {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        let child = Command::new("niri")
            .args(["msg", "--json", "event-stream"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            return;
        };
        let Some(stdout) = child.stdout.take() else {
            return;
        };
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Some(next) = apply(&KeyboardLayout::read(), &line) else {
                continue;
            };
            *KeyboardLayout::write() = next;
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl KeyboardLayout {
    /// The layouts' names, like "English (US)" and "Norwegian".
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn current(&self) -> usize {
        self.current
    }

    /// The layout in use, by name.
    pub fn name(&self) -> Option<&str> {
        self.names.get(self.current).map(String::as_str)
    }

    /// Switches to the layout at `index`.
    pub fn switch(index: usize) {
        amane::spawn(&format!("niri msg action switch-layout {index}"));
    }
}

/// What a line of the event stream makes of `now`, when it says anything
/// about the layouts.
fn apply(now: &KeyboardLayout, line: &str) -> Option<KeyboardLayout> {
    let event: Value = serde_json::from_str(line).ok()?;
    if let Some(layouts) = event.get("KeyboardLayoutsChanged") {
        let layouts = &layouts["keyboard_layouts"];
        let names = layouts["names"]
            .as_array()?
            .iter()
            .filter_map(|name| name.as_str().map(String::from))
            .collect();
        let current = layouts["current_idx"].as_u64().unwrap_or(0) as usize;
        let next = KeyboardLayout { names, current };
        return (next != *now).then_some(next);
    }
    if let Some(switched) = event.get("KeyboardLayoutSwitched") {
        let current = switched["idx"].as_u64()? as usize;
        return (current != now.current).then(|| KeyboardLayout {
            names: now.names.clone(),
            current,
        });
    }
    None
}

/// A short code for a layout's name, as the bar shows the input source: the
/// language's own two letters where the name says, else its first two.
pub fn short(name: &str) -> String {
    const KNOWN: [(&str, &str); 14] = [
        ("English (US", "US"),
        ("English (UK", "GB"),
        ("Norwegian", "NO"),
        ("Swedish", "SE"),
        ("Danish", "DK"),
        ("German", "DE"),
        ("French", "FR"),
        ("Spanish", "ES"),
        ("Italian", "IT"),
        ("Finnish", "FI"),
        ("Dutch", "NL"),
        ("Polish", "PL"),
        ("Russian", "RU"),
        ("Japanese", "JP"),
    ];
    if let Some((_, code)) = KNOWN.iter().find(|(known, _)| name.starts_with(known)) {
        return String::from(*code);
    }
    name.chars()
        .filter(char::is_ascii_alphabetic)
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_layouts_and_their_switches() {
        let start = KeyboardLayout::default();
        let line = r#"{"KeyboardLayoutsChanged":{"keyboard_layouts":{"names":["English (US)","Norwegian"],"current_idx":1}}}"#;
        let now = apply(&start, line).unwrap();
        assert_eq!(now.name(), Some("Norwegian"));

        let switched = apply(&now, r#"{"KeyboardLayoutSwitched":{"idx":0}}"#).unwrap();
        assert_eq!(switched.name(), Some("English (US)"));

        assert!(apply(&switched, r#"{"KeyboardLayoutSwitched":{"idx":0}}"#).is_none());
        assert!(apply(&switched, r#"{"WorkspaceActivated":{"id":1}}"#).is_none());
    }

    #[test]
    fn a_recorded_stream_replays_to_the_last_switch() {
        let stream = include_str!("../../../tests/fixtures/niri/event-stream.jsonl");
        let mut now = KeyboardLayout::default();
        let mut seen = Vec::new();
        for line in stream.lines() {
            if let Some(next) = apply(&now, line) {
                now = next;
                seen.push(now.current);
            }
        }
        assert_eq!(now.names, ["Norwegian", "English (US)"]);
        // changed (0), switched to 1, then back to 0; the rest says nothing
        assert_eq!(seen, [0, 1, 0]);
    }

    #[test]
    fn a_layout_has_a_short_code() {
        assert_eq!(short("Norwegian"), "NO");
        assert_eq!(short("English (US, intl., with dead keys)"), "US");
        assert_eq!(short("Klingon"), "KL");
    }
}
