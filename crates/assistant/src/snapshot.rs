//! What the machine has right now for a command to name: its switches and
//! whether they're on, its apps, its power modes, what the session allows.
//! The shell fills one on its own thread from its services; it crosses to
//! the worker as plain data. A command is only read as naming what's here.

use serde::{Deserialize, Serialize};

use crate::command::Power;
use crate::text;

/// What the machine has now for a command to name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Quick Settings' switches.
    pub switches: Vec<Switch>,
    /// The apps that can be opened.
    pub apps: Vec<App>,
    /// The power profiles, as (what they're called, the daemon's name).
    pub power_modes: Vec<(String, String)>,
    /// What the session allows.
    pub powers: Vec<Power>,
    /// Whether there's a screen brightness to set.
    pub brightness: bool,
    /// Whether a timer is running, so there's one to cancel.
    pub timer_running: bool,
}

/// One of Quick Settings' switches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Switch {
    /// Its control's name: "night-light".
    pub control: String,
    /// What it's called: "Night Light".
    pub title: String,
    /// Whether it's on, for one that's a switch rather than a button.
    pub on: Option<bool>,
}

/// An app that can be opened.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct App {
    /// Its desktop id: "firefox".
    pub id: String,
    /// Its name, as the launcher shows it.
    pub name: String,
    /// What kind of app it is, as people say it: "terminal", "browser",
    /// from its desktop categories and keywords ([`kinds`]).
    #[serde(default)]
    pub kinds: Vec<String>,
}

/// What people call apps with these desktop `categories` and `keywords`:
/// a TerminalEmulator is a "terminal".
pub fn kinds(categories: &[String], keywords: &[String]) -> Vec<String> {
    let mut kinds: Vec<String> = Vec::new();
    for category in categories {
        let words: &[&str] = match category.as_str() {
            "TerminalEmulator" => &["terminal", "console", "shell"],
            "WebBrowser" => &["browser", "web browser", "internet"],
            "FileManager" => &["file manager", "files", "file browser"],
            "Email" => &["email", "mail", "inbox"],
            "Calculator" => &["calculator"],
            "TextEditor" => &["text editor", "editor"],
            "Calendar" => &["calendar"],
            "InstantMessaging" | "Chat" => &["chat", "messenger"],
            "Music" => &["music", "music player"],
            "Video" => &["video player"],
            "IDE" => &["ide", "code editor"],
            "Settings" => &["settings"],
            _ => &[],
        };
        kinds.extend(words.iter().map(|word| word.to_string()));
    }
    kinds.extend(keywords.iter().map(|keyword| text::normalize(keyword)));
    kinds.retain(|kind| !kind.is_empty());
    kinds.sort();
    kinds.dedup();
    kinds
}

impl Switch {
    /// A switch from its control's name, its title and whether it's on.
    pub fn new(control: &str, title: &str, on: Option<bool>) -> Self {
        Self {
            control: control.to_string(),
            title: title.to_string(),
            on,
        }
    }

    /// What it's offered to the model as: its title, plainly ("wifi",
    /// "night light").
    pub fn name(&self) -> String {
        text::normalize(&self.title)
    }

    /// Every name it's known by, its own first.
    pub fn names(&self) -> Vec<String> {
        let mut names = vec![self.name()];
        names.extend(aliases(&self.control).iter().map(|alias| alias.to_string()));
        names
    }
}

/// Other names people use for a control, lowercased.
pub fn aliases(control: &str) -> &'static [&'static str] {
    match control {
        "wifi" => &["wifi", "wi fi", "wireless", "wlan", "internet"],
        "bluetooth" => &["bt"],
        "focus" => &["do not disturb", "dnd", "focus mode", "notifications"],
        "dark-style" => &["dark mode", "dark theme", "dark style", "dark"],
        "night-light" => &[
            "night shift",
            "night mode",
            "blue light filter",
            "warm screen",
            "nightlight",
        ],
        "airplane" => &["airplane", "flight mode", "aeroplane mode", "plane mode"],
        "keep-awake" => &["caffeine", "stay awake", "keep the screen on"],
        "vpn" => &["vpn"],
        "tailscale" => &["tailscale"],
        "wired" => &["ethernet", "wired network", "cable"],
        "keyboard" => &["keyboard light", "keyboard backlight", "backlight"],
        "screenshot" => &["screenshot", "screen shot", "screen capture"],
        _ => &[],
    }
}

/// The one thing `found` has, if it has only one.
fn only<T>(mut found: impl Iterator<Item = T>) -> Option<T> {
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

impl Snapshot {
    /// The switch `name` names, by any of its names.
    pub fn switch(&self, name: &str) -> Option<&Switch> {
        let name = text::normalize(name);
        self.switches
            .iter()
            .find(|switch| switch.names().contains(&name))
    }

    /// The app `name` names: by its name, or its name's start when that's
    /// a whole word and only one app has it ("code" for "Visual Studio
    /// Code" doesn't count, "visual studio" does).
    pub fn app(&self, name: &str) -> Option<&App> {
        let name = text::normalize(name);
        if name.is_empty() {
            return None;
        }
        if let Some(app) = self
            .apps
            .iter()
            .find(|app| text::normalize(&app.name) == name || app.id.to_lowercase() == name)
        {
            return Some(app);
        }
        let starting = |app: &&App| {
            let own = text::normalize(&app.name);
            own.strip_prefix(&name)
                .is_some_and(|rest| rest.starts_with(' '))
        };
        let of_kind = |app: &&App| app.kinds.contains(&name);
        only(self.apps.iter().filter(starting)).or_else(|| only(self.apps.iter().filter(of_kind)))
    }

    /// The power profile `name` names, by what it's called or the
    /// daemon's name.
    pub fn power_mode(&self, name: &str) -> Option<&str> {
        let name = text::normalize(name);
        let name = name.strip_suffix(" mode").unwrap_or(&name);
        self.power_modes
            .iter()
            .find(|(title, id)| text::normalize(title) == name || text::normalize(id) == name)
            .map(|(_, id)| id.as_str())
    }

    /// Words Whistle is told to listen for: the names of what's here.
    pub fn keywords(&self) -> Vec<String> {
        let mut words: Vec<String> = Vec::new();
        for switch in &self.switches {
            words.push(switch.title.clone());
        }
        for app in &self.apps {
            words.push(app.name.clone());
        }
        for word in ["volume", "brightness", "louder", "quieter", "song", "timer"] {
            words.push(word.to_string());
        }
        words.sort();
        words.dedup();
        words
    }
}

#[cfg(test)]
pub(crate) fn sample() -> Snapshot {
    Snapshot {
        switches: vec![
            Switch::new("wifi", "Wi‑Fi", Some(true)),
            Switch::new("bluetooth", "Bluetooth", Some(true)),
            Switch::new("focus", "Focus", Some(false)),
            Switch::new("dark-style", "Dark Style", Some(false)),
            Switch::new("night-light", "Night Light", Some(false)),
            Switch::new("airplane", "Airplane Mode", Some(false)),
            Switch::new("keep-awake", "Keep Awake", Some(false)),
            Switch::new("screenshot", "Screenshot", None),
        ],
        apps: vec![
            App {
                id: "firefox".into(),
                name: "Firefox".into(),
                kinds: Vec::new(),
            },
            App {
                id: "org.gnome.Nautilus".into(),
                name: "Files".into(),
                kinds: Vec::new(),
            },
            App {
                id: "com.mitchellh.ghostty".into(),
                name: "Ghostty".into(),
                kinds: vec!["terminal".into()],
            },
            App {
                id: "code".into(),
                name: "Visual Studio Code".into(),
                kinds: Vec::new(),
            },
            App {
                id: "spotify".into(),
                name: "Spotify".into(),
                kinds: Vec::new(),
            },
        ],
        power_modes: vec![
            ("Power Saver".into(), "power-saver".into()),
            ("Balanced".into(), "balanced".into()),
            ("Performance".into(), "performance".into()),
        ],
        powers: Power::ALL.to_vec(),
        brightness: true,
        timer_running: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_are_found_by_any_name() {
        let snapshot = sample();
        assert_eq!(snapshot.switch("wifi").unwrap().control, "wifi");
        assert_eq!(snapshot.switch("Wi-Fi").unwrap().control, "wifi");
        assert_eq!(snapshot.switch("do not disturb").unwrap().control, "focus");
        assert_eq!(snapshot.switch("dark mode").unwrap().control, "dark-style");
        assert!(snapshot.switch("teleport").is_none());
    }

    #[test]
    fn apps_are_found_by_name_or_a_unique_start() {
        let snapshot = sample();
        assert_eq!(snapshot.app("firefox").unwrap().id, "firefox");
        assert_eq!(snapshot.app("visual studio").unwrap().id, "code");
        assert_eq!(snapshot.app("files").unwrap().id, "org.gnome.Nautilus");
        assert!(snapshot.app("vis").is_none());
        assert_eq!(
            snapshot.app("terminal").unwrap().id,
            "com.mitchellh.ghostty"
        );
        assert!(snapshot.app("").is_none());
    }

    #[test]
    fn power_modes_are_found_as_people_say_them() {
        let snapshot = sample();
        assert_eq!(snapshot.power_mode("power saver"), Some("power-saver"));
        assert_eq!(snapshot.power_mode("performance mode"), Some("performance"));
        assert_eq!(snapshot.power_mode("turbo"), None);
    }
}
