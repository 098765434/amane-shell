//! What a widget is: its kind (what it watches), the sizes it comes in,
//! and the key one placed on the desktop is known by. Two widgets of the
//! same kind are two keys, each with its own settings.

use amane::{Battery, Service};

use super::catalog::{self, units};
use crate::services::desktop::system_stats::SystemStats;
use crate::services::installed;
use crate::surfaces::quick_settings::{self, Page};
use emaki_kit::grid::GridKey;
use emaki_kit::grid::geometry::Span;
use emaki_kit::icons::{ICON_BATTERY_FULL, ICON_COMPUTER};
use emaki_kit::settings::ACCENTS;

// The icons the font has that the kit doesn't name yet.
pub const ICON_CPU: char = '\u{e30d}';
pub const ICON_MEMORY: char = '\u{e322}';
pub const ICON_THERMOSTAT: char = '\u{f076}';
pub const ICON_DISK: char = '\u{f80e}';
pub const ICON_NETWORK: char = '\u{e8d5}';
pub const ICON_SCHEDULE: char = '\u{e8b5}';

/// The widget sizes on the desktop's grid: a cell is Quick Settings'
/// (74 px, 16 px between), so Small is 164 × 164.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetSize {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

impl WidgetSize {
    pub const ALL: [WidgetSize; 4] = [Self::Small, Self::Medium, Self::Large, Self::ExtraLarge];

    pub const fn span(self) -> Span {
        match self {
            Self::Small => Span::new(2, 2),
            Self::Medium => Span::new(4, 2),
            Self::Large => Span::new(4, 4),
            Self::ExtraLarge => Span::new(8, 4),
        }
    }

    /// The size a span is, or Small for one no size has.
    pub fn of(span: Span) -> Self {
        Self::ALL
            .into_iter()
            .find(|size| size.span() == span)
            .unwrap_or(Self::Small)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::ExtraLarge => "extra-large",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.name() == name)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::ExtraLarge => "Extra Large",
        }
    }
}

const SPANS_S_M_L: [Span; 3] = [
    WidgetSize::Small.span(),
    WidgetSize::Medium.span(),
    WidgetSize::Large.span(),
];
const SPANS_S_M: [Span; 2] = [WidgetSize::Small.span(), WidgetSize::Medium.span()];
const SPANS_L_XL: [Span; 2] = [WidgetSize::Large.span(), WidgetSize::ExtraLarge.span()];

/// The gallery's sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Clock,
    System,
    Battery,
}

impl Category {
    /// The gallery's sections, in its order.
    pub const ALL: [Category; 3] = [Self::Clock, Self::System, Self::Battery];

    /// Its key in the gallery's sidebar.
    pub fn name(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::System => "system",
            Self::Battery => "battery",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Clock => "Clock",
            Self::System => "System",
            Self::Battery => "Batteries",
        }
    }

    pub fn glyph(self) -> char {
        match self {
            Self::Clock => ICON_SCHEDULE,
            Self::System => ICON_COMPUTER,
            Self::Battery => ICON_BATTERY_FULL,
        }
    }
}

/// What a widget watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    Cpu,
    Memory,
    Temperature,
    Network,
    Disk,
    /// CPU, memory, network and disk on one card.
    System,
    /// The machine's battery and the peripherals'.
    Battery,
    /// The time drawn in pen strokes, each digit changing like a pen.
    Clock,
}

/// What a kind reads from the system's readings (`SystemStats`), which
/// go on only while a card that reads them shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reads {
    Nothing,
    /// The processor, memory, network and disks.
    System,
    /// Those and the temperature sensors, which cost more to read.
    Temperatures,
}

/// Everything a kind is, in one place: a new kind is one more of these
/// (and its face, in `catalog`), each field said, so nothing about it falls
/// to a default it didn't choose.
pub struct KindSpec {
    /// The name kept in the layout file and used over IPC.
    pub name: &'static str,
    pub title: &'static str,
    /// One line on what it shows, for the gallery.
    pub blurb: &'static str,
    pub glyph: char,
    pub category: Category,
    /// The sizes it comes in, smallest first.
    pub sizes: &'static [Span],
    pub default_size: WidgetSize,
    /// Whether this machine has what it watches.
    pub available: fn() -> bool,
    /// What a click on it opens.
    pub open: fn(),
    pub reads: Reads,
    /// What it can be set to, in its menu's order.
    pub settings: &'static [SettingSpec],
}

/// One thing a widget can be set to, kept under `key` in its settings.
pub struct SettingSpec {
    pub key: &'static str,
    /// Its menu item: a submenu's title, or a switch's.
    pub title: &'static str,
    pub kind: SettingKind,
}

pub enum SettingKind {
    /// One of `options` (each its value and its name), or `automatic`, kept
    /// as no value: a submenu with the one set checked.
    Choice {
        automatic: &'static str,
        options: fn() -> Vec<(String, String)>,
    },
    /// On (kept as `value`) or off (no value); its switch is checked while
    /// it's on, or while it's off when `inverted` ("24-Hour Time" over
    /// `hours: 12`).
    Toggle { value: &'static str, inverted: bool },
}

impl SettingSpec {
    /// Whether `value` is one this setting takes: none (automatic, off),
    /// a toggle's own value, or for a choice anything (a device that comes
    /// and goes is still the one meant).
    pub fn accepts(&self, value: Option<&str>) -> bool {
        match (&self.kind, value) {
            (_, None) | (SettingKind::Choice { .. }, Some(_)) => true,
            (SettingKind::Toggle { value: on, .. }, Some(value)) => *on == value,
        }
    }
}

const fn choice(
    key: &'static str,
    title: &'static str,
    automatic: &'static str,
    options: fn() -> Vec<(String, String)>,
) -> SettingSpec {
    SettingSpec {
        key,
        title,
        kind: SettingKind::Choice { automatic, options },
    }
}

const fn toggle(
    key: &'static str,
    title: &'static str,
    value: &'static str,
    inverted: bool,
) -> SettingSpec {
    SettingSpec {
        key,
        title,
        kind: SettingKind::Toggle { value, inverted },
    }
}

/// The same name as value and title, for lists of what's there.
fn same(names: impl IntoIterator<Item = String>) -> Vec<(String, String)> {
    names.into_iter().map(|name| (name.clone(), name)).collect()
}

fn devices() -> Vec<(String, String)> {
    catalog::devices()
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect()
}

fn interfaces() -> Vec<(String, String)> {
    SystemStats::read()
        .interfaces()
        .into_iter()
        .map(|name| (name.to_string(), units::interface_name(name)))
        .collect()
}

fn mounts() -> Vec<(String, String)> {
    same(SystemStats::read().disks.iter().map(|d| d.mount.clone()))
}

fn sensors() -> Vec<(String, String)> {
    SystemStats::read()
        .sensors
        .iter()
        .map(|s| (s.label.clone(), units::sensor_name(&s.label)))
        .collect()
}

fn ranges() -> Vec<(String, String)> {
    vec![(String::from("15"), String::from("Last 15 Minutes"))]
}

fn colors() -> Vec<(String, String)> {
    ACCENTS
        .iter()
        .filter_map(|(name, hex)| Some((hex.as_ref()?.to_string(), name.to_string())))
        .collect()
}

fn lines() -> Vec<(String, String)> {
    [("thin", "Thin"), ("bold", "Bold")]
        .into_iter()
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect()
}

fn always() -> bool {
    true
}

fn has_battery() -> bool {
    Battery::read().present() || !catalog::devices().is_empty()
}

fn nothing() {}

/// The first of `programs` installed, launched.
fn launch_first(programs: &[&'static str]) {
    if let Some(program) = programs.iter().copied().find(|p| installed(p)) {
        crate::services::launch(&[program]);
    }
}

fn system_monitor() {
    launch_first(&["missioncenter", "gnome-system-monitor", "resources"]);
}

fn power_page() {
    quick_settings::open_alone(Page::Power);
}

fn network_settings() {
    let settings = emaki_kit::app_ids::SETTINGS;
    if installed(settings) {
        amane::Launch::argv(&[settings, "--page", "network"]).activated();
    } else {
        launch_first(&["nm-connection-editor"]);
    }
}

fn disk_usage() {
    launch_first(&["baobab", "gnome-disks"]);
}

/// The readings every system kind shares but its own.
const fn reading(
    name: &'static str,
    title: &'static str,
    blurb: &'static str,
    glyph: char,
    settings: &'static [SettingSpec],
) -> KindSpec {
    KindSpec {
        name,
        title,
        blurb,
        glyph,
        category: Category::System,
        sizes: &SPANS_S_M_L,
        default_size: WidgetSize::Small,
        available: always,
        open: system_monitor,
        reads: Reads::System,
        settings,
    }
}

/// How far back a graph goes: two minutes, or a quarter hour.
const RANGE: SettingSpec = choice("range", "Graph", "Last 2 Minutes", ranges);

const CPU: KindSpec = reading(
    "cpu",
    "CPU",
    "How busy the processor is, and lately.",
    ICON_CPU,
    &[RANGE],
);

const MEMORY: KindSpec = reading(
    "memory",
    "Memory",
    "Memory and swap in use.",
    ICON_MEMORY,
    &[RANGE],
);

const TEMPERATURE: KindSpec = KindSpec {
    reads: Reads::Temperatures,
    ..reading(
        "temperature",
        "Temperature",
        "How warm the processor is, or the hottest sensor.",
        ICON_THERMOSTAT,
        &[
            choice("sensor", "Show", "Processor", sensors),
            RANGE,
            toggle("unit", "Fahrenheit", "f", false),
        ],
    )
};

const NETWORK: KindSpec = KindSpec {
    open: network_settings,
    ..reading(
        "network",
        "Network",
        "What comes in and goes out.",
        ICON_NETWORK,
        &[
            choice("interface", "Show", "Default Route", interfaces),
            RANGE,
            toggle("unit", "Bits per Second", "bits", false),
        ],
    )
};

const DISK: KindSpec = KindSpec {
    sizes: &SPANS_S_M,
    open: disk_usage,
    ..reading(
        "disk",
        "Disk",
        "How full a disk is.",
        ICON_DISK,
        &[choice("mount", "Show", "Main Disk", mounts)],
    )
};

const SYSTEM: KindSpec = KindSpec {
    sizes: &SPANS_L_XL,
    default_size: WidgetSize::Large,
    ..reading(
        "system",
        "System",
        "CPU, memory, network and disks at once.",
        ICON_COMPUTER,
        &[],
    )
};

const BATTERY: KindSpec = KindSpec {
    name: "battery",
    title: "Batteries",
    blurb: "The battery and your devices' charge.",
    glyph: ICON_BATTERY_FULL,
    category: Category::Battery,
    sizes: &SPANS_S_M_L,
    default_size: WidgetSize::Small,
    available: has_battery,
    open: power_page,
    reads: Reads::Nothing,
    settings: &[choice("device", "Show", "First Device", devices)],
};

const CLOCK: KindSpec = KindSpec {
    name: "clock",
    title: "Clock",
    blurb: "The time, each digit drawn like a pen.",
    glyph: ICON_SCHEDULE,
    category: Category::Clock,
    sizes: &SPANS_S_M_L,
    default_size: WidgetSize::Medium,
    available: always,
    // nothing to open: the time is all it shows
    open: nothing,
    reads: Reads::Nothing,
    settings: &[
        choice("color", "Color", "Accent", colors),
        choice("line", "Line", "Regular", lines),
        toggle("seconds", "Show Seconds", "on", false),
        toggle("hours", "24-Hour Time", "12", true),
        toggle("date", "Show Date", "off", true),
    ],
};

impl WidgetKind {
    /// The gallery's order.
    pub const ALL: [WidgetKind; 8] = [
        Self::Clock,
        Self::Battery,
        Self::Cpu,
        Self::Memory,
        Self::Temperature,
        Self::Network,
        Self::Disk,
        Self::System,
    ];

    /// Everything this kind is.
    pub const fn spec(self) -> &'static KindSpec {
        match self {
            Self::Cpu => &CPU,
            Self::Memory => &MEMORY,
            Self::Temperature => &TEMPERATURE,
            Self::Network => &NETWORK,
            Self::Disk => &DISK,
            Self::System => &SYSTEM,
            Self::Battery => &BATTERY,
            Self::Clock => &CLOCK,
        }
    }

    /// The name kept in the layout file and used over IPC.
    pub fn name(self) -> &'static str {
        self.spec().name
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    pub fn title(self) -> &'static str {
        self.spec().title
    }

    /// One line on what it shows, for the gallery.
    pub fn blurb(self) -> &'static str {
        self.spec().blurb
    }

    pub fn glyph(self) -> char {
        self.spec().glyph
    }

    pub fn category(self) -> Category {
        self.spec().category
    }

    /// The sizes it comes in, smallest first.
    pub fn supported(self) -> &'static [Span] {
        self.spec().sizes
    }

    pub fn sizes(self) -> impl Iterator<Item = WidgetSize> {
        self.supported().iter().map(|span| WidgetSize::of(*span))
    }

    pub fn default_size(self) -> WidgetSize {
        self.spec().default_size
    }

    /// Whether this machine has what it watches.
    pub fn available(self) -> bool {
        (self.spec().available)()
    }

    /// What a click on it opens: its Quick Settings page, or the app that
    /// shows more of what it watches, when one is installed.
    pub fn open(self) {
        (self.spec().open)();
    }

    pub fn reads(self) -> Reads {
        self.spec().reads
    }

    /// Its setting kept under `key`, if it has one.
    pub fn setting(self, key: &str) -> Option<&'static SettingSpec> {
        self.spec()
            .settings
            .iter()
            .find(|setting| setting.key == key)
    }
}

/// A widget on the desktop: its own id, and what it watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetKey {
    pub id: u32,
    pub kind: WidgetKind,
}

impl GridKey for WidgetKey {
    fn supported(&self) -> &'static [Span] {
        self.kind.supported()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_unique_name_and_its_default_size() {
        let mut names = std::collections::HashSet::new();
        for kind in WidgetKind::ALL {
            assert!(names.insert(kind.name()));
            assert_eq!(WidgetKind::named(kind.name()), Some(kind));
            assert!(kind.supported().contains(&kind.default_size().span()));
            assert!(
                kind.supported()
                    .windows(2)
                    .all(|w| w[0].cells() < w[1].cells())
            );
        }
        assert_eq!(WidgetKind::named("nope"), None);
    }

    #[test]
    fn every_setting_has_a_key_of_its_own_and_takes_none() {
        for kind in WidgetKind::ALL {
            let keys: Vec<&str> = kind.spec().settings.iter().map(|s| s.key).collect();
            let mut unique = keys.clone();
            unique.dedup();
            assert_eq!(keys, unique, "{kind:?}");
            for setting in kind.spec().settings {
                assert!(setting.accepts(None));
                assert_eq!(kind.setting(setting.key).map(|s| s.key), Some(setting.key));
            }
        }
        let seconds = WidgetKind::Clock.setting("seconds").unwrap();
        assert!(seconds.accepts(Some("on")) && !seconds.accepts(Some("yes")));
        assert!(WidgetKind::Cpu.setting("sensor").is_none());
    }

    #[test]
    fn sizes_are_macos_on_the_grid() {
        for size in WidgetSize::ALL {
            assert_eq!(WidgetSize::of(size.span()), size);
            assert_eq!(WidgetSize::named(size.name()), Some(size));
        }
        assert_eq!(WidgetSize::Medium.span(), Span::new(4, 2));
        assert_eq!(WidgetSize::ExtraLarge.span(), Span::new(8, 4));
    }
}
