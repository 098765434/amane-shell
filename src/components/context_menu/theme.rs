use amane::Color;

use super::tokens::{ThemeTokens, tokens};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub mode: Mode,
    pub surface: Color,
    pub border: Color,
    pub fg: Color,
    pub muted: Color,
    pub hover_fill: Color,
    pub hover_destructive_fill: Color,
    pub selected_fill: Color,
    pub on_selected_fg: Color,
    pub accent: Color,
    pub separator: Color,
    pub destructive: Color,
}

impl Theme {
    pub fn light() -> Self {
        Self::from_mode(Mode::Light)
    }

    pub fn dark() -> Self {
        Self::from_mode(Mode::Dark)
    }

    pub fn auto() -> Self {
        let mode = match std::env::var("AMANE_THEME").ok().as_deref() {
            Some("light") => Mode::Light,
            Some("dark") => Mode::Dark,
            _ => Mode::Dark,
        };

        Self::from_mode(mode)
    }

    pub fn with_accent(mut self, accent: Color) -> Self {
        self.accent = accent;
        self.selected_fill = accent;
        self
    }

    fn from_mode(mode: Mode) -> Self {
        let t = tokens();
        let picked: ThemeTokens = match mode {
            Mode::Light => t.light,
            Mode::Dark => t.dark,
        };

        Self {
            mode,
            surface: picked.surface,
            border: picked.border,
            fg: picked.fg,
            muted: picked.muted,
            hover_fill: picked.hover_fill,
            hover_destructive_fill: picked.hover_destructive_fill,
            selected_fill: picked.selected_fill,
            on_selected_fg: picked.on_selected_fg,
            accent: picked.accent,
            separator: picked.separator,
            destructive: picked.destructive,
        }
    }
}
