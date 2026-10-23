use amane::Color;

use super::tokens::{ThemeTokens, parse_theme, theme_tokens};
use crate::theme::Density;
pub use crate::theme::Mode;
use crate::tokens::{Key, resolved};

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

    /// The colour roles for the current mode.
    pub fn auto() -> Self {
        Self::from_tokens(Key::current().mode, *theme_tokens())
    }

    pub fn with_accent(mut self, accent: Color) -> Self {
        self.accent = accent;
        self.selected_fill = accent;
        self
    }

    fn from_mode(mode: Mode) -> Self {
        let r = resolved(Key::new(mode, Density::Comfortable));
        let picked = parse_theme(&r.sys()).unwrap_or_else(|e| panic!("menu theme tokens: {e}"));
        Self::from_tokens(mode, picked)
    }

    fn from_tokens(mode: Mode, picked: ThemeTokens) -> Self {
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
