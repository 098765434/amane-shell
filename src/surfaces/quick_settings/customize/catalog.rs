//! The catalog the Customize window lists: every control under a category,
//! the categories a sidebar names (macOS's Customize Control Center names
//! them Battery, Connectivity, Display & Brightness, …), and what a
//! category, or a search, shows. Pure, so it's tested.

use crate::components::icons::{
    ICON_APPS, ICON_BATTERY_FULL, ICON_BRIGHTNESS_HIGH, ICON_DND, ICON_KEYBOARD, ICON_LOCK,
    ICON_MUSIC, ICON_POWER, ICON_SCREENSHOT, ICON_TUNE, ICON_VOLUME_UP, ICON_WIFI,
};

use super::super::controls::ControlId;

/// A row of the sidebar: every control, or one kind of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Category {
    AllControls,
    Battery,
    Connectivity,
    DisplayBrightness,
    Focus,
    Keyboards,
    LockScreen,
    NowPlaying,
    Power,
    Screenshot,
    Sounds,
    Utilities,
}

impl Category {
    /// The sidebar's order: All Controls, then the rest by name.
    pub(crate) const ALL: [Category; 12] = [
        Self::AllControls,
        Self::Battery,
        Self::Connectivity,
        Self::DisplayBrightness,
        Self::Focus,
        Self::Keyboards,
        Self::LockScreen,
        Self::NowPlaying,
        Self::Power,
        Self::Screenshot,
        Self::Sounds,
        Self::Utilities,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::AllControls => "All Controls",
            Self::Battery => "Battery",
            Self::Connectivity => "Connectivity",
            Self::DisplayBrightness => "Display & Brightness",
            Self::Focus => "Focus",
            Self::Keyboards => "Keyboards",
            Self::LockScreen => "Lock Screen",
            Self::NowPlaying => "Now Playing",
            Self::Power => "Power",
            Self::Screenshot => "Screenshot",
            Self::Sounds => "Sounds",
            Self::Utilities => "Utilities",
        }
    }

    /// The glyph on its sidebar tile.
    pub(crate) fn glyph(self) -> char {
        match self {
            Self::AllControls => ICON_APPS,
            Self::Battery => ICON_BATTERY_FULL,
            Self::Connectivity => ICON_WIFI,
            Self::DisplayBrightness => ICON_BRIGHTNESS_HIGH,
            Self::Focus => ICON_DND,
            Self::Keyboards => ICON_KEYBOARD,
            Self::LockScreen => ICON_LOCK,
            Self::NowPlaying => ICON_MUSIC,
            Self::Power => ICON_POWER,
            Self::Screenshot => ICON_SCREENSHOT,
            Self::Sounds => ICON_VOLUME_UP,
            Self::Utilities => ICON_TUNE,
        }
    }

    /// The key its row's hover is kept under.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::AllControls => "all",
            Self::Battery => "battery",
            Self::Connectivity => "connectivity",
            Self::DisplayBrightness => "display",
            Self::Focus => "focus",
            Self::Keyboards => "keyboards",
            Self::LockScreen => "lock",
            Self::NowPlaying => "now-playing",
            Self::Power => "power",
            Self::Screenshot => "screenshot",
            Self::Sounds => "sounds",
            Self::Utilities => "utilities",
        }
    }

    /// The controls under it, in Control Center's order.
    pub(crate) fn controls(self) -> Vec<ControlId> {
        ControlId::ALL
            .into_iter()
            .filter(|id| self == Self::AllControls || category_of(*id) == self)
            .collect()
    }
}

/// The category `id` is listed under.
pub(crate) fn category_of(id: ControlId) -> Category {
    match id {
        ControlId::Battery | ControlId::PowerMode => Category::Battery,
        ControlId::Wifi
        | ControlId::Bluetooth
        | ControlId::Airplane
        | ControlId::Vpn
        | ControlId::Wired
        | ControlId::Tailscale => Category::Connectivity,
        ControlId::Display | ControlId::NightLight | ControlId::DarkStyle => {
            Category::DisplayBrightness
        }
        ControlId::Focus => Category::Focus,
        ControlId::Keyboard => Category::Keyboards,
        ControlId::Lock => Category::LockScreen,
        ControlId::NowPlaying => Category::NowPlaying,
        ControlId::PowerOff => Category::Power,
        ControlId::Screenshot => Category::Screenshot,
        ControlId::Sound => Category::Sounds,
        ControlId::KeepAwake | ControlId::BackgroundApps | ControlId::SoftwareUpdate => {
            Category::Utilities
        }
    }
}

/// One titled group of controls in the window's pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) title: &'static str,
    pub(crate) ids: Vec<ControlId>,
}

/// What the pane shows: for a `query`, the controls whose name or category
/// has it, in one section; else for All Controls, **Suggestions** (the
/// controls `hidden` from the card, left out when there are none) and then
/// every category; else the one category.
pub(crate) fn sections(category: Category, query: &str, hidden: &[ControlId]) -> Vec<Section> {
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        let ids: Vec<ControlId> = ControlId::ALL
            .into_iter()
            .filter(|id| {
                id.title().to_lowercase().contains(&query)
                    || category_of(*id).title().to_lowercase().contains(&query)
            })
            .collect();
        return vec![Section {
            title: "Results",
            ids,
        }];
    }
    if category != Category::AllControls {
        return vec![Section {
            title: category.title(),
            ids: category.controls(),
        }];
    }
    let mut out = Vec::new();
    let suggested: Vec<ControlId> = ControlId::ALL
        .into_iter()
        .filter(|id| hidden.contains(id))
        .collect();
    if !suggested.is_empty() {
        out.push(Section {
            title: "Suggestions",
            ids: suggested,
        });
    }
    for category in Category::ALL.into_iter().skip(1) {
        let ids = category.controls();
        if !ids.is_empty() {
            out.push(Section {
                title: category.title(),
                ids,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_is_in_exactly_one_category() {
        let mut seen = Vec::new();
        for category in Category::ALL.into_iter().skip(1) {
            for id in category.controls() {
                assert_eq!(category_of(id), category);
                assert!(!seen.contains(&id), "{id:?} listed twice");
                seen.push(id);
            }
        }
        assert_eq!(seen.len(), ControlId::ALL.len());
        assert_eq!(Category::AllControls.controls().len(), ControlId::ALL.len());
        let mut keys: Vec<&str> = Category::ALL.iter().map(|c| c.key()).collect();
        keys.dedup();
        assert_eq!(keys.len(), Category::ALL.len());
    }

    #[test]
    fn all_controls_starts_with_what_is_hidden() {
        let hidden = [ControlId::Vpn, ControlId::KeepAwake];
        let shown = sections(Category::AllControls, "", &hidden);
        assert_eq!(shown[0].title, "Suggestions");
        // in Control Center's order, not the order given
        assert_eq!(shown[0].ids, vec![ControlId::Vpn, ControlId::KeepAwake]);
        assert_eq!(shown[1].title, "Battery");
        assert_eq!(shown.len(), Category::ALL.len());
        // with nothing hidden there are no suggestions
        let shown = sections(Category::AllControls, "", &[]);
        assert_eq!(shown[0].title, "Battery");
    }

    #[test]
    fn a_category_is_one_section() {
        let shown = sections(Category::Connectivity, "", &[ControlId::Vpn]);
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].title, "Connectivity");
        assert!(shown[0].ids.contains(&ControlId::Tailscale));
        assert!(!shown[0].ids.contains(&ControlId::Sound));
    }

    #[test]
    fn a_search_finds_by_name_or_category() {
        let shown = sections(Category::Sounds, " blue ", &[]);
        assert_eq!(shown[0].title, "Results");
        assert_eq!(shown[0].ids, vec![ControlId::Bluetooth]);
        // the category's name counts too, whatever row is picked
        let shown = sections(Category::Focus, "connect", &[]);
        assert_eq!(shown[0].ids, Category::Connectivity.controls());
        let shown = sections(Category::AllControls, "zzz", &[]);
        assert!(shown[0].ids.is_empty());
    }
}
