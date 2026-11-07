//! The one drawing of a menu's rows, for both hosts: what a row holds (its
//! tick, icon, label, shortcut, and chevron or trailing glyph), how wide it
//! is, and its colours. The host draws the row's box around it and its panel
//! around them all; each host has its [`Look`].

use amane::{Center, Color, Image, Parent, Rectangle, Row, Text, Widget};

use super::model::{Check, Entry, Icon, MenuItem};
use crate::color::{over, with_alpha};
use crate::divider::Divider;
use crate::icons::{ICON_CHECK, ICON_RECORD};
use crate::layout::{boxed, slot, split_row, text_width};
use crate::look::style::{self, px};
use crate::material::{self, Level};
use crate::tokens::{Styled, menu, sys};

/// Which host draws the menu, which sets its sizes and colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// An app window's menu (a dropdown's, a right click's): its own panel,
    /// the chosen row filled with the tint.
    Popup,
    /// The shell's: a glass card, the keyboard's row in the accent.
    Layer,
}

/// How a row is shown now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowState {
    /// Under the pointer or the keyboard.
    pub focused: bool,
    /// Reached with the keyboard.
    pub keyboard: bool,
    /// A dropdown's chosen value.
    pub active: bool,
}

/// A row's fill, label colour, glyph colour and shortcut colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    /// The row's fill.
    pub fill: Color,
    /// Its label's colour.
    pub label: Color,
    /// Its tick's, icon's and trailing glyph's.
    pub glyph: Color,
    /// Its shortcut's.
    pub shortcut: Color,
}

/// What the rows of `entries` have room for: a tick column, an icon column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Columns {
    /// Some row has a tick.
    pub check: bool,
    /// Some row has an icon.
    pub icon: bool,
}

impl Columns {
    /// The columns `items` need.
    pub fn of<'a>(items: impl IntoIterator<Item = &'a MenuItem>) -> Self {
        items
            .into_iter()
            .fold(Self::default(), |columns, item| Self {
                check: columns.check || item.check.is_some(),
                icon: columns.icon || item.icon.is_some(),
            })
    }

    /// The columns the rows among `entries` need.
    pub fn of_entries(entries: &[Entry]) -> Self {
        Self::of(entries.iter().filter_map(|entry| match entry {
            Entry::Item(item) => Some(item),
            _ => None,
        }))
    }
}

impl Look {
    /// A row's height.
    pub fn row_height(self) -> f32 {
        match self {
            Look::Popup => style::menu_item_height(),
            Look::Layer => menu::row::height(),
        }
    }

    /// Room between a row's edge and what's in it.
    pub fn row_padding(self) -> f32 {
        match self {
            Look::Popup => px(12.0),
            Look::Layer => sys::space::inset::l(),
        }
    }

    /// Room between a row's parts.
    fn gap(self) -> f32 {
        match self {
            Look::Popup => px(8.0),
            Look::Layer => sys::space::gap::m(),
        }
    }

    /// How wide a tick's or an icon's column is.
    fn slot(self) -> f32 {
        match self {
            Look::Popup => px(15.0),
            Look::Layer => menu::icon_slot::width(),
        }
    }

    fn text(self) -> crate::tokens::TextStyle {
        sys::r#type::label()
    }

    /// A line's height: a row's, a header's or a separator's.
    pub fn entry_height(self, entry: &Entry) -> f32 {
        match entry {
            Entry::Item(_) | Entry::Header(_) => self.row_height(),
            Entry::Separator => Divider::height(),
        }
    }

    /// A row's colours in `state`.
    pub fn colors(self, item: &MenuItem, state: RowState) -> Colors {
        match self {
            Look::Popup => {
                let (label, glyph) = if !item.enabled {
                    (style::label_secondary(), style::label_secondary())
                } else if state.active {
                    (style::glyph_on_tint(), style::glyph_on_tint())
                } else if item.destructive {
                    (
                        sys::color::status::critical::text(),
                        sys::color::status::critical::text(),
                    )
                } else {
                    (style::label_primary(), style::label_secondary())
                };
                let base = if state.active {
                    style::tint()
                } else {
                    Color::TRANSPARENT
                };
                let fill = if state.keyboard {
                    over(style::hover_of(label), base)
                } else {
                    base
                };
                Colors {
                    fill,
                    label,
                    glyph,
                    shortcut: style::label_secondary(),
                }
            }
            Look::Layer => {
                let focused = state.focused && item.enabled;
                let keyboard = focused && state.keyboard;
                let fill = if !focused {
                    Color::TRANSPARENT
                } else if keyboard && !item.destructive {
                    sys::color::accent::bg()
                } else if item.destructive {
                    sys::color::status::critical::hover()
                } else {
                    sys::color::state::hover()
                };
                let label = if !item.enabled {
                    material::label(Level::Secondary)
                } else if keyboard && !item.destructive {
                    sys::color::accent::fg()
                } else if item.destructive {
                    sys::color::status::critical::text()
                } else {
                    material::label(Level::Primary)
                };
                let shortcut = if keyboard && !item.destructive {
                    with_alpha(sys::color::accent::fg(), menu::shortcut::hover_opacity())
                } else if focused {
                    with_alpha(
                        material::label(Level::Primary),
                        menu::shortcut::hover_opacity(),
                    )
                } else {
                    material::label(Level::Secondary)
                };
                Colors {
                    fill,
                    label,
                    glyph: label,
                    shortcut,
                }
            }
        }
    }

    /// What's in a row: its tick and icon columns (as `columns` has them),
    /// its label, and at its end the shortcut and the chevron or trailing
    /// glyph.
    pub fn content(self, item: &MenuItem, columns: Columns, colors: Colors) -> Row {
        let mut leading: Vec<Box<dyn Widget>> = Vec::new();
        if columns.check {
            leading.push(self.check_slot(item.check, colors.glyph));
        }
        if columns.icon {
            leading.push(self.icon_slot(item.icon.as_ref(), colors.glyph));
        }
        leading.push(boxed(
            Text::new(item.label.clone())
                .style(self.text())
                .color(colors.label)
                .elide(),
        ));
        let leading = Row::new(leading)
            .gap(self.gap())
            .align(Center)
            .height(Parent);

        let mut trailing: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(keys) = &item.shortcut {
            trailing.push(boxed(
                Text::new(keys.clone())
                    .style(self.text())
                    .color(colors.shortcut),
            ));
        }
        if item.has_submenu() {
            trailing.push(boxed(
                slot(menu::chevron_slot::width()).child(
                    Text::new("\u{203A}")
                        .style(self.text())
                        .color(colors.label)
                        .tight(),
                ),
            ));
        } else if let Some(glyph) = item.trailing {
            trailing.push(boxed(crate::layout::icon(glyph, self.slot(), colors.glyph)));
        }
        split_row(leading, trailing).height(Parent)
    }

    /// A header's line: its title, small and grey.
    pub fn header(self, title: &str) -> Rectangle {
        Rectangle::new()
            .width(Parent)
            .height(self.row_height())
            .padding(crate::layout::pad(self.row_padding(), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Text::new(title.to_string())
                    .style(self.text())
                    .color(material::label(Level::Secondary))
                    .elide(),
            )
    }

    fn check_slot(self, check: Option<Check>, color: Color) -> Box<dyn Widget> {
        let side = self.slot();
        let glyph = match check {
            Some(Check::On) => Some(ICON_CHECK),
            Some(Check::Radio(true)) => Some(ICON_RECORD),
            _ => None,
        };
        match glyph {
            Some(glyph) => {
                let size = if check == Some(Check::On) {
                    side
                } else {
                    side * 0.5
                };
                boxed(slot(side).child(crate::layout::icon(glyph, size, color)))
            }
            None => boxed(slot(side)),
        }
    }

    fn icon_slot(self, icon: Option<&Icon>, color: Color) -> Box<dyn Widget> {
        let side = self.slot();
        match icon {
            Some(Icon::Glyph { ch, font }) => {
                let size = match self {
                    Look::Popup => side,
                    Look::Layer => sys::r#type::label().size,
                };
                let text = Text::new(ch.clone()).size(size).color(color).tight();
                let text = match font {
                    Some(font) => text.font(font),
                    None => text.font(sys::r#type::label().family),
                };
                boxed(slot(side).child(text))
            }
            Some(Icon::Image(path)) => boxed(slot(side).fill(Image::contain(path.clone()))),
            None => boxed(slot(side)),
        }
    }

    /// How wide `item`'s row is, with `columns`.
    pub fn row_width(self, item: &MenuItem, columns: Columns) -> f32 {
        let column = self.slot() + self.gap();
        let mut width = text_width(&item.label, self.text()) + 2.0 * self.row_padding();
        if columns.check {
            width += column;
        }
        if columns.icon {
            width += column;
        }
        if let Some(keys) = &item.shortcut {
            width += text_width(keys, self.text()) + sys::space::gap::xl();
        }
        if item.has_submenu() {
            width += menu::chevron_slot::width() + sys::space::gap::s();
        } else if item.trailing.is_some() {
            width += column;
        }
        width
    }

    /// How wide a header's line is.
    pub fn header_width(self, title: &str) -> f32 {
        text_width(title, self.text()) + 2.0 * self.row_padding()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_column_makes_every_row_wider_alike() {
        let plain = MenuItem::new("Name");
        let none = Columns::of([&plain]);
        let ticked = Columns::of([&plain, &MenuItem::new("Size").checked(true)]);
        assert!(!none.check && ticked.check);
        for look in [Look::Popup, Look::Layer] {
            assert!(look.row_width(&plain, ticked) > look.row_width(&plain, none));
        }
    }

    #[test]
    fn a_destructive_row_is_red_and_a_disabled_one_grey() {
        let state = RowState {
            focused: true,
            ..RowState::default()
        };
        let red = Look::Layer.colors(&MenuItem::new("Delete").destructive(), state);
        assert_eq!(red.label, sys::color::status::critical::text());
        let grey = Look::Layer.colors(&MenuItem::new("Off").disabled(), state);
        assert_eq!(grey.fill, Color::TRANSPARENT);
    }
}
