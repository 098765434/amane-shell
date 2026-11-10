//! `appearance.json`: light or dark, the accent, the glass, text size and motion.

use super::*;

/// The accents to pick from, the system colours in their dark
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

    /// Whether dark shows black surfaces (Settings' Black, for OLED panels):
    /// with `mode` dark it's always Black; with none, Black whenever the
    /// system is dark.
    pub black: bool,

    /// What the wallpaper does under Black (niri's `SetWallpaperDim`).
    pub black_wallpaper: BlackWallpaper,

    /// The accent as `#rrggbb`, or none for the system's (else the tokens').
    pub accent: Option<String>,

    /// How tightly the shell is laid out.
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
            black: false,
            black_wallpaper: BlackWallpaper::Dimmed,
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

/// The wallpaper under Black: an OLED leaves black pixels off, so the less of
/// it that's lit, the less power it takes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum BlackWallpaper {
    /// As it is.
    Shown,
    /// Darkened to a quarter.
    #[default]
    Dimmed,
    /// Not drawn: black.
    Hidden,
}

impl BlackWallpaper {
    /// Every choice, in Settings' order.
    pub const ALL: [BlackWallpaper; 3] = [
        BlackWallpaper::Dimmed,
        BlackWallpaper::Hidden,
        BlackWallpaper::Shown,
    ];

    /// How much niri dims the wallpaper, 0–100.
    pub fn percent(self) -> u8 {
        match self {
            BlackWallpaper::Shown => 0,
            BlackWallpaper::Dimmed => 75,
            BlackWallpaper::Hidden => 100,
        }
    }

    /// The name Settings shows.
    pub fn label(self) -> &'static str {
        match self {
            BlackWallpaper::Shown => "Shown",
            BlackWallpaper::Dimmed => "Dimmed",
            BlackWallpaper::Hidden => "Hidden",
        }
    }
}

settings_file!(
    Appearance,
    crate::app_ids::SHELL,
    "appearance",
    appearance,
    change_appearance
);

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
