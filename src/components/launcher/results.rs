//! The list a search shows in the grid's place: a row an entry, its icon
//! (the app's, or a glyph on a raised tile), its title and a line under
//! it, with one highlight gliding to the row picked. It scrolls as the
//! grid does, in a [`ScrollView`].

use amane::{Button, Center, Color, Column, Rectangle, Row, Stack, Start, Text, Widget};

use crate::components::app_icon;
use crate::components::icons::{ICON_APPS, ICON_SEARCH};
use crate::components::kinetic::Axis;
use crate::components::layout::{boxed, icon, pad, slot};
use crate::components::scroll_view::ScrollView;
use crate::tokens::{Styled, sys};

use super::entries::{Action, Entry, Icon, run_action};
use super::grid::At;
use super::{BODY, Layout, ROW, ROW_GAP, WIDTH, app_menu, hover, on_body_scroll};
use crate::components::material::{self, FillLevel, Level};

// TODO(token): `launcher.result.icon` and `launcher.empty.icon` once Penpot
// has them (manual §1.1).
/// How big a row's icon is.
const RESULT_ICON: f32 = 40.0;
/// How big the glyph over an empty state's note is.
const EMPTY_ICON: f32 = 40.0;

pub(super) fn list(entries: &[Entry], at: At) -> Rectangle {
    let highlight = Rectangle::new()
        .width(WIDTH)
        .height(ROW)
        .radius(sys::radius::popover())
        .fill(sys::color::state::selected())
        .opacity(at.highlight)
        .translate(0.0, at.y);
    let rows: Vec<Box<dyn Widget>> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| boxed(row(entry, index)))
        .collect();

    ScrollView::new(Axis::Vertical, WIDTH, BODY)
        .content(Layout::List.content(entries.len()))
        .offset(at.scroll)
        .scrollbar(at.scrollbar)
        .on_scroll(on_body_scroll)
        .build(Stack::new(vec![
            boxed(highlight),
            boxed(Column::new(rows).gap(ROW_GAP)),
        ]))
        .opacity(at.fade)
}

fn row(entry: &Entry, index: usize) -> Rectangle {
    let action = entry.action.clone();
    let leading: Box<dyn Widget> = match &entry.icon {
        Icon::App(app) => boxed(app_icon::entry_tile(app, RESULT_ICON)),
        Icon::Glyph(glyph) => boxed(
            slot(RESULT_ICON)
                .radius(sys::radius::raised())
                .fill(material::fill_role(FillLevel::Secondary))
                .child(icon(
                    *glyph,
                    RESULT_ICON / 2.0,
                    material::label(Level::Primary),
                )),
        ),
    };
    let title = Text::new(entry.title.clone())
        .style(sys::r#type::title())
        .color(material::label(Level::Primary))
        .elide();
    let text: Box<dyn Widget> = match &entry.subtitle {
        Some(subtitle) => boxed(
            Column::new(vec![
                boxed(title),
                boxed(
                    Text::new(subtitle.clone())
                        .style(sys::r#type::caption())
                        .color(material::label(Level::Secondary))
                        .elide(),
                ),
            ])
            .gap(sys::space::gap::_2xs()),
        ),
        None => boxed(title),
    };

    Rectangle::new()
        .width(WIDTH)
        .height(ROW)
        .fill(Color::TRANSPARENT)
        .radius(sys::radius::popover())
        .padding(pad(sys::space::inset::l(), 0.0))
        .align_child(Start, Center)
        .on_hover(hover(index))
        .on_click(move |button| match (button, &action) {
            (Button::Left, _) => run_action(&action),
            (Button::Right, Action::Launch(app)) => app_menu::open(app),
            _ => {}
        })
        .child(
            Row::new(vec![leading, text])
                .gap(sys::space::gap::xl())
                .align(Center),
        )
}

/// Why nothing shows.
#[derive(Clone, Copy)]
pub(super) enum Empty {
    /// The apps haven't been read yet.
    Loading,
    /// The category has nothing under it.
    Category,
}

/// What shows where nothing does: a glyph over a note saying why.
pub(super) fn empty(why: Empty) -> Rectangle {
    let (glyph, note) = match why {
        Empty::Loading => (ICON_SEARCH, "Loading…"),
        Empty::Category => (ICON_APPS, "Nothing in this category"),
    };
    let secondary = material::label(Level::Secondary);
    Rectangle::new()
        .width(WIDTH)
        .height(BODY)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(
            Column::new(vec![
                boxed(icon(glyph, EMPTY_ICON, secondary)),
                boxed(Text::new(note).style(sys::r#type::label()).color(secondary)),
            ])
            .gap(sys::space::gap::m())
            .align(Center),
        )
}
