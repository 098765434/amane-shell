//! A one-line text field's text: a caret and a selection, kept in
//! characters, edited the way a one-line text field is. A surface that
//! draws its search text itself rather than focusing a `TextInput` (so
//! every key reaches its `on_key`) keeps one of these and hands it the keys
//! ([`Field::handle`]); the launcher and Quick Settings' Customize window
//! do. Copying and pasting go through wl-clipboard ([`copy`], [`paste`]).

use amane::{Key, Modifiers};

use crate::services::{run, run_now};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Field {
    text: String,
    // where the caret is, in characters
    caret: usize,
    // the selection's other end, in characters; none without a selection
    anchor: Option<usize>,
}

impl Field {
    pub(crate) const fn new() -> Self {
        Self {
            text: String::new(),
            caret: 0,
            anchor: None,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn caret(&self) -> usize {
        self.caret
    }

    pub(crate) fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// The selection as an ordered range, empty at the caret without one.
    pub(crate) fn selection(&self) -> (usize, usize) {
        match self.anchor {
            Some(anchor) => (anchor.min(self.caret), anchor.max(self.caret)),
            None => (self.caret, self.caret),
        }
    }

    /// Characters `from` up to `to`.
    pub(crate) fn slice(&self, from: usize, to: usize) -> String {
        self.text.chars().skip(from).take(to - from).collect()
    }

    pub(crate) fn selected(&self) -> String {
        let (from, to) = self.selection();
        self.slice(from, to)
    }

    pub(crate) fn select_all(&mut self) {
        self.anchor = Some(0);
        self.caret = self.len();
        self.settle();
    }

    /// Moves the caret to `to`, extending the selection with Shift held,
    /// else dropping it.
    pub(crate) fn move_to(&mut self, to: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
        self.caret = to.min(self.len());
        self.settle();
    }

    /// Left: a step back, or without Shift to the selection's start.
    pub(crate) fn left(&mut self, extend: bool) {
        match (extend, self.anchor) {
            (false, Some(_)) => self.move_to(self.selection().0, false),
            _ => self.move_to(self.caret.saturating_sub(1), extend),
        }
    }

    /// Right: a step on, or without Shift to the selection's end.
    pub(crate) fn right(&mut self, extend: bool) {
        match (extend, self.anchor) {
            (false, Some(_)) => self.move_to(self.selection().1, false),
            _ => self.move_to(self.caret + 1, extend),
        }
    }

    /// Types `text` at the caret, over the selection. Line breaks and other
    /// control characters are left out: the field is one line.
    pub(crate) fn insert(&mut self, text: &str) {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return;
        }
        self.delete_selection();
        let at = self.byte(self.caret);
        self.text.insert_str(at, &clean);
        self.caret += clean.chars().count();
    }

    /// Removes the selection, if there is one; whether it did.
    pub(crate) fn delete_selection(&mut self) -> bool {
        let (from, to) = self.selection();
        self.anchor = None;
        if from == to {
            return false;
        }
        let range = self.byte(from)..self.byte(to);
        self.text.replace_range(range, "");
        self.caret = from;
        true
    }

    /// Backspace: the selection, else the character before the caret;
    /// whether anything went.
    pub(crate) fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.caret == 0 {
            return false;
        }
        let at = self.byte(self.caret - 1);
        self.text.remove(at);
        self.caret -= 1;
        true
    }

    /// Edits for `key` with `held`, as a one-line field does: characters
    /// and Space type, Backspace deletes, Left/Right/Home/End move the
    /// caret (Shift extends the selection), and Ctrl+A/C/X/V select all,
    /// copy, cut and paste. Other chords (Alt, Super) are swallowed, not
    /// typed. Whether the key was one of the field's at all.
    pub(crate) fn handle(&mut self, key: Key, held: Modifiers) -> bool {
        if held.ctrl
            && let Key::Character(letter) = key
        {
            match letter.to_ascii_lowercase() {
                'a' => self.select_all(),
                'c' => copy(&self.selected()),
                'x' => {
                    copy(&self.selected());
                    self.delete_selection();
                }
                'v' => {
                    if let Some(text) = paste() {
                        self.insert(&text);
                    }
                }
                _ => return false,
            }
            return true;
        }
        if held.command() {
            return false;
        }
        match key {
            Key::Character(letter) => self.insert(&letter.to_string()),
            Key::Space => self.insert(" "),
            Key::Backspace => {
                self.backspace();
            }
            Key::Left => self.left(held.shift),
            Key::Right => self.right(held.shift),
            Key::Home => self.move_to(0, held.shift),
            Key::End => self.move_to(usize::MAX, held.shift),
            _ => return false,
        }
        true
    }

    // an empty selection is none
    fn settle(&mut self) {
        if self.anchor == Some(self.caret) {
            self.anchor = None;
        }
    }

    // the byte offset of character `at`
    fn byte(&self, at: usize) -> usize {
        self.text
            .char_indices()
            .nth(at)
            .map_or(self.text.len(), |(i, _)| i)
    }
}

/// Puts `text` on the clipboard (`wl-copy`).
pub(crate) fn copy(text: &str) {
    if !text.is_empty() {
        run("wl-copy", vec![String::from("--"), text.to_string()]);
    }
}

/// The clipboard's text (`wl-paste`). Blocks for as long as it takes.
pub(crate) fn paste() -> Option<String> {
    run_now("wl-paste", &[String::from("--no-newline")])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Field {
        let mut field = Field::default();
        field.insert(text);
        field
    }

    #[test]
    fn typing_goes_in_at_the_caret() {
        let mut field = typed("fox");
        field.move_to(0, false);
        field.insert("fire");
        assert_eq!(field.text(), "firefox");
        assert_eq!(field.caret(), 4);
    }

    #[test]
    fn backspace_takes_whole_characters() {
        let mut field = typed("café");
        assert!(field.backspace());
        assert_eq!(field.text(), "caf");
        field.move_to(0, false);
        assert!(!field.backspace());
    }

    #[test]
    fn shift_selects_and_typing_replaces_it() {
        let mut field = typed("hello world");
        field.move_to(5, false);
        field.left(true);
        field.left(true);
        assert_eq!(field.selected(), "lo");
        field.insert("p");
        assert_eq!(field.text(), "help world");
    }

    #[test]
    fn an_arrow_without_shift_collapses_to_the_selections_end() {
        let mut field = typed("abcdef");
        field.move_to(1, false);
        field.move_to(4, true);
        field.left(false);
        assert_eq!((field.caret(), field.selected()), (1, String::new()));
        field.move_to(4, true);
        field.right(false);
        assert_eq!(field.caret(), 4);
    }

    #[test]
    fn select_all_then_backspace_clears() {
        let mut field = typed("query");
        field.select_all();
        assert_eq!(field.selected(), "query");
        assert!(field.backspace());
        assert_eq!(field, Field::default());
    }

    #[test]
    fn pasted_line_breaks_are_dropped() {
        let field = typed("one\ntwo");
        assert_eq!(field.text(), "onetwo");
    }

    #[test]
    fn the_keys_edit_as_a_field_does() {
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        let mut field = Field::new();
        assert!(field.handle(Key::Character('h'), none));
        assert!(field.handle(Key::Character('i'), none));
        assert!(field.handle(Key::Space, none));
        assert_eq!(field.text(), "hi ");
        assert!(field.handle(Key::Backspace, none));
        assert!(field.handle(Key::Home, none));
        assert!(field.handle(Key::Right, shift));
        assert_eq!(field.selected(), "h");
        assert!(field.handle(Key::End, none));
        assert_eq!(field.caret(), 2);
        // not the field's: Enter, Escape, a Super chord
        assert!(!field.handle(Key::Enter, none));
        assert!(!field.handle(Key::Escape, none));
        let logo = Modifiers {
            logo: true,
            ..Modifiers::default()
        };
        assert!(!field.handle(Key::Character('q'), logo));
        assert_eq!(field.text(), "hi");
    }

    #[test]
    fn selecting_back_to_the_caret_is_no_selection() {
        let mut field = typed("ab");
        field.left(true);
        field.right(true);
        assert_eq!(field.selection(), (2, 2));
        assert!(!field.delete_selection());
    }
}
