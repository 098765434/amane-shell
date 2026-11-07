//! `dock.json`: the dock's size, magnification, pins and where it is.

use super::*;

/// How much the dock's icons grow under the pointer.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Magnify {
    /// Icons stay their size.
    Off,
    /// A little.
    Small,
    #[default]
    /// Somewhat.
    Medium,
    /// A lot.
    Large,
}

impl Magnify {
    /// Every choice, smallest first.
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
    /// How much an icon grows under the pointer.
    pub magnification: Magnify,

    /// Whether it slides away until the pointer meets the screen's edge.
    pub auto_hide: bool,

    /// Whether it rises with niri's overview.
    pub rise_with_overview: bool,

    /// Whether a dot marks the apps with a window open.
    pub running_dots: bool,

    /// Whether an app's name shows over its icon on hover.
    pub tooltips: bool,

    /// Whether an app's icon bounces while it starts.
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

settings_file!(Dock, crate::app_ids::SHELL, "dock", dock, change_dock);
