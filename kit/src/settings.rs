//! The shell's settings that a settings app changes and the shell follows
//! live: `~/.config/amane-shell/appearance.json`, `desktop.json`,
//! `launcher.json`, `status-bar.json`, `dock.json` and `nixos.json`,
//! through amane's `Config`. [`theme`](crate::theme) reads the appearance,
//! between the `AMANE_*` overrides and what the system says.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::RwLockReadGuard;

use amane::serde::{Deserialize, Serialize};
use amane::{Color, Config, Service, Settings};

use crate::theme::{Density, Glass, Mode};

/// The accents to pick from, macOS 27's system colours in their dark
/// form, and none for the system's.
pub const ACCENTS: [(&str, Option<&str>); 9] = [
    ("System", None),
    ("Blue", Some("#0091ff")),
    ("Purple", Some("#db34f2")),
    ("Pink", Some("#ff375f")),
    ("Red", Some("#ff4245")),
    ("Orange", Some("#ff9230")),
    ("Yellow", Some("#ffd600")),
    ("Green", Some("#30d158")),
    ("Graphite", Some("#98989f")),
];

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

/// Which edge of the screen the status bar runs along.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum BarPosition {
    #[default]
    Bottom,
    Top,
}

/// What the status bar is made of.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum BarStyle {
    /// The standard material, frosted by niri's layer rule.
    #[default]
    Frosted,
    /// niri's glass material, clearer than the frost.
    Glass,
    /// Opaque: black in dark mode, white in light.
    Solid,
}

/// How the clock's tooltip gives the day.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum DateFormat {
    /// "Sunday 25 October".
    #[default]
    Long,
    /// "Sun 25 Oct".
    Short,
    /// The locale's numbers, such as "25/10/26".
    Numeric,
}

impl DateFormat {
    pub const ALL: [DateFormat; 3] = [Self::Long, Self::Short, Self::Numeric];

    /// The `strftime` pattern for it.
    pub fn pattern(self) -> &'static str {
        match self {
            Self::Long => "%A %-d %B",
            Self::Short => "%a %-d %b",
            Self::Numeric => "%x",
        }
    }
}

/// The status bar: where it sits, what it's made of, its clock, its
/// workspaces and which of its icons show.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct StatusBar {
    pub position: BarPosition,
    pub style: BarStyle,

    /// 24-hour time, else 12-hour with AM and PM.
    pub clock_24h: bool,
    pub clock_seconds: bool,
    pub date_format: DateFormat,

    /// Whether the workspace pips show on the left.
    pub show_workspaces: bool,

    /// Whether scrolling over the workspaces moves between them.
    pub scroll_workspaces: bool,

    /// The icons left out, by their names, like "volume".
    pub hidden: Vec<String>,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            position: BarPosition::Bottom,
            style: BarStyle::Frosted,
            clock_24h: true,
            clock_seconds: false,
            date_format: DateFormat::Long,
            show_workspaces: true,
            scroll_workspaces: true,
            hidden: Vec::new(),
        }
    }
}

impl StatusBar {
    /// The `strftime` pattern for the clock.
    pub fn time_pattern(&self) -> &'static str {
        match (self.clock_24h, self.clock_seconds) {
            (true, false) => "%H:%M",
            (true, true) => "%H:%M:%S",
            (false, false) => "%-I:%M %p",
            (false, true) => "%-I:%M:%S %p",
        }
    }

    /// Whether the icon by this name shows.
    pub fn shows(&self, name: &str) -> bool {
        !self.hidden.iter().any(|hidden| hidden == name)
    }

    /// Shows or hides the icon by this name.
    pub fn set_shown(&mut self, name: &str, shown: bool) {
        self.hidden.retain(|hidden| hidden != name);

        if !shown {
            self.hidden.push(String::from(name));
        }
    }
}

impl Settings for StatusBar {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "status-bar";
}

/// The status bar settings as they are now; a view that reads them redraws
/// when they change.
pub fn status_bar() -> RwLockReadGuard<'static, Config<StatusBar>> {
    Config::<StatusBar>::read()
}

/// Changes the status bar settings and saves them, for input handlers.
pub fn change_status_bar(edit: impl FnOnce(&mut StatusBar)) {
    Config::<StatusBar>::change(edit);
}

/// How much the dock's icons grow under the pointer.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Magnify {
    Off,
    Small,
    #[default]
    Medium,
    Large,
}

impl Magnify {
    pub const ALL: [Magnify; 4] = [Self::Off, Self::Small, Self::Medium, Self::Large];

    /// How much more than Medium it grows: 0 for none.
    pub fn amount(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Small => 0.5,
            Self::Medium => 1.0,
            Self::Large => 1.6,
        }
    }
}

/// The smallest and largest the dock's icons are set to.
pub const DOCK_ICON_SIZES: (u8, u8) = (32, 64);

/// The dock: its icons, whether it hides, and what it shows and animates.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Dock {
    /// The icons' resting size, within `DOCK_ICON_SIZES`.
    pub icon_size: u8,
    pub magnification: Magnify,

    /// Whether it slides away until the pointer meets the screen's edge.
    pub auto_hide: bool,

    /// Whether it rises with niri's overview.
    pub rise_with_overview: bool,

    /// Whether a dot marks the apps with a window open.
    pub running_dots: bool,

    /// Whether an app's name shows over its icon on hover.
    pub tooltips: bool,

    pub animations: bool,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            icon_size: 44,
            magnification: Magnify::Medium,
            auto_hide: true,
            rise_with_overview: true,
            running_dots: true,
            tooltips: true,
            animations: true,
        }
    }
}

impl Dock {
    /// The icons' resting size, kept within `DOCK_ICON_SIZES`.
    pub fn icon(&self) -> f32 {
        let (least, most) = DOCK_ICON_SIZES;

        f32::from(self.icon_size.clamp(least, most))
    }
}

impl Settings for Dock {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "dock";
}

/// The dock settings as they are now; a view that reads them redraws when
/// they change.
pub fn dock() -> RwLockReadGuard<'static, Config<Dock>> {
    Config::<Dock>::read()
}

/// Changes the dock settings and saves them, for input handlers.
pub fn change_dock(edit: impl FnOnce(&mut Dock)) {
    Config::<Dock>::change(edit);
}

/// Where the NixOS configuration Shell Settings manages lives: its flake
/// repo and the `nixosConfigurations` attribute to build.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Nixos {
    /// The flake repo; none for `~/nixos-config`.
    pub repo: Option<PathBuf>,

    /// The `nixosConfigurations` attribute; empty for `nixos-niri`.
    pub host: String,
}

impl Nixos {
    /// The repo, `~/nixos-config` unless set.
    pub fn repo_path(&self) -> PathBuf {
        self.repo.clone().unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("nixos-config")
        })
    }

    /// The attribute to build, `nixos-niri` unless set (it's the flake's
    /// name for the machine, not its hostname).
    pub fn host_attr(&self) -> String {
        if self.host.trim().is_empty() {
            String::from("nixos-niri")
        } else {
            self.host.trim().to_string()
        }
    }
}

impl Settings for Nixos {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "nixos";
}

/// The NixOS settings as they are now; a view that reads them redraws when
/// they change.
pub fn nixos() -> RwLockReadGuard<'static, Config<Nixos>> {
    Config::<Nixos>::read()
}

/// Changes the NixOS settings and saves them, for input handlers.
pub fn change_nixos(edit: impl FnOnce(&mut Nixos)) {
    Config::<Nixos>::change(edit);
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

    #[test]
    fn the_status_bar_reads_an_older_file_and_hides_by_name() {
        let mut bar: StatusBar =
            serde_json::from_str(r#"{ "position": "top", "hidden": ["volume"] }"#)
                .unwrap_or_default();

        assert_eq!(bar.position, BarPosition::Top);
        assert_eq!(bar.style, BarStyle::Frosted);
        assert!(bar.show_workspaces);
        assert!(!bar.shows("volume"));

        bar.set_shown("volume", true);
        bar.set_shown("battery", false);
        bar.set_shown("battery", false);

        assert!(bar.shows("volume"));
        assert_eq!(bar.hidden, vec![String::from("battery")]);
    }

    #[test]
    fn the_clock_follows_its_format() {
        let mut bar = StatusBar::default();
        assert_eq!(bar.time_pattern(), "%H:%M");

        bar.clock_24h = false;
        bar.clock_seconds = true;
        assert_eq!(bar.time_pattern(), "%-I:%M:%S %p");
    }

    #[test]
    fn the_dock_keeps_its_icons_within_bounds() {
        let dock: Dock = serde_json::from_str(r#"{ "icon_size": 200, "magnification": "off" }"#)
            .unwrap_or_default();

        assert_eq!(dock.icon(), 64.0);
        assert_eq!(dock.magnification, Magnify::Off);
        assert!(dock.auto_hide);
        assert_eq!(Dock::default().icon(), 44.0);
    }
}
