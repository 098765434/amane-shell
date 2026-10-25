//! List — rows of the same kind of thing: networks, devices, outputs.
//!
//! Modelled on Material 3's lists. A [`ListItem`] has M3's slots — a leading
//! icon, the headline, and trailing items (a check, a lock, a percentage) —
//! and its states: selected, and a `sys.color.state.hover` layer while
//! hovered, when it can be clicked. A [`List`] stacks items under an
//! optional section header. Drawn in the shell's look: each item is a
//! rounded `surface.raised` row, filled `state.selected` when selected.
//!
//! Only one-line items for now; M3's supporting and overline text are the
//! next slots to add when a list needs them.

use std::cell::Cell;
use std::rc::Rc;

use amane::{Center, Color, Column, Parent, Rectangle, Row, Size, Start, Text, Widget};

use crate::buttons::{Actions, interactive, state_fill};
use crate::icons::ICON_CHECK;
use crate::layout::{boxed, caption, clipped_label, icon, pad, split_row};
use crate::tokens::{Styled, sys};

thread_local! {
    // how wide the rows being built are, when not a popover's width
    static ROW_WIDTH: Cell<Option<f32>> = const { Cell::new(None) };
}

/// Builds `f` with every row in it `width` wide (the headline is elided to
/// what's left after the trailing items), for a surface that isn't a
/// popover's width.
pub fn with_row_width<R>(width: f32, f: impl FnOnce() -> R) -> R {
    let previous = ROW_WIDTH.replace(Some(width));
    let out = f();
    ROW_WIDTH.set(previous);
    out
}

/// One row. `id` keys its hover state, so it must be unique and stable.
pub struct ListItem {
    id: String,
    headline: String,
    leading: Option<char>,
    // the leading glyph on an accent tile, as a sidebar names a section
    tile: bool,
    trailing: Vec<Box<dyn Widget>>,
    selected: bool,
    // no fill of its own: only the selected and hover fills
    bare: bool,
    color: Option<Color>,
    actions: Actions,
}

impl ListItem {
    pub fn new(id: impl Into<String>, headline: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            headline: headline.into(),
            leading: None,
            tile: false,
            trailing: Vec::new(),
            selected: false,
            bare: false,
            color: None,
            actions: Actions::default(),
        }
    }

    /// A Material Symbols glyph before the headline.
    pub fn leading(mut self, glyph: char) -> Self {
        self.leading = Some(glyph);
        self
    }

    /// A Material Symbols glyph on a small accent tile before the headline,
    /// as the rows of a sidebar name their sections.
    pub fn leading_tile(mut self, glyph: char) -> Self {
        self.leading = Some(glyph);
        self.tile = true;
        self
    }

    /// No fill until selected or hovered, for rows on a surface that reads
    /// as one piece (a sidebar) rather than a list of raised rows.
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }

    /// One more item at the trailing end, `gap.s` after the last.
    pub fn trailing(mut self, item: impl Widget + 'static) -> Self {
        self.trailing.push(boxed(item));
        self
    }

    /// The joined network, the default output, the connected device: filled,
    /// and its headline full-strength where the rest read secondary.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The headline and icon colour, in place of the one `selected` picks.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// What a left click does.
    pub fn on_press(mut self, press: impl Fn() + 'static) -> Self {
        self.actions.press = Some(Rc::new(press));
        self
    }

    /// What a right click does, such as opening a menu.
    pub fn on_secondary(mut self, secondary: impl Fn() + 'static) -> Self {
        self.actions.secondary = Some(Rc::new(secondary));
        self
    }

    pub fn build(self) -> Rectangle {
        let color = self.color.unwrap_or(if self.selected {
            sys::color::surface::panel::fg()
        } else {
            sys::color::content::secondary()
        });
        let row_w = ROW_WIDTH.get().unwrap_or_else(sys::size::popover::width);
        let headline_w = (row_w - sys::size::row::trailing_width()).max(1.0);

        let mut leading = Vec::new();
        if let Some(glyph) = self.leading {
            if self.tile {
                let side = sys::size::icon::m() + sys::space::gap::xs() * 2.0;
                leading.push(boxed(
                    Rectangle::new()
                        .width(Size::Fixed(side))
                        .height(Size::Fixed(side))
                        .radius(sys::radius::raised())
                        .fill(sys::color::accent::bg())
                        .align_child(Center, Center)
                        .child(icon(glyph, sys::size::icon::s(), sys::color::accent::fg())),
                ));
            } else {
                leading.push(boxed(icon(glyph, sys::size::icon::m(), color)));
            }
        }
        leading.push(boxed(clipped_label(
            self.headline,
            sys::r#type::label(),
            color,
            headline_w,
            Start,
        )));
        let content = split_row(
            Row::new(leading).gap(sys::space::gap::m()).align(Center),
            self.trailing,
        );

        let fill = if self.selected {
            sys::color::state::selected()
        } else if self.bare {
            Color::TRANSPARENT
        } else {
            sys::color::surface::raised::bg()
        };
        let clickable = self.actions.press.is_some() || self.actions.secondary.is_some();
        let fill = if clickable {
            state_fill(&self.id, fill, false)
        } else {
            fill
        };

        let row = Rectangle::new()
            .width(Parent)
            .height(Size::Fixed(sys::size::row::height()))
            .radius(sys::radius::raised())
            .fill(fill)
            .padding(pad(sys::space::inset::l(), 0.0))
            .align_child(Center, Center)
            .clip()
            .child(content);

        if clickable {
            interactive(row, self.id, false, self.actions)
        } else {
            row
        }
    }
}

/// The accent check a selected item ends with.
pub fn check() -> Text {
    icon(ICON_CHECK, sys::size::icon::m(), sys::color::accent::bg())
}

/// Trailing supporting text: a percentage, or "Connecting…".
pub fn trailing_text(text: impl Into<String>, color: Color) -> Text {
    Text::new(text.into())
        .style(sys::r#type::caption())
        .color(color)
}

/// Items `gap.xs` apart under an optional section header.
pub struct List {
    header: Option<String>,
    items: Vec<ListItem>,
    empty: Option<String>,
}

impl List {
    pub fn new(items: Vec<ListItem>) -> Self {
        Self {
            header: None,
            items,
            empty: None,
        }
    }

    /// The section's title, as a caption above the items.
    pub fn header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }

    /// A note shown in place of the items while there are none.
    pub fn empty(mut self, note: impl Into<String>) -> Self {
        self.empty = Some(note.into());
        self
    }

    pub fn build(self) -> Column {
        let mut rows: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(header) = self.header {
            rows.push(boxed(caption(header)));
        }
        if self.items.is_empty()
            && let Some(note) = self.empty
        {
            rows.push(boxed(caption(note)));
        }
        rows.extend(self.items.into_iter().map(|item| boxed(item.build())));

        Column::new(rows).gap(sys::space::gap::xs()).width(Parent)
    }
}
