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

/// The displays: what the shell does to them, beside what niri's own
/// `displays.kdl` keeps.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Displays {
    /// On battery, run each display at its slowest refresh rate (at the
    /// same resolution), and go back on AC.
    pub battery_refresh: bool,
}

impl Settings for Displays {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "displays";
}

/// The display settings as they are now; a view that reads them redraws
/// when they change.
pub fn displays() -> RwLockReadGuard<'static, Config<Displays>> {
    Config::<Displays>::read()
}

/// Changes the display settings and saves them, for input handlers.
pub fn change_displays(edit: impl FnOnce(&mut Displays)) {
    Config::<Displays>::change(edit);
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
    /// No strip at all, as macOS 26's menu bar: the items sit on the
    /// wallpaper, light or dark to suit it.
    Clear,
}

/// When the status bar hides past its edge, as macOS's "Automatically hide
/// and show the menu bar".
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum AutoHide {
    #[default]
    Never,
    Always,
    /// While no window is on the bar's monitor's workspace.
    Desktop,
    /// While the focused window on its monitor is fullscreen.
    Fullscreen,
}

impl AutoHide {
    pub const ALL: [AutoHide; 4] = [Self::Never, Self::Always, Self::Desktop, Self::Fullscreen];
}

/// The setting was a switch once: `true` is Always, `false` Never.
impl<'de> Deserialize<'de> for AutoHide {
    fn deserialize<D: amane::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(crate = "amane::serde", untagged)]
        enum Either {
            Switch(bool),
            Mode(String),
        }
        Ok(match Either::deserialize(deserializer)? {
            Either::Switch(true) => Self::Always,
            Either::Switch(false) => Self::Never,
            Either::Mode(mode) => match mode.as_str() {
                "always" => Self::Always,
                "desktop" => Self::Desktop,
                "fullscreen" => Self::Fullscreen,
                _ => Self::Never,
            },
        })
    }
}

/// The clock's face.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum ClockStyle {
    #[default]
    Digital,
    /// A small drawn clock face, its time in the tooltip.
    Analog,
}

/// When the date shows before the time.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case")]
pub enum ShowDate {
    /// Unless the bar is short of room.
    WhenSpace,
    Always,
    #[default]
    Never,
}

impl ShowDate {
    pub const ALL: [ShowDate; 3] = [Self::WhenSpace, Self::Always, Self::Never];
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

    /// The order of the items on the right, by their names; ones missing
    /// keep their usual place among the rest.
    pub order: Vec<String>,

    /// The apps not allowed in the status bar, by their tray items' ids,
    /// as macOS's "Allow in the Menu Bar" leaves them out.
    pub tray_denied: Vec<String>,

    /// Now Playing shows with nothing playing too, as macOS's "Always
    /// Show"; else only while something plays or was paused lately.
    pub media_always: bool,

    /// When it hides past the screen's edge until the pointer meets it,
    /// as the dock can; hidden, it keeps no room from windows.
    pub auto_hide: AutoHide,

    pub clock_style: ClockStyle,
    pub show_date: ShowDate,
    /// The day of the week before the time.
    pub show_weekday: bool,
    /// The colons blink each second, as macOS's "Flash the time separators".
    pub flash_separators: bool,

    /// Today's weather for `weather_place`, from Open-Meteo.
    pub weather: bool,
    /// A place name, found once with Open-Meteo's geocoding.
    pub weather_place: String,

    /// The apps whose Live Activities stay out of the bar, by the name
    /// they give ("Now Playing", "Drop").
    pub activities_denied: Vec<String>,
    /// How many Live Activities show compact, with their text; the rest
    /// show as their icon alone.
    pub activities_compact: u8,
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
            order: Vec::new(),
            tray_denied: Vec::new(),
            media_always: false,
            auto_hide: AutoHide::Never,
            clock_style: ClockStyle::Digital,
            show_date: ShowDate::Never,
            show_weekday: false,
            flash_separators: false,
            weather: false,
            weather_place: String::new(),
            activities_denied: Vec::new(),
            activities_compact: 2,
        }
    }
}

impl StatusBar {
    /// The `strftime` pattern for what the clock shows: the day of the
    /// week and the date when they show (`dated` says whether there's room
    /// for a date shown "when space allows"), then the time.
    pub fn clock_pattern(&self, dated: bool) -> String {
        let date = match self.show_date {
            ShowDate::Always => true,
            ShowDate::WhenSpace => dated,
            ShowDate::Never => false,
        };
        let day = match (date, self.show_weekday) {
            (true, true) => "%a %-d %b  ",
            (true, false) => "%-d %b  ",
            (false, true) => "%a  ",
            (false, false) => "",
        };
        format!("{day}{}", self.time_pattern())
    }

    /// The `strftime` pattern for the time.
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

/// Every item on the status bar's right, by name, in its usual order,
/// with what Shell Settings calls it and whether it can be hidden.
pub const BAR_ITEMS: [(&str, &str, bool); 16] = [
    ("tray", "Apps", true),
    ("privacy", "Camera, Microphone and Screen Sharing", true),
    (
        "status-icons",
        "Night Light, VPN, Power Mode, Airplane",
        true,
    ),
    ("activities", "Live Activities", true),
    ("weather", "Weather", true),
    ("volume", "Volume", true),
    ("bluetooth", "Bluetooth", true),
    ("network", "Network", true),
    ("tailscale", "Tailscale, While Connected", true),
    ("keep-awake", "Keep Awake, While On", true),
    ("keyboard", "Keyboard Layout, With Two or More", true),
    ("phone", "Phone Battery, While Connected", true),
    ("battery", "Battery", true),
    ("quick-settings", "Quick Settings", false),
    ("notifications", "Notifications and Do Not Disturb", true),
    ("clock", "Clock", false),
];

impl StatusBar {
    /// The items' names in the order kept, ignoring names it doesn't
    /// know; the ones it leaves out each go back after the item they
    /// usually follow.
    pub fn items(&self) -> Vec<&'static str> {
        let known = |name: &str| {
            // Now Playing and the timer became Live Activities
            let name = match name {
                "media" | "timer" => "activities",
                name => name,
            };
            BAR_ITEMS
                .iter()
                .find(|item| item.0 == name)
                .map(|item| item.0)
        };
        let mut items: Vec<&'static str> = Vec::new();
        for name in &self.order {
            if let Some(name) = known(name)
                && !items.contains(&name)
            {
                items.push(name);
            }
        }
        for (index, (name, ..)) in BAR_ITEMS.into_iter().enumerate() {
            if items.contains(&name) {
                continue;
            }
            let after = BAR_ITEMS[..index]
                .iter()
                .rev()
                .find_map(|before| items.iter().position(|placed| *placed == before.0));
            items.insert(after.map_or(0, |at| at + 1), name);
        }
        items
    }

    /// The items' names with programs' own items among them (`app:<id>`,
    /// those running now): each where it was kept, and a new one just
    /// after the tray, as macOS adds a menu bar extra at the left.
    pub fn items_with(&self, apps: &[String]) -> Vec<String> {
        let mut items: Vec<String> = self.items().into_iter().map(String::from).collect();
        let mut fresh = 0;
        for app in apps {
            if items.contains(app) {
                continue;
            }
            let kept = self.order.iter().position(|name| name == app);
            let at = match kept {
                // after the nearest item kept before it that is here now
                Some(kept) => self.order[..kept]
                    .iter()
                    .rev()
                    .find_map(|before| items.iter().position(|placed| placed == before))
                    .map_or(0, |at| at + 1),
                None => {
                    let tray = items.iter().position(|name| name == "tray");
                    fresh += 1;
                    tray.map_or(fresh - 1, |tray| tray + fresh)
                }
            };
            items.insert(at.min(items.len()), app.clone());
        }
        items
    }

    /// Moves the item `name` to `index` among the others, programs' items
    /// `apps` too; the order kept remembers where each program's went.
    pub fn move_item_with(&mut self, name: &str, index: usize, apps: &[String]) {
        let mut items = self.items_with(apps);
        let Some(from) = items.iter().position(|item| item == name) else {
            return;
        };
        let item = items.remove(from);
        items.insert(index.min(items.len()), item);
        // programs not running now keep their places too, after what they followed
        for (kept, away) in self.order.iter().enumerate() {
            if !away.starts_with("app:") || items.contains(away) {
                continue;
            }
            let at = self.order[..kept]
                .iter()
                .rev()
                .find_map(|before| items.iter().position(|placed| placed == before))
                .map_or(0, |at| at + 1);
            items.insert(at, away.clone());
        }
        self.order = items;
    }

    /// Moves the item `name` to `index` among the others.
    pub fn move_item(&mut self, name: &str, index: usize) {
        let mut items = self.items();
        let Some(from) = items.iter().position(|item| *item == name) else {
            return;
        };
        let item = items.remove(from);
        items.insert(index.min(items.len()), item);
        self.order = items.into_iter().map(String::from).collect();
    }
}

impl StatusBar {
    /// Whether the app whose tray item has this id may show in the bar.
    pub fn allows(&self, id: &str) -> bool {
        !self.tray_denied.iter().any(|denied| denied == id)
    }

    pub fn set_allowed(&mut self, id: &str, allowed: bool) {
        self.tray_denied.retain(|denied| denied != id);

        if !allowed {
            self.tray_denied.push(String::from(id));
        }
    }
}

impl StatusBar {
    /// Whether the app by this name may show Live Activities in the bar.
    pub fn allows_activity(&self, app: &str) -> bool {
        !self.activities_denied.iter().any(|denied| denied == app)
    }

    pub fn set_activity_allowed(&mut self, app: &str, allowed: bool) {
        self.activities_denied.retain(|denied| denied != app);

        if !allowed {
            self.activities_denied.push(String::from(app));
        }
    }

    /// How many activities show compact, 1 to 3.
    pub fn compact_activities(&self) -> usize {
        usize::from(self.activities_compact.clamp(1, 3))
    }
}

/// The apps the bar's own Live Activities go under: what plays, the
/// launcher's timer, and screen sharing.
pub const BUILTIN_ACTIVITIES: [&str; 3] = ["Now Playing", "Timer", "Screen Sharing"];

/// A file in the shell's cache folder.
fn cache_file(name: &str) -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(cache.join("amane-shell").join(name))
}

/// Writes `text` to `file` whole, through a part file.
fn write_whole(file: &std::path::Path, text: &str) {
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let partial = file.with_extension("part");
    if std::fs::write(&partial, text).is_ok() {
        let _ = std::fs::rename(partial, file);
    }
}

/// Where the shell keeps the tray apps it has seen, so Shell Settings can
/// list them when they aren't running.
fn tray_seen_file() -> Option<PathBuf> {
    cache_file("tray-seen.json")
}

/// Every program seen with its own item in the bar (`ItemSpec`), as (id,
/// what it calls itself), by id.
pub fn status_apps_seen() -> BTreeMap<String, String> {
    cache_file("status-apps-seen.json")
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Remembers a program's item, once; a name it had before is kept when it
/// now has none.
pub fn remember_status_app(id: &str, name: &str) {
    let mut seen = status_apps_seen();
    if seen
        .get(id)
        .is_some_and(|known| name.is_empty() || known == name)
    {
        return;
    }
    seen.insert(String::from(id), String::from(name));
    let (Some(file), Ok(text)) = (
        cache_file("status-apps-seen.json"),
        serde_json::to_string_pretty(&seen),
    ) else {
        return;
    };
    write_whole(&file, &text);
}

/// Every app seen showing a Live Activity, by name, so Shell Settings can
/// list them when none is going on.
pub fn activities_seen() -> std::collections::BTreeSet<String> {
    cache_file("activities-seen.json")
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Remembers an app that showed a Live Activity, once.
pub fn remember_activity(app: &str) {
    let mut seen = activities_seen();
    if app.is_empty() || !seen.insert(String::from(app)) {
        return;
    }
    let (Some(file), Ok(text)) = (
        cache_file("activities-seen.json"),
        serde_json::to_string_pretty(&seen),
    ) else {
        return;
    };
    write_whole(&file, &text);
}

/// Every tray app seen, as (id, title), by id.
pub fn tray_seen() -> BTreeMap<String, String> {
    tray_seen_file()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Remembers a tray app, once; a title it had before is kept when it
/// now has none.
pub fn remember_tray(id: &str, title: &str) {
    let mut seen = tray_seen();
    let known = seen.get(id);
    if known.is_some_and(|known| title.is_empty() || known == title) {
        return;
    }
    seen.insert(String::from(id), String::from(title));
    let Some(file) = tray_seen_file() else {
        return;
    };
    let Ok(text) = serde_json::to_string_pretty(&seen) else {
        return;
    };
    write_whole(&file, &text);
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

// ───── Drop (amane-drop) ────────────────────────────────────────────────────

/// What a folder tile does with what's let go on it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum FolderMode {
    #[default]
    Copy,
    Move,
}

/// A tile of a Drop grid: somewhere to let files go, or something to do
/// with them, as Dropzone's actions.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case", tag = "kind")]
pub enum DropTile {
    /// Copies (or moves) what's let go into the folder; a click opens it.
    Folder {
        path: PathBuf,
        #[serde(default)]
        mode: FolderMode,
    },
    /// Opens what's let go with the app, by its desktop file id.
    App { id: String },
    /// Copies into a folder chosen once.
    CopyTo {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Moves into a folder chosen once.
    MoveTo {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Asks for a name, then moves the file there.
    RenameMove {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Saves text let go as a file there.
    SaveText {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Sends what's let go to a phone over KDE Connect: the one paired, or
    /// the one asked for when there are more.
    Phone {
        #[serde(default)]
        device: Option<String>,
    },
    /// Uploads what's let go and copies its link.
    Upload { to: UploadTo },
}

/// Where an upload tile sends files.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case")]
pub enum UploadTo {
    /// 0x0.st, the null pointer: no account, kept a while, then gone.
    #[serde(rename = "0x0")]
    ZeroX0,
    Imgur,
    Sftp,
    S3,
}

impl UploadTo {
    pub const ALL: [Self; 4] = [Self::ZeroX0, Self::Imgur, Self::Sftp, Self::S3];

    /// Its name on a tile and in Settings.
    pub fn title(self) -> &'static str {
        match self {
            Self::ZeroX0 => "0x0.st",
            Self::Imgur => "Imgur",
            Self::Sftp => "SFTP",
            Self::S3 => "S3",
        }
    }
}

/// One grid of tiles; each has a Drop Bar of its own.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct DropGrid {
    pub name: String,
    pub tiles: Vec<DropTile>,
}

impl Default for DropGrid {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let mut tiles = Vec::new();
        for folder in ["Downloads", "Desktop"] {
            let path = home.join(folder);
            if path.is_dir() {
                tiles.push(DropTile::Folder {
                    path,
                    mode: FolderMode::Copy,
                });
            }
        }
        tiles.extend([
            DropTile::CopyTo { folder: None },
            DropTile::MoveTo { folder: None },
            DropTile::RenameMove { folder: None },
            DropTile::SaveText { folder: None },
        ]);
        Self {
            name: String::from("Grid"),
            tiles,
        }
    }
}

/// amane-drop's grids, `~/.config/amane-drop/grids.json`, followed live by
/// the app and Shell Settings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct DropGrids {
    pub grids: Vec<DropGrid>,
    /// Tiles a row.
    pub columns: u8,
}

impl Default for DropGrids {
    fn default() -> Self {
        Self {
            grids: vec![DropGrid::default()],
            columns: 4,
        }
    }
}

impl Settings for DropGrids {
    const APP: &'static str = "amane-drop";
    const FILE: &'static str = "grids";
}

impl DropGrids {
    /// Tiles a row, 3 to 6.
    pub fn columns(&self) -> usize {
        usize::from(self.columns.clamp(3, 6))
    }
}

/// The grids as they are now; a view that reads them redraws when they change.
pub fn drop_grids() -> RwLockReadGuard<'static, Config<DropGrids>> {
    Config::<DropGrids>::read()
}

/// Changes the grids and saves them, for input handlers.
pub fn change_drop_grids(edit: impl FnOnce(&mut DropGrids)) {
    Config::<DropGrids>::change(edit);
}

/// The SFTP server Drop uploads to. A password, when there's no key, is
/// kept in the keyring as "sftp".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct SftpAccount {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// The folder files go in, on the server.
    pub folder: String,
    /// Where they can be read on the web, which a file's name is put after
    /// for its link; none, and an upload gives no link.
    pub url: String,
    /// A private key to log in with, else the password.
    pub key: Option<PathBuf>,
}

impl Default for SftpAccount {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            user: String::new(),
            folder: String::new(),
            url: String::new(),
            key: None,
        }
    }
}

/// The S3 bucket Drop uploads to, at AWS or any S3-compatible store. The
/// secret access key is kept in the keyring as "s3".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(crate = "amane::serde", default)]
pub struct S3Account {
    /// Another store's address (`https://<id>.r2.cloudflarestorage.com`),
    /// none for AWS.
    pub endpoint: String,
    /// `us-east-1` when none; R2's is `auto`.
    pub region: String,
    pub bucket: String,
    /// Put before each file's name: a folder in the bucket.
    pub prefix: String,
    pub key_id: String,
    /// Where the bucket can be read on the web, for links; none, and the
    /// bucket's own address is given.
    pub url: String,
    /// Uploads readable by anyone (`public-read`).
    pub public: bool,
}

/// amane-drop's accounts and what it does with a link,
/// `~/.config/amane-drop/sharing.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct DropSharing {
    pub sftp: SftpAccount,
    pub s3: S3Account,
    /// Imgur's client id for an app registered there (anonymous uploads).
    pub imgur_client_id: String,
    /// An upload's link goes on the clipboard.
    pub copy_link: bool,
    /// A notification says it's done.
    pub notify: bool,
}

impl Default for DropSharing {
    fn default() -> Self {
        Self {
            sftp: SftpAccount::default(),
            s3: S3Account::default(),
            imgur_client_id: String::new(),
            copy_link: true,
            notify: true,
        }
    }
}

impl Settings for DropSharing {
    const APP: &'static str = "amane-drop";
    const FILE: &'static str = "sharing";
}

impl DropSharing {
    /// What `to` still needs before it can upload, in a few words; none
    /// when it's ready. Secrets are checked when it runs.
    pub fn missing(&self, to: UploadTo) -> Option<&'static str> {
        match to {
            UploadTo::ZeroX0 => None,
            UploadTo::Imgur if self.imgur_client_id.trim().is_empty() => Some("No client ID"),
            UploadTo::Imgur => None,
            UploadTo::Sftp if self.sftp.host.trim().is_empty() => Some("No server"),
            UploadTo::Sftp if self.sftp.user.trim().is_empty() => Some("No user name"),
            UploadTo::Sftp => None,
            UploadTo::S3 if self.s3.bucket.trim().is_empty() => Some("No bucket"),
            UploadTo::S3 if self.s3.key_id.trim().is_empty() => Some("No access key"),
            UploadTo::S3 => None,
        }
    }
}

/// The accounts as they are now.
pub fn drop_sharing() -> RwLockReadGuard<'static, Config<DropSharing>> {
    Config::<DropSharing>::read()
}

/// Changes the accounts and saves them.
pub fn change_drop_sharing(edit: impl FnOnce(&mut DropSharing)) {
    Config::<DropSharing>::change(edit);
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

#[cfg(test)]
mod bar_order_tests {
    use super::*;

    fn bar(order: &[&str]) -> StatusBar {
        StatusBar {
            order: order.iter().map(|name| String::from(*name)).collect(),
            ..StatusBar::default()
        }
    }

    #[test]
    fn no_order_is_the_usual_one() {
        assert_eq!(bar(&[]).items(), BAR_ITEMS.map(|item| item.0));
    }

    #[test]
    fn items_left_out_follow_their_usual_neighbour() {
        let items = bar(&["clock", "volume", "nonsense", "battery"]).items();
        assert_eq!(items.len(), BAR_ITEMS.len());
        assert_eq!(
            &items[..6],
            [
                "tray",
                "privacy",
                "status-icons",
                "activities",
                "weather",
                "clock"
            ]
        );
        let volume = items.iter().position(|item| *item == "volume").unwrap();
        assert_eq!(items[volume + 1], "bluetooth");
        let battery = items.iter().position(|item| *item == "battery").unwrap();
        assert_eq!(items[battery + 1], "quick-settings");
    }

    #[test]
    fn now_playing_and_the_timer_became_live_activities() {
        let items = bar(&["timer", "clock", "media"]).items();
        assert_eq!(items.len(), BAR_ITEMS.len());
        let at = |name| items.iter().position(|item| *item == name);
        assert!(at("activities") < at("clock"));
        assert!(at("media").is_none() && at("timer").is_none());
    }

    #[test]
    fn drop_tiles_read_as_written() {
        let grids = DropGrids::default();
        let text = serde_json::to_string(&grids).unwrap();
        assert_eq!(serde_json::from_str::<DropGrids>(&text).unwrap(), grids);
        let tile: DropTile = serde_json::from_str(r#"{"kind":"folder","path":"/tmp"}"#).unwrap();
        assert_eq!(
            tile,
            DropTile::Folder {
                path: PathBuf::from("/tmp"),
                mode: FolderMode::Copy
            }
        );
        assert_eq!(grids.columns(), 4);
        let upload: DropTile = serde_json::from_str(r#"{"kind":"upload","to":"0x0"}"#).unwrap();
        assert_eq!(
            upload,
            DropTile::Upload {
                to: UploadTo::ZeroX0
            }
        );
        let phone: DropTile = serde_json::from_str(r#"{"kind":"phone"}"#).unwrap();
        assert_eq!(phone, DropTile::Phone { device: None });
    }

    #[test]
    fn an_account_says_what_it_lacks() {
        let mut sharing = DropSharing::default();
        assert_eq!(sharing.missing(UploadTo::ZeroX0), None);
        assert_eq!(sharing.missing(UploadTo::Sftp), Some("No server"));
        sharing.sftp.host = String::from("example.org");
        sharing.sftp.user = String::from("me");
        assert_eq!(sharing.missing(UploadTo::Sftp), None);
        assert_eq!(sharing.sftp.port, 22);
        assert!(sharing.copy_link);
    }

    #[test]
    fn activities_are_allowed_until_denied() {
        let mut now = StatusBar::default();
        assert!(now.allows_activity("Drop"));
        now.set_activity_allowed("Drop", false);
        now.set_activity_allowed("Drop", false);
        assert_eq!(now.activities_denied, ["Drop"]);
        now.set_activity_allowed("Drop", true);
        assert!(now.allows_activity("Drop"));
        assert_eq!(now.compact_activities(), 2);
        now.activities_compact = 9;
        assert_eq!(now.compact_activities(), 3);
    }

    #[test]
    fn a_new_program_goes_after_the_tray_and_keeps_its_place() {
        let apps = [String::from("app:org.amane.drop")];
        let mut now = bar(&[]);
        let items = now.items_with(&apps);
        assert_eq!(items.len(), BAR_ITEMS.len() + 1);
        assert_eq!(&items[..2], ["tray", "app:org.amane.drop"]);

        now.move_item_with("app:org.amane.drop", 5, &apps);
        assert_eq!(now.items_with(&apps)[5], "app:org.amane.drop");

        // gone a while, it still has its place when it's back
        now.move_item_with("clock", 0, &[]);
        assert!(now.order.contains(&apps[0]));
        let items = now.items_with(&apps);
        let at = |name: &str| items.iter().position(|item| item == name);
        // one on, as the clock went before it
        assert_eq!(at("app:org.amane.drop"), Some(6));
        assert_eq!(now.items().len(), BAR_ITEMS.len());
    }

    #[test]
    fn moving_keeps_every_item_once() {
        let mut now = bar(&[]);
        now.move_item("clock", 0);
        assert_eq!(now.items()[0], "clock");
        assert_eq!(now.items().len(), BAR_ITEMS.len());
        now.move_item("clock", 99);
        assert_eq!(now.items().last(), Some(&"clock"));
    }
}

#[cfg(test)]
mod bar_setting_tests {
    use super::*;

    #[test]
    fn hiding_was_a_switch_once() {
        let old: StatusBar = serde_json::from_str(r#"{ "auto_hide": true }"#).unwrap();
        assert_eq!(old.auto_hide, AutoHide::Always);
        let old: StatusBar = serde_json::from_str(r#"{ "auto_hide": false }"#).unwrap();
        assert_eq!(old.auto_hide, AutoHide::Never);
        let new: StatusBar = serde_json::from_str(r#"{ "auto_hide": "fullscreen" }"#).unwrap();
        assert_eq!(new.auto_hide, AutoHide::Fullscreen);
        let text = serde_json::to_string(&new).unwrap();
        assert!(text.contains(r#""auto_hide":"fullscreen""#));
    }

    #[test]
    fn the_clock_says_the_day_when_asked() {
        let mut bar = StatusBar::default();
        assert_eq!(bar.clock_pattern(true), "%H:%M");
        bar.show_weekday = true;
        assert_eq!(bar.clock_pattern(true), "%a  %H:%M");
        bar.show_date = ShowDate::WhenSpace;
        assert_eq!(bar.clock_pattern(true), "%a %-d %b  %H:%M");
        assert_eq!(bar.clock_pattern(false), "%a  %H:%M");
        bar.show_date = ShowDate::Always;
        assert_eq!(bar.clock_pattern(false), "%a %-d %b  %H:%M");
    }
}
