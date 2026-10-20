use amane::Color;

use super::tokens::{ThemeTokens, tokens};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub surface: Color,
    pub fg: Color,
    pub fg_strong: Color,
    pub pill: Color,
    pub pill_focused: Color,
    pub accent: Color,
    pub urgent: Color,
}

impl Theme {
    pub fn auto() -> Self {
        let mode = match std::env::var("AMANE_THEME").ok().as_deref() {
            Some("light") => Mode::Light,
            Some("dark") => Mode::Dark,
            _ => Mode::Dark,
        };

        Self::from_mode(mode)
    }

    fn from_mode(mode: Mode) -> Self {
        let t = tokens();
        let picked: ThemeTokens = match mode {
            Mode::Light => t.light,
            Mode::Dark => t.dark,
        };

        Self {
            surface: picked.surface,
            fg: picked.fg,
            fg_strong: picked.fg_strong,
            pill: picked.pill,
            pill_focused: picked.pill_focused,
            accent: picked.accent,
            urgent: picked.urgent,
        }
    }
}
