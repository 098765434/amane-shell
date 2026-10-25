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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Density {
    Comfortable,
    Compact,
}

/// How see-through the glass is, as Apple's Liquid Glass setting has it.
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
    pub const ALL: [Glass; 3] = [Glass::Clear, Glass::Tinted, Glass::Solid];

    /// The name in settings and `AMANE_GLASS`.
    pub fn name(self) -> &'static str {
        match self {
            Glass::Clear => "clear",
            Glass::Tinted => "tinted",
            Glass::Solid => "solid",
        }
    }
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Light, Mode::Dark];

    /// The theme name in Penpot's `mode` group.
    pub fn theme(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }
}

impl Density {
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    /// The theme name in Penpot's `density` group.
    pub fn theme(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }
}

/// `AMANE_THEME=light|dark`, else the settings file's, else the system
/// preference, else dark.
pub fn mode() -> Mode {
    static OVERRIDE: OnceLock<Option<Mode>> = OnceLock::new();
    let forced = OVERRIDE.get_or_init(|| {
        let var = std::env::var("AMANE_THEME").ok();
        Mode::ALL
            .into_iter()
            .find(|m| var.as_deref() == Some(m.theme()))
    });

    if let Some(forced) = *forced {
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

/// The accent the settings file picks, else the one the system publishes,
/// if any (many portals publish none).
pub fn accent() -> Option<Color> {
    settings::appearance()
        .accent_color()
        .or_else(|| Appearance::read().accent())
}

/// `AMANE_GLASS=clear|tinted|solid`, else the settings file's.
pub fn glass() -> Glass {
    static OVERRIDE: OnceLock<Option<Glass>> = OnceLock::new();
    let forced = OVERRIDE.get_or_init(|| {
        let var = std::env::var("AMANE_GLASS").ok();
        Glass::ALL
            .into_iter()
            .find(|g| var.as_deref() == Some(g.name()))
    });

    forced.unwrap_or_else(|| settings::appearance().glass)
}

/// `AMANE_HIGH_CONTRAST=1`, else whether the system asks for higher
/// contrast.
pub fn high_contrast() -> bool {
    static OVERRIDE: OnceLock<bool> = OnceLock::new();
    *OVERRIDE.get_or_init(|| std::env::var("AMANE_HIGH_CONTRAST").as_deref() == Ok("1"))
        || Appearance::read().high_contrast()
}

/// `AMANE_ADAPTIVE_GLASS=1`, else the settings file's: whether small glass
/// turns light or dark with its backdrop.
pub fn adaptive_glass() -> bool {
    static OVERRIDE: OnceLock<bool> = OnceLock::new();
    *OVERRIDE.get_or_init(|| std::env::var("AMANE_ADAPTIVE_GLASS").as_deref() == Ok("1"))
        || settings::appearance().adaptive_glass
}

/// Whether to reduce motion: the settings file's choice, else the
/// system's.
pub fn reduced_motion() -> bool {
    settings::appearance()
        .reduce_motion
        .unwrap_or_else(|| Appearance::read().reduced_motion())
}

/// `AMANE_TEXT_SCALE`, else the settings file's, else 1.
pub fn text_scale() -> f32 {
    static SCALE: OnceLock<Option<f32>> = OnceLock::new();

    let forced = SCALE.get_or_init(|| {
        std::env::var("AMANE_TEXT_SCALE")
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

/// `AMANE_DENSITY=compact|comfortable`, else the settings file's.
pub fn density() -> Density {
    static DENSITY: OnceLock<Option<Density>> = OnceLock::new();

    let forced = DENSITY.get_or_init(|| {
        let var = std::env::var("AMANE_DENSITY").ok();
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
