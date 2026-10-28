//! What a widget is: its kind (what it watches), the sizes it comes in,
//! and the key one placed on the desktop is known by. Two widgets of the
//! same kind are two keys, each with its own settings.

use amane::{Battery, Service};

use crate::components::grid::GridKey;
use crate::components::grid::geometry::Span;
use crate::components::icons::{ICON_BATTERY_FULL, ICON_COMPUTER};
use crate::surfaces::quick_settings::{self, Page};
use crate::services::installed;

// The icons the font has that the kit doesn't name yet.
pub const ICON_CPU: char = '\u{e30d}';
pub const ICON_MEMORY: char = '\u{e322}';
pub const ICON_THERMOSTAT: char = '\u{f076}';
pub const ICON_DISK: char = '\u{f80e}';
pub const ICON_NETWORK: char = '\u{e8d5}';
pub const ICON_SCHEDULE: char = '\u{e8b5}';

/// macOS's widget sizes on the desktop's grid: a cell is Quick Settings'
/// (74 px, 16 px between), so Small is 164 × 164, near macOS's 170 pt.
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

    /// The name kept in the layout file and used over IPC.
    pub fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Temperature => "temperature",
            Self::Network => "network",
            Self::Disk => "disk",
            Self::System => "system",
            Self::Battery => "battery",
            Self::Clock => "clock",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Temperature => "Temperature",
            Self::Network => "Network",
            Self::Disk => "Disk",
            Self::System => "System",
            Self::Battery => "Batteries",
            Self::Clock => "Clock",
        }
    }

    /// One line on what it shows, for the gallery.
    pub fn blurb(self) -> &'static str {
        match self {
            Self::Cpu => "How busy the processor is, and lately.",
            Self::Memory => "Memory and swap in use.",
            Self::Temperature => "How warm the hottest sensor is.",
            Self::Network => "What comes in and goes out.",
            Self::Disk => "How full a disk is.",
            Self::System => "CPU, memory, network and disks at once.",
            Self::Battery => "The battery and your devices' charge.",
            Self::Clock => "The time, each digit drawn like a pen.",
        }
    }

    pub fn glyph(self) -> char {
        match self {
            Self::Cpu => ICON_CPU,
            Self::Memory => ICON_MEMORY,
            Self::Temperature => ICON_THERMOSTAT,
            Self::Network => ICON_NETWORK,
            Self::Disk => ICON_DISK,
            Self::System => ICON_COMPUTER,
            Self::Battery => ICON_BATTERY_FULL,
            Self::Clock => ICON_SCHEDULE,
        }
    }

    pub fn category(self) -> Category {
        match self {
            Self::Battery => Category::Battery,
            Self::Clock => Category::Clock,
            _ => Category::System,
        }
    }

    /// The sizes it comes in, smallest first.
    pub fn supported(self) -> &'static [Span] {
        match self {
            Self::Disk => &SPANS_S_M,
            Self::System => &SPANS_L_XL,
            _ => &SPANS_S_M_L,
        }
    }

    pub fn sizes(self) -> impl Iterator<Item = WidgetSize> {
        self.supported().iter().map(|span| WidgetSize::of(*span))
    }

    pub fn default_size(self) -> WidgetSize {
        match self {
            Self::System => WidgetSize::Large,
            Self::Clock => WidgetSize::Medium,
            _ => WidgetSize::Small,
        }
    }

    /// Whether this machine has what it watches.
    pub fn available(self) -> bool {
        match self {
            Self::Battery => Battery::read().present() || !Battery::read().peripherals().is_empty(),
            _ => true,
        }
    }

    /// What a click on it opens: its Quick Settings page, or the app that
    /// shows more of what it watches, when one is installed.
    pub fn open(self) {
        let first = |programs: &[&'static str]| programs.iter().copied().find(|p| installed(p));
        match self {
            Self::Battery => quick_settings::open(Page::Power),
            // nothing to open: the time is all it shows
            Self::Clock => {}
            Self::Network => {
                if let Some(program) = first(&["nm-connection-editor"]) {
                    amane::spawn(program);
                }
            }
            Self::Disk => {
                if let Some(program) = first(&["baobab", "gnome-disks"]) {
                    amane::spawn(program);
                }
            }
            _ => {
                if let Some(program) =
                    first(&["missioncenter", "gnome-system-monitor", "resources"])
                {
                    amane::spawn(program);
                }
            }
        }
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
    fn sizes_are_macos_on_the_grid() {
        for size in WidgetSize::ALL {
            assert_eq!(WidgetSize::of(size.span()), size);
            assert_eq!(WidgetSize::named(size.name()), Some(size));
        }
        assert_eq!(WidgetSize::Medium.span(), Span::new(4, 2));
        assert_eq!(WidgetSize::ExtraLarge.span(), Span::new(8, 4));
    }
}
