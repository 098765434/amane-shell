//! What the launcher is searching: everything, from the pill, or one kind
//! of thing, picked from the round buttons beside it, Ctrl+1 … 4 or IPC
//! (`launcher open files`).

use crate::components::icons::{ICON_APPS, ICON_CONTENT_PASTE, ICON_FOLDER, ICON_LAYERS};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    /// The pill: apps, answers, the session and the web.
    #[default]
    Home,
    Apps,
    Files,
    Actions,
    Clipboard,
}

impl Mode {
    /// The modes the round buttons pick, in their order (Ctrl+1 … 4).
    pub(super) const PICKED: [Self; 4] = [Self::Apps, Self::Files, Self::Actions, Self::Clipboard];

    pub(super) fn glyph(self) -> char {
        match self {
            Self::Home | Self::Apps => ICON_APPS,
            Self::Files => ICON_FOLDER,
            Self::Actions => ICON_LAYERS,
            Self::Clipboard => ICON_CONTENT_PASTE,
        }
    }

    /// What the search field says while it's empty.
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Home => "Spotlight Search",
            Self::Apps => "Applications",
            Self::Files => "Files",
            Self::Actions => "Actions",
            Self::Clipboard => "Clipboard",
        }
    }

    /// The digit that picks it with Ctrl.
    pub(super) fn digit(self) -> Option<char> {
        Self::PICKED
            .iter()
            .position(|mode| *mode == self)
            .and_then(|at| char::from_digit(at as u32 + 1, 10))
    }

    /// The mode Ctrl+`digit` picks.
    pub(super) fn from_digit(digit: char) -> Option<Self> {
        let at = digit.to_digit(10)?.checked_sub(1)?;
        Self::PICKED.get(at as usize).copied()
    }

    /// The mode IPC names, like "files".
    pub(super) fn named(name: &str) -> Option<Self> {
        match name {
            "home" | "search" => Some(Self::Home),
            "apps" | "applications" => Some(Self::Apps),
            "files" => Some(Self::Files),
            "actions" => Some(Self::Actions),
            "clipboard" => Some(Self::Clipboard),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digits_pick_the_buttons_in_order() {
        assert_eq!(Mode::from_digit('1'), Some(Mode::Apps));
        assert_eq!(Mode::from_digit('4'), Some(Mode::Clipboard));
        assert_eq!(Mode::from_digit('0'), None);
        assert_eq!(Mode::from_digit('5'), None);
        assert_eq!(Mode::Files.digit(), Some('2'));
        assert_eq!(Mode::Home.digit(), None);
    }

    #[test]
    fn ipc_names_the_modes() {
        assert_eq!(Mode::named("clipboard"), Some(Mode::Clipboard));
        assert_eq!(Mode::named("apps"), Some(Mode::Apps));
        assert_eq!(Mode::named("nope"), None);
    }
}
