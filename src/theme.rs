//! The runtime inputs the token system may vary on top of Penpot's values
//! (manual §2): light/dark mode and density select theme sets, the system
//! accent replaces the accent primitives, and text scale multiplies font
//! sizes. Env overrides are read once; the system values are read per
//! frame through amane's `Appearance` service, so a change redraws.

use std::sync::OnceLock;

use amane::{Appearance, Color, Scheme, Service};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Density {
    Comfortable,
    Compact,
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

/// `AMANE_THEME=light|dark`, else the system preference, else dark.
pub fn mode() -> Mode {
    static OVERRIDE: OnceLock<Option<Mode>> = OnceLock::new();
    let forced = OVERRIDE.get_or_init(|| {
        let var = std::env::var("AMANE_THEME").ok();
        Mode::ALL
            .into_iter()
            .find(|m| var.as_deref() == Some(m.theme()))
    });

    forced.unwrap_or_else(|| match Appearance::read().scheme() {
        Scheme::Light => Mode::Light,
        Scheme::Dark | Scheme::NoPreference => Mode::Dark,
    })
}

/// The accent the system publishes, if any (many portals publish none).
pub fn system_accent() -> Option<Color> {
    Appearance::read().accent()
}

/// `AMANE_TEXT_SCALE`, else 1.
pub fn text_scale() -> f32 {
    static SCALE: OnceLock<f32> = OnceLock::new();
    *SCALE.get_or_init(|| parse_text_scale(std::env::var("AMANE_TEXT_SCALE").ok().as_deref()))
}

/// A text scale from its variable: a finite number, kept within 0.5–3.
fn parse_text_scale(var: Option<&str>) -> f32 {
    var.and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|s| s.is_finite())
        .map_or(1.0, |s| s.clamp(0.5, 3.0))
}

/// `AMANE_DENSITY=compact|comfortable`, else comfortable.
pub fn density() -> Density {
    static DENSITY: OnceLock<Density> = OnceLock::new();
    *DENSITY.get_or_init(|| {
        let var = std::env::var("AMANE_DENSITY").ok();
        Density::ALL
            .into_iter()
            .find(|d| var.as_deref() == Some(d.theme()))
            .unwrap_or(Density::Comfortable)
    })
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
