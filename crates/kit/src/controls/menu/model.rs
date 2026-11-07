//! What a menu holds: rows of [`MenuItem`], separators and headers, each
//! row with an optional icon, checkmark, shortcut and submenu. App windows'
//! menus and the shell's both take these.

use std::path::PathBuf;
use std::rc::Rc;

/// A row's leading picture: a glyph (in a given font, or the label's), or an
/// image file.
#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    /// A glyph, in `font` or else the label's.
    Glyph {
        /// The glyph.
        ch: String,
        /// Its font, as an icon font.
        font: Option<&'static str>,
    },
    /// An image file.
    Image(PathBuf),
}

impl Icon {
    /// A glyph in the label's font.
    pub fn glyph(ch: impl Into<String>) -> Self {
        Self::Glyph {
            ch: ch.into(),
            font: None,
        }
    }

    /// A glyph in `font`, as an icon font.
    pub fn glyph_with_font(ch: impl Into<String>, font: &'static str) -> Self {
        Self::Glyph {
            ch: ch.into(),
            font: Some(font),
        }
    }

    /// An image file, drawn to fit.
    pub fn image(path: impl Into<PathBuf>) -> Self {
        Self::Image(path.into())
    }

    /// The glyph, when it's a single-character one.
    pub fn glyph_char(&self) -> Option<char> {
        match self {
            Self::Glyph { ch, .. } => {
                let mut chars = ch.chars();
                let first = chars.next()?;
                chars.next().is_none().then_some(first)
            }
            Self::Image(_) => None,
        }
    }
}

/// A Material Symbols glyph, as the kit's controls use.
impl From<char> for Icon {
    fn from(ch: char) -> Self {
        Self::glyph_with_font(ch.to_string(), crate::tokens::sys::r#type::icon())
    }
}

/// A row that's on or off: a checkbox's tick, or one of a group of choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Ticked.
    On,
    /// Not ticked; the row keeps the tick's room so the labels line up.
    Off,
    /// One of a group of which one is chosen, chosen or not.
    Radio(bool),
}

impl Check {
    /// A tick that's on when `on` is.
    pub fn of(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }

    /// Whether the row is the chosen one.
    pub fn on(self) -> bool {
        matches!(self, Self::On | Self::Radio(true))
    }
}

/// One row of a menu.
#[derive(Clone)]
pub struct MenuItem {
    /// What the row says.
    pub label: String,
    /// A picture before the label.
    pub icon: Option<Icon>,
    /// A glyph at the row's end, as a hint of what it opens.
    pub trailing: Option<char>,
    /// The keys that do the same, shown at the row's end.
    pub shortcut: Option<String>,
    /// A tick, for a row that's on or off.
    pub check: Option<Check>,
    /// A row that deletes or ends something, drawn in red.
    pub destructive: bool,
    /// Whether it can be picked.
    pub enabled: bool,
    pub(crate) on_select: Option<Rc<dyn Fn()>>,
    pub(crate) submenu: Option<Vec<Entry>>,
}

impl std::fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuItem")
            .field("label", &self.label)
            .field("icon", &self.icon)
            .field("check", &self.check)
            .field("submenu", &self.submenu.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

/// Rows are the same when they look the same; what they do isn't compared.
impl PartialEq for MenuItem {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
            && self.icon == other.icon
            && self.trailing == other.trailing
            && self.shortcut == other.shortcut
            && self.check == other.check
            && self.destructive == other.destructive
            && self.enabled == other.enabled
            && self.submenu.is_some() == other.submenu.is_some()
    }
}

impl MenuItem {
    /// A row that says `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            trailing: None,
            shortcut: None,
            check: None,
            destructive: false,
            enabled: true,
            on_select: None,
            submenu: None,
        }
    }

    /// [`MenuItem::new`].
    pub fn label(label: impl Into<String>) -> Self {
        Self::new(label)
    }

    /// Puts `icon` before the label.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Puts `glyph` at the row's end.
    pub fn trailing(mut self, glyph: char) -> Self {
        self.trailing = Some(glyph);
        self
    }

    /// Shows `keys` at the row's end.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// Ticks the row, or leaves the tick's room.
    pub fn check(mut self, check: Check) -> Self {
        self.check = Some(check);
        self
    }

    /// Ticked when `on`.
    pub fn checked(self, on: bool) -> Self {
        self.check(Check::of(on))
    }

    /// Draws it in red: it deletes or ends something.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Shows it greyed, and it can't be picked.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// What picking it does.
    pub fn on_select(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Opens `menu` beside it, instead of doing something itself.
    pub fn submenu(mut self, menu: ContextMenu) -> Self {
        self.submenu = Some(menu.entries);
        self
    }

    /// Whether it opens a submenu.
    pub fn has_submenu(&self) -> bool {
        self.submenu.is_some()
    }
}

impl<S: Into<String>> From<S> for MenuItem {
    fn from(label: S) -> Self {
        Self::new(label)
    }
}

/// One line of a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// A row.
    Item(MenuItem),
    /// A line between groups of rows.
    Separator,
    /// A small title over the rows after it, which can't be picked.
    Header(String),
}

impl Entry {
    /// Whether it's a row that can be picked.
    pub fn selectable(&self) -> bool {
        matches!(self, Entry::Item(item) if item.enabled)
    }
}

/// A menu, built a line at a time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextMenu {
    pub(crate) entries: Vec<Entry>,
}

impl ContextMenu {
    /// A menu with nothing in it yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `item` as a row.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(Entry::Item(item));
        self
    }

    /// Adds a line between groups of rows.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// Adds a small title over the rows after it.
    pub fn header(mut self, title: impl Into<String>) -> Self {
        self.entries.push(Entry::Header(title.into()));
        self
    }

    /// Its lines.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Whether it has no lines.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checked_row_and_a_radio_say_whether_theyre_on() {
        assert!(Check::of(true).on());
        assert!(!Check::Off.on());
        assert!(Check::Radio(true).on());
        assert!(!Check::Radio(false).on());
    }

    #[test]
    fn a_single_glyph_is_a_char() {
        assert_eq!(Icon::from('\u{e5ca}').glyph_char(), Some('\u{e5ca}'));
        assert_eq!(Icon::glyph("ab").glyph_char(), None);
    }

    #[test]
    fn headers_and_disabled_rows_cant_be_picked() {
        let menu = ContextMenu::new()
            .header("Apps")
            .item(MenuItem::new("A"))
            .item(MenuItem::new("B").disabled())
            .separator();
        let picks: Vec<bool> = menu.entries().iter().map(Entry::selectable).collect();
        assert_eq!(picks, [false, true, false, false]);
    }
}
