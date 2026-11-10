//! The runtime inputs the token system may vary on top of Penpot's values
//! (manual §2): light/dark mode and density select theme sets, the system
//! accent replaces the accent primitives, and text scale multiplies font
//! sizes. The glass level, high contrast and reduced motion change how
//! glass is tinted and how things move (`glass_region`, `motion`). Env overrides are read once; the system values are read per
//! frame through amane's `Appearance` service, so a change redraws.

use std::sync::OnceLock;

use amane::serde::{Deserialize, Serialize};
use amane::{Appearance, Color, Scheme, Service};

use crate::settings;

/// Light or dark: which of Penpot's `mode` themes is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Mode {
    /// Dark text on light surfaces.
    Light,
    /// Light text on dark surfaces.
    Dark,
}

/// How tightly things are spaced: which of Penpot's `density` themes is active.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Density {
    /// The default spacing.
    Comfortable,
    /// Tighter spacing and smaller sizes.
    Compact,
}

/// How see-through the glass is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Glass {
    /// Clearer than the tokens: more of what's behind shows through.
    Clear,
    /// The tokens' tints.
    Tinted,
    /// Opaque, without the glass material: Reduce Transparency.
    Solid,
}

impl Glass {
    /// Every level, clearest first.
    pub const ALL: [Glass; 3] = [Glass::Clear, Glass::Tinted, Glass::Solid];

    /// The name in settings and `EMAKI_GLASS`.
    pub fn name(self) -> &'static str {
        match self {
            Glass::Clear => "clear",
            Glass::Tinted => "tinted",
            Glass::Solid => "solid",
        }
    }
}

impl Mode {
    /// Both modes, light first.
    pub const ALL: [Mode; 2] = [Mode::Light, Mode::Dark];

    /// The theme name in Penpot's `mode` group.
    pub fn theme(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }
}

/// The surfaces under dark mode: as the tokens draw them, or true black
/// for OLED panels, whose black pixels are off. Which of Penpot's `surface`
/// themes is active; black only ever applies over dark.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Surface {
    /// The dark theme's own greys.
    #[default]
    Standard,
    /// `#000` windows, groups and materials, dimmer white, no glass.
    Black,
}

impl Surface {
    /// Both surfaces, the standard first.
    pub const ALL: [Surface; 2] = [Surface::Standard, Surface::Black];

    /// The theme name in Penpot's `surface` group.
    pub fn theme(self) -> &'static str {
        match self {
            Surface::Standard => "standard",
            Surface::Black => "black",
        }
    }
}

impl Density {
    /// Both densities, the default first.
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    /// The theme name in Penpot's `density` group.
    pub fn theme(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }
}

/// `EMAKI_THEME`, read once: `light`, `dark` or `black` (dark with black
/// surfaces).
fn forced_theme() -> Option<(Mode, Surface)> {
    static OVERRIDE: OnceLock<Option<(Mode, Surface)>> = OnceLock::new();
    *OVERRIDE.get_or_init(|| match std::env::var("EMAKI_THEME").ok()?.as_str() {
        "light" => Some((Mode::Light, Surface::Standard)),
        "dark" => Some((Mode::Dark, Surface::Standard)),
        "black" => Some((Mode::Dark, Surface::Black)),
        _ => None,
    })
}

/// `EMAKI_THEME=light|dark|black`, else the settings file's, else the
/// system preference, else dark.
pub fn mode() -> Mode {
    if let Some((forced, _)) = forced_theme() {
        return forced;
    }

    if let Some(chosen) = settings::appearance().mode {
        return chosen;
    }

    match Appearance::read().scheme() {
        Scheme::Light => Mode::Light,
        Scheme::Dark | Scheme::NoPreference => Mode::Dark,
    }
}

/// `EMAKI_THEME=black`, else the settings file's Black, but only while the
/// mode is dark: light mode's surfaces stay as they are.
pub fn surface() -> Surface {
    if mode() != Mode::Dark {
        return Surface::Standard;
    }
    if let Some((_, forced)) = forced_theme() {
        return forced;
    }
    if settings::appearance().black {
        Surface::Black
    } else {
        Surface::Standard
    }
}

/// The accent the settings file picks, else the one the system publishes,
/// if any (many portals publish none).
pub fn accent() -> Option<Color> {
    settings::appearance()
        .accent_color()
        .or_else(|| Appearance::read().accent())
}

/// `EMAKI_GLASS=clear|tinted|solid`, else the settings file's; always solid
/// under black surfaces, where glass would only light pixels black leaves off
/// and blur would keep the compositor busy.
pub fn glass() -> Glass {
    if surface() == Surface::Black {
        return Glass::Solid;
    }
    static OVERRIDE: OnceLock<Option<Glass>> = OnceLock::new();
    let forced = OVERRIDE.get_or_init(|| {
        let var = std::env::var("EMAKI_GLASS").ok();
        Glass::ALL
            .into_iter()
            .find(|g| var.as_deref() == Some(g.name()))
    });

    forced.unwrap_or_else(|| settings::appearance().glass)
}

/// `EMAKI_HIGH_CONTRAST=1`, else whether the system asks for higher
/// contrast.
pub fn high_contrast() -> bool {
    static OVERRIDE: OnceLock<bool> = OnceLock::new();
    *OVERRIDE.get_or_init(|| std::env::var("EMAKI_HIGH_CONTRAST").as_deref() == Ok("1"))
        || Appearance::read().high_contrast()
}

/// `EMAKI_ADAPTIVE_GLASS=1`, else the settings file's: whether small glass
/// turns light or dark with its backdrop.
pub fn adaptive_glass() -> bool {
    static OVERRIDE: OnceLock<bool> = OnceLock::new();
    *OVERRIDE.get_or_init(|| std::env::var("EMAKI_ADAPTIVE_GLASS").as_deref() == Ok("1"))
        || settings::appearance().adaptive_glass
}

/// `EMAKI_VIBRANCY=1`, else the settings file's: whether labels on glass
/// blend with the backdrop under them (niri's vibrancy).
pub fn vibrancy() -> bool {
    static OVERRIDE: OnceLock<bool> = OnceLock::new();
    *OVERRIDE.get_or_init(|| std::env::var("EMAKI_VIBRANCY").as_deref() == Ok("1"))
        || settings::appearance().vibrancy
}

/// Whether to reduce motion: the settings file's choice, else the
/// system's.
pub fn reduced_motion() -> bool {
    settings::appearance()
        .reduce_motion
        .unwrap_or_else(|| Appearance::read().reduced_motion())
}

/// `EMAKI_TEXT_SCALE`, else the settings file's, else 1.
pub fn text_scale() -> f32 {
    static SCALE: OnceLock<Option<f32>> = OnceLock::new();

    let forced = SCALE.get_or_init(|| {
        std::env::var("EMAKI_TEXT_SCALE")
            .ok()
            .map(|var| parse_text_scale(Some(&var)))
    });

    forced.unwrap_or_else(|| {
        let scale = settings::appearance().text_scale;

        if scale.is_finite() {
            scale.clamp(0.5, 3.0)
        } else {
            1.0
        }
    })
}

/// A text scale from its variable: a finite number, kept within 0.5–3.
fn parse_text_scale(var: Option<&str>) -> f32 {
    var.and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite())
        .map_or(1.0, |s| s.clamp(0.5, 3.0))
}

/// `EMAKI_DENSITY=compact|comfortable`, else the settings file's.
pub fn density() -> Density {
    static DENSITY: OnceLock<Option<Density>> = OnceLock::new();

    let forced = DENSITY.get_or_init(|| {
        let var = std::env::var("EMAKI_DENSITY").ok();
        Density::ALL
            .into_iter()
            .find(|d| var.as_deref() == Some(d.theme()))
    });

    forced.unwrap_or_else(|| settings::appearance().density)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_scale_is_a_sane_number() {
        assert_eq!(parse_text_scale(None), 1.0);
        assert_eq!(parse_text_scale(Some(" 1.25 ")), 1.25);
        assert_eq!(parse_text_scale(Some("10")), 3.0);
        assert_eq!(parse_text_scale(Some("0.1")), 0.5);
        assert_eq!(parse_text_scale(Some("NaN")), 1.0);
        assert_eq!(parse_text_scale(Some("big")), 1.0);
    }
}
