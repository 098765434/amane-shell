use amane::Color;

use super::tokens::{ThemeTokens, parse_theme, theme_tokens};
pub use crate::theme::Mode;
use crate::theme::{self, Density};
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
    /// Built by [`Theme::auto`]: re-read at each render, so an open menu
    /// follows the system's mode and accent.
    follows_system: bool,
}

impl Theme {
    pub fn light() -> Self {
        Self::from_mode(Mode::Light)
    }

    pub fn dark() -> Self {
        Self::from_mode(Mode::Dark)
    }

    /// The colour roles for the current mode and accent.
    pub fn auto() -> Self {
        Self {
            follows_system: true,
            ..Self::from_tokens(theme::mode(), *theme_tokens())
        }
    }

    /// A fixed accent; the theme then no longer follows the system.
    pub fn with_accent(mut self, accent: Color) -> Self {
        self.accent = accent;
        self.selected_fill = accent;
        self.follows_system = false;
        self
    }

    /// This theme as it should draw now.
    pub fn current(self) -> Self {
        if self.follows_system {
            Self::auto()
        } else {
            self
        }
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
            follows_system: false,
        }
    }
}
