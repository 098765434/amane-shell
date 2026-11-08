//! The sheet things are added from while the shell is being arranged: a
//! sidebar (a search field over the sections), a hairline, and a pane of
//! what can be added over a footer (a hint, Undo Remove, Reset and Done).
//! Quick Settings' Customize window and the widget gallery are each one,
//! with a pane of their own.

use amane::{Center, Rectangle, Row, Scroll, Start, Widget};

use crate::tokens::sys;
use emaki_kit::area::Area;
use emaki_kit::buttons::{PushButton, Shape, Style};
use emaki_kit::divider::Divider;
use emaki_kit::kinetic::Axis;
use emaki_kit::layout::{boxed, caption, frame, pad, split_row};
use emaki_kit::list::{self, List, ListItem};
use emaki_kit::scroll_view::ScrollView;
use emaki_kit::search_field::SearchField;
use emaki_kit::text_edit::Field;

/// A sheet's measures and the prefix its controls' ids share.
pub struct Sheet {
    /// What the ids start with: `emaki.qs.customize`.
    pub id: &'static str,
    /// The sidebar's width.
    pub sidebar: f32,
    /// The room between the sheet's edge and what's on it.
    pub inset: f32,
}

/// One of the sidebar's rows.
pub struct Section {
    /// What its id ends with.
    pub key: &'static str,
    /// What it says.
    pub title: &'static str,
    /// The glyph on its tile.
    pub glyph: char,
    /// Whether it's the one showing.
    pub picked: bool,
    /// What a click on it does.
    pub pick: Box<dyn Fn()>,
}

/// The sidebar's search field.
pub struct Search<'a> {
    /// What's typed.
    pub field: &'a Field,
    /// What it says while empty.
    pub placeholder: &'static str,
    /// What clearing it does.
    pub clear: fn(),
}

/// How the sidebar's rows scroll, when they might not fit.
pub struct Scrolling {
    /// Given how far the rows can scroll, how far they have.
    pub offset: Box<dyn FnOnce(f32) -> f32>,
    /// What the wheel over them does.
    pub on_scroll: fn(Scroll),
}

/// The footer's words and what its buttons do.
pub struct Footer {
    /// What it says at the start.
    pub hint: &'static str,
    /// The reset button's name: "Reset Layout", "Remove All".
    pub reset: &'static str,
    /// Undo Remove, while there's a remove to undo.
    pub undo: Option<fn()>,
    /// What the reset button does.
    pub on_reset: fn(),
    /// What Done does.
    pub on_done: fn(),
}

impl Sheet {
    /// How tall the footer is.
    pub fn footer_height() -> f32 {
        sys::size::control::height() + sys::space::gap::m() * 2.0
    }

    /// The whole sheet over `area`: `sidebar` at the start, and the pane
    /// `pane` draws in the room it's given (where that is on the surface)
    /// over the footer.
    pub fn build(
        &self,
        area: &Area,
        sidebar: Rectangle,
        pane: impl FnOnce(Area) -> Rectangle,
        footer: Footer,
    ) -> Rectangle {
        let pane_x = self.sidebar + Divider::width();
        let pane_w = (area.width - pane_x).max(1.0);
        let footer_h = Self::footer_height();
        let pane_h = (area.height - footer_h - Divider::height()).max(1.0);
        let pane = pane(Area::new(area.x + pane_x, area.y, pane_w, pane_h));
        let right = amane::Column::new(vec![
            boxed(pane),
            boxed(Divider::build()),
            boxed(self.footer(pane_w, footer_h, footer)),
        ])
        .width(pane_w);

        frame(area.width, area.height)
            .align_child(Start, Start)
            .child(
                Row::new(vec![
                    boxed(sidebar),
                    boxed(Divider::vertical(area.height)),
                    boxed(right),
                ])
                .align(Start),
            )
    }

    /// The search field over the section rows, `height` tall, the rows
    /// scrolling if `scrolling` says how.
    pub fn sidebar(
        &self,
        search: Search,
        sections: Vec<Section>,
        height: f32,
        scrolling: Option<Scrolling>,
    ) -> Rectangle {
        let inner_w = self.sidebar - self.inset * 2.0;
        let gap = sys::space::gap::m();
        let search = SearchField::new(format!("{}.search", self.id), search.field, true)
            .placeholder(search.placeholder)
            .width(inner_w)
            .on_clear(search.clear)
            .build();

        let rows = sections.len() as f32;
        let items: Vec<ListItem> = sections
            .into_iter()
            .map(|section| {
                let pick = section.pick;
                ListItem::new(
                    format!("{}.section.{}", self.id, section.key),
                    section.title,
                )
                .leading_tile(section.glyph)
                .bare(true)
                .selected(section.picked)
                .on_press(pick)
            })
            .collect();
        let list = list::with_row_width(inner_w, || List::new(items).build());
        let list: Box<dyn Widget> = match scrolling {
            Some(scrolling) => {
                let rows_h = rows * sys::size::row::height() + (rows - 1.0) * sys::space::gap::xs();
                let list_h = (height - self.inset * 2.0 - SearchField::height() - gap).max(1.0);
                let offset = (scrolling.offset)((rows_h - list_h).max(0.0));
                boxed(
                    ScrollView::new(Axis::Vertical, inner_w, list_h)
                        .content(rows_h)
                        .offset(offset)
                        .on_scroll(scrolling.on_scroll)
                        .build(list),
                )
            }
            None => boxed(list),
        };

        frame(self.sidebar, height)
            .padding(pad(self.inset, self.inset))
            .align_child(Start, Start)
            .child(
                amane::Column::new(vec![boxed(search), list])
                    .gap(gap)
                    .width(inner_w),
            )
    }

    fn footer(&self, width: f32, height: f32, footer: Footer) -> Rectangle {
        let button = |name: &str, label: &str, style: Style, press: fn()| {
            let button = PushButton::new(format!("{}.{name}", self.id), label)
                .style(style)
                .shape(Shape::Round)
                .on_press(press);
            let button = match style {
                Style::Filled => button.text(sys::r#type::label()),
                _ => button,
            };
            boxed(button.build())
        };
        let mut actions: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(undo) = footer.undo {
            actions.push(button("undo", "Undo Remove", Style::Plain, undo));
        }
        actions.push(button("reset", footer.reset, Style::Plain, footer.on_reset));
        actions.push(button("done", "Done", Style::Filled, footer.on_done));
        frame(width, height)
            .padding(pad(self.inset, 0.0))
            .align_child(Start, Center)
            .child(split_row(caption(footer.hint), actions))
    }
}
