//! `status-bar.json`: where the bar is, how it looks, its items, clock, activities and
//! Quick Settings' controls.

use super::*;

/// Which edge of the screen the status bar runs along.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum BarPosition {
    /// Along the bottom of the screen.
    #[default]
    Bottom,
    /// Along the top.
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
    /// No strip at all: the items sit on the
    /// wallpaper, light or dark to suit it.
    Clear,
}

/// When the status bar hides past its edge, and shows again.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum AutoHide {
    /// It always shows.
    #[default]
    Never,
    /// It hides until the pointer meets its edge.
    Always,
    /// While no window is on the bar's monitor's workspace.
    Desktop,
    /// While the focused window on its monitor is fullscreen.
    Fullscreen,
}

impl AutoHide {
    /// Every choice, in the order Settings lists them.
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

/// How tall the status bar is, and how big its icons are.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum BarSize {
    /// 18 px with 16 px icons, the size it always was.
    #[default]
    Compact,
    /// 24 px with 16 px icons.
    Regular,
    /// 32 px with 24 px icons.
    Comfortable,
}

impl BarSize {
    /// Every size, smallest first.
    pub const ALL: [BarSize; 3] = [Self::Compact, Self::Regular, Self::Comfortable];

    /// How tall the bar is: `sys.size.panel.height` compact.
    /// TODO(token): `bar.size.regular.height`, `bar.size.comfortable.height`.
    pub fn height(self) -> f32 {
        match self {
            Self::Compact => crate::tokens::sys::size::panel::height(),
            Self::Regular => 24.0,
            Self::Comfortable => 32.0,
        }
    }

    /// Its name in Emaki Settings.
    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Regular => "Regular",
            Self::Comfortable => "Comfortable",
        }
    }
}

/// The clock's face.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum ClockStyle {
    /// The time in figures.
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
    /// Always.
    Always,
    /// Never.
    #[default]
    Never,
}

impl ShowDate {
    /// Every choice, in the order Settings lists them.
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
    /// Every format, longest first.
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
    /// Which edge it's on.
    pub position: BarPosition,
    /// What it's made of.
    pub style: BarStyle,
    /// How tall it is.
    pub size: BarSize,

    /// 24-hour time, else 12-hour with AM and PM.
    pub clock_24h: bool,
    /// Whether the clock shows seconds.
    pub clock_seconds: bool,
    /// How the date reads.
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

    /// The apps not allowed in the status bar, by their tray items' ids.
    pub tray_denied: Vec<String>,

    /// Now Playing shows with nothing playing too ("Always Show"); else
    /// only while something plays or was paused lately.
    pub media_always: bool,

    /// When it hides past the screen's edge until the pointer meets it,
    /// as the dock can; hidden, it keeps no room from windows.
    pub auto_hide: AutoHide,

    /// Figures or a clock face.
    pub clock_style: ClockStyle,
    /// When the date shows.
    pub show_date: ShowDate,
    /// The day of the week before the time.
    pub show_weekday: bool,
    /// The colons blink each second ("Flash the time separators").
    pub flash_separators: bool,

    /// Today's weather for `weather_place`, from Open-Meteo.
    pub weather: bool,
    /// A place name, found once with Open-Meteo's geocoding.
    pub weather_place: String,

    /// The apps whose activities stay out of the bar, by the name
    /// they give ("Now Playing", "Drop").
    pub activities_denied: Vec<String>,
    /// How many activities show compact, with their text; the rest
    /// show as their icon alone.
    pub activities_compact: u8,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            position: BarPosition::Bottom,
            style: BarStyle::Frosted,
            size: BarSize::Compact,
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
/// with what Emaki Settings calls it and whether it can be hidden.
pub const BAR_ITEMS: [(&str, &str, bool); 16] = [
    ("tray", "Apps", true),
    ("privacy", "Camera, Microphone and Screen Sharing", true),
    (
        "status-icons",
        "Night Light, VPN, Power Mode, Airplane",
        true,
    ),
    ("activities", "Activities", true),
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

/// Every Quick Settings control, by the shell's name for it, with what
/// Emaki Settings calls it and the glyph its tile shows at rest.
pub const QS_CONTROLS: [(&str, &str, Glyph, Accent); 22] = [
    ("wifi", "Wi-Fi", Glyph::Font(icons::ICON_WIFI), Accent::Blue),
    (
        "bluetooth",
        "Bluetooth",
        Glyph::Font(icons::ICON_BLUETOOTH),
        Accent::Blue,
    ),
    (
        "now-playing",
        "Now Playing",
        Glyph::Font(icons::ICON_MUSIC),
        Accent::Pink,
    ),
    ("vpn", "VPN", Glyph::Font(icons::ICON_VPN), Accent::Blue),
    (
        "wired",
        "Wired network",
        Glyph::Font(icons::ICON_WIRED),
        Accent::Blue,
    ),
    (
        "focus",
        "Do Not Disturb",
        Glyph::Font(icons::ICON_DND),
        Accent::Indigo,
    ),
    (
        "dark-style",
        "Dark Style",
        Glyph::Font(icons::ICON_DARK_MODE),
        Accent::Gray,
    ),
    (
        "night-light",
        "Night Light",
        Glyph::Font(icons::ICON_NIGHT_LIGHT),
        Accent::Orange,
    ),
    (
        "display",
        "Display brightness",
        Glyph::Font(icons::ICON_BRIGHTNESS_HIGH),
        Accent::Blue,
    ),
    (
        "sound",
        "Sound",
        Glyph::Font(icons::ICON_VOLUME_UP),
        Accent::Pink,
    ),
    (
        "battery",
        "Battery",
        Glyph::Font(icons::ICON_BATTERY_FULL),
        Accent::Green,
    ),
    (
        "power-mode",
        "Power Mode",
        Glyph::Font(icons::ICON_BALANCED),
        Accent::Green,
    ),
    (
        "keyboard",
        "Keyboard brightness",
        Glyph::Font(icons::ICON_KEYBOARD_LIGHT),
        Accent::Gray,
    ),
    (
        "airplane",
        "Airplane Mode",
        Glyph::Font(icons::ICON_AIRPLANE),
        Accent::Orange,
    ),
    (
        "background-apps",
        "Background apps",
        Glyph::Font(icons::ICON_BACKGROUND_APPS),
        Accent::Gray,
    ),
    (
        "screenshot",
        "Screenshot",
        Glyph::Font(icons::ICON_SCREENSHOT),
        Accent::Gray,
    ),
    ("lock", "Lock", Glyph::Font(icons::ICON_LOCK), Accent::Gray),
    (
        "power-off",
        "Power",
        Glyph::Font(icons::ICON_POWER),
        Accent::Red,
    ),
    ("tailscale", "Tailscale", Glyph::Tailscale, Accent::Gray),
    (
        "keep-awake",
        "Keep Awake",
        Glyph::Font(icons::ICON_KEEP_AWAKE),
        Accent::Brown,
    ),
    (
        "phone",
        "Phone",
        Glyph::Font(icons::ICON_PHONE),
        Accent::Green,
    ),
    (
        "software-update",
        "Software Update",
        Glyph::Font(icons::ICON_UPDATE),
        Accent::Gray,
    ),
];

impl StatusBar {
    /// The items' names in the order kept, ignoring names it doesn't
    /// know; the ones it leaves out each go back after the item they
    /// usually follow.
    pub fn items(&self) -> Vec<&'static str> {
        let known = |name: &str| {
            // Now Playing and the timer became activities
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
    /// after the tray, at the left of the apps' items.
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

    /// Moves the item `name` to `index` among the bar's own items;
    /// programs' items keep their places in the order.
    pub fn move_item(&mut self, name: &str, index: usize) {
        self.move_item_with(name, index, &[]);
    }
}

impl StatusBar {
    /// Whether the app whose tray item has this id may show in the bar.
    pub fn allows(&self, id: &str) -> bool {
        !self.tray_denied.iter().any(|denied| denied == id)
    }

    /// Lets the app with this tray item id show in the bar, or keeps it out.
    pub fn set_allowed(&mut self, id: &str, allowed: bool) {
        self.tray_denied.retain(|denied| denied != id);

        if !allowed {
            self.tray_denied.push(String::from(id));
        }
    }
}

impl StatusBar {
    /// Whether the app by this name may show activities in the bar.
    pub fn allows_activity(&self, app: &str) -> bool {
        !self.activities_denied.iter().any(|denied| denied == app)
    }

    /// Lets the app by this name show activities, or keeps them out.
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

/// The apps the bar's own activities go under: what plays, the
/// launcher's timer, and screen sharing.
pub const BUILTIN_ACTIVITIES: [&str; 3] = ["Now Playing", "Timer", "Screen Sharing"];

settings_file!(
    StatusBar,
    crate::app_ids::SHELL,
    "status-bar",
    status_bar,
    change_status_bar
);
