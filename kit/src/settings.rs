//! The shell's settings that a settings app changes and the shell follows
//! live: `~/.config/amane-shell/appearance.json` and `desktop.json`,
//! through amane's `Config`. [`theme`](crate::theme) reads the appearance,
//! between the `AMANE_*` overrides and what the system says.

use std::collections::BTreeMap;
use std::sync::RwLockReadGuard;

use amane::serde::{Deserialize, Serialize};
use amane::{Color, Config, Service, Settings};

use crate::theme::{Density, Glass, Mode};

/// How the shell looks: mode, accent, density, text size, how see-through
/// its glass is, and whether it moves.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Appearance {
    /// Light or dark, or none to follow the system.
    pub mode: Option<Mode>,

    /// The accent as `#rrggbb`, or none for the system's (else the tokens').
    pub accent: Option<String>,

    pub density: Density,

    /// Multiplies every font size, kept within 0.5–3.
    pub text_scale: f32,

    /// How see-through the glass is.
    pub glass: Glass,

    /// Whether to reduce motion, or none to follow the system.
    pub reduce_motion: Option<bool>,

    /// Whether small glass turns light or dark with what's behind it, from
    /// niri's readings of the backdrop (an experiment, off by default).
    pub adaptive_glass: bool,

    /// Whether labels on glass blend with the frosted backdrop under them,
    /// through niri's vibrancy (an experiment, off by default).
    pub vibrancy: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: None,
            accent: None,
            density: Density::Comfortable,
            text_scale: 1.0,
            glass: Glass::Tinted,
            reduce_motion: None,
            adaptive_glass: false,
            vibrancy: false,
        }
    }
}

impl Settings for Appearance {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "appearance";
}

impl Appearance {
    /// The accent colour, if a valid one is set.
    pub fn accent_color(&self) -> Option<Color> {
        let hex = self.accent.as_deref()?.strip_prefix('#')?;

        if hex.len() != 6 {
            return None;
        }

        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();

        Some(Color::rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// The appearance settings as they are now; a view that reads them redraws
/// when they change, from this program or the settings app. Every token
/// read looks here, so it hands out the lock rather than a copy.
pub fn appearance() -> RwLockReadGuard<'static, Config<Appearance>> {
    Config::<Appearance>::read()
}

/// Changes the appearance settings and saves them, for input handlers.
pub fn change_appearance(edit: impl FnOnce(&mut Appearance)) {
    Config::<Appearance>::change(edit);
}

/// How the desktop's widgets look while windows are open over the desktop.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum WidgetStyle {
    /// Faded back while a window is open on the monitor's workspace, as
    /// macOS dims them.
    #[default]
    Automatic,
    /// Always as they are with nothing open.
    Full,
    /// Always faded back.
    Dim,
}

/// The desktop: its widgets.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Desktop {
    pub show_widgets: bool,
    pub widget_style: WidgetStyle,
}

impl Default for Desktop {
    fn default() -> Self {
        Self {
            show_widgets: true,
            widget_style: WidgetStyle::Automatic,
        }
    }
}

impl Settings for Desktop {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "desktop";
}

/// The desktop settings as they are now; a view that reads them redraws
/// when they change.
pub fn desktop() -> RwLockReadGuard<'static, Config<Desktop>> {
    Config::<Desktop>::read()
}

/// Changes the desktop settings and saves them, for input handlers.
pub fn change_desktop(edit: impl FnOnce(&mut Desktop)) {
    Config::<Desktop>::change(edit);
}

/// How the launcher shows the apps in its Apps mode.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum AppsLayout {
    #[default]
    Grid,
    List,
}

/// An action's quick keys: what they run, by the launcher's key for it,
/// and its name then, for a settings page that can't ask the launcher.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", default)]
pub struct QuickKey {
    pub action: String,
    pub title: String,
}

/// The launcher's clipboard history: off until asked for, and what it
/// keeps when on.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct ClipboardHistory {
    pub enabled: bool,

    /// How many copies it keeps, newest first.
    pub max_items: usize,

    /// Whether copied images are kept too.
    pub images: bool,

    /// Apps whose copies aren't kept, by their app ids, like password
    /// managers.
    pub exclude_apps: Vec<String>,
}

impl Default for ClipboardHistory {
    fn default() -> Self {
        Self {
            enabled: false,
            max_items: 200,
            images: true,
            exclude_apps: [
                "org.keepassxc.KeePassXC",
                "keepassxc",
                "bitwarden",
                "Bitwarden",
                "1password",
                "1Password",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

/// The launcher: how it lays the apps out, what its search leaves out,
/// the quick keys that run its actions, and its clipboard history.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Launcher {
    pub apps_layout: AppsLayout,

    /// The sources the search leaves out, by their ids, like "web".
    pub excluded_providers: Vec<String>,

    /// Keys typed in the launcher, like "nl", and the action each runs.
    pub quick_keys: BTreeMap<String, QuickKey>,

    pub clipboard: ClipboardHistory,
}

impl Launcher {
    /// The quick keys that run `action`, if any do.
    pub fn keys_for(&self, action: &str) -> Option<&str> {
        self.quick_keys
            .iter()
            .find(|(_, key)| key.action == action)
            .map(|(keys, _)| keys.as_str())
    }
}

impl Settings for Launcher {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "launcher";
}

/// The launcher settings as they are now; a view that reads them redraws
/// when they change.
pub fn launcher() -> RwLockReadGuard<'static, Config<Launcher>> {
    Config::<Launcher>::read()
}

/// Changes the launcher settings and saves them, for input handlers.
pub fn change_launcher(edit: impl FnOnce(&mut Launcher)) {
    Config::<Launcher>::change(edit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_accent_and_turns_down_a_broken_one() {
        let with = |accent: &str| Appearance {
            accent: Some(String::from(accent)),
            ..Appearance::default()
        };

        assert_eq!(
            with("#0a84ff").accent_color(),
            Some(Color::rgb(10, 132, 255))
        );
        assert_eq!(with("0a84ff").accent_color(), None);
        assert_eq!(with("#0a84f").accent_color(), None);
        assert_eq!(Appearance::default().accent_color(), None);
    }

    #[test]
    fn an_older_file_keeps_the_defaults_for_what_it_lacks() {
        let appearance: Appearance =
            serde_json::from_str(r#"{ "mode": "light" }"#).unwrap_or_default();

        assert_eq!(appearance.mode, Some(Mode::Light));
        assert_eq!(appearance.text_scale, 1.0);
        assert_eq!(appearance.glass, Glass::Tinted);
        assert_eq!(appearance.reduce_motion, None);
    }

    #[test]
    fn the_desktop_shows_its_widgets_until_told_otherwise() {
        let desktop: Desktop =
            serde_json::from_str(r#"{ "widget_style": "dim" }"#).unwrap_or_default();

        assert!(desktop.show_widgets);
        assert_eq!(desktop.widget_style, WidgetStyle::Dim);
    }
}
