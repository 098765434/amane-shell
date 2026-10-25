//! The list a search shows in the grid's place: a row an entry, its icon
//! (the app's, or a glyph on a raised tile), its title and a line under
//! it, with one highlight gliding to the row picked. It scrolls as the
//! grid does ([`scrolled_by`]).

use amane::{Center, Color, Column, Rectangle, Row, Stack, Start, Text, Widget};

use crate::components::app_icon;
use crate::components::layout::{boxed, icon, pad, slot};
use crate::tokens::{Styled, sys};

use super::entries::{Entry, Icon, run_action};
use super::grid::{At, scrolled_by};
use super::{BODY, ROW, ROW_GAP, WIDTH, hover, on_scroll};

// TODO(token): `launcher.result.icon` once Penpot has it (manual §1.1).
/// How big a row's icon is.
const RESULT_ICON: f32 = 40.0;

pub(super) fn list(entries: &[Entry], at: At) -> Rectangle {
    let content = (entries.len() as f32 * (ROW + ROW_GAP)).max(BODY);
    let highlight = Rectangle::new()
        .width(WIDTH)
        .height(ROW)
        .radius(sys::radius::popover())
        .fill(sys::color::state::selected())
        .translate(0.0, at.y);
    let rows: Vec<Box<dyn Widget>> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| boxed(row(entry, index)))
        .collect();

    Rectangle::new()
        .width(WIDTH)
        .height(BODY)
        .fill(Color::TRANSPARENT)
        .clip()
        .padding(scrolled_by(at.scroll))
        .on_scroll(on_scroll)
        .align_child(Start, Start)
        .opacity(at.fade)
        .child(
            Rectangle::new()
                .width(WIDTH)
                .height(content)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Start)
                .child(Stack::new(vec![
                    boxed(highlight),
                    boxed(Column::new(rows).gap(ROW_GAP)),
                ])),
        )
}

fn row(entry: &Entry, index: usize) -> Rectangle {
    let action = entry.action.clone();
    let leading: Box<dyn Widget> = match &entry.icon {
        Icon::App(app) => boxed(app_icon::entry_tile(app, RESULT_ICON)),
        Icon::Glyph(glyph) => boxed(
            slot(RESULT_ICON)
                .radius(sys::radius::raised())
                .fill(sys::color::surface::raised::bg())
                .child(icon(
                    *glyph,
                    RESULT_ICON / 2.0,
                    sys::color::surface::panel::fg(),
                )),
        ),
    };
    let title = Text::new(entry.title.clone())
        .style(sys::r#type::title())
        .color(sys::color::surface::panel::fg())
        .elide();
    let text: Box<dyn Widget> = match &entry.subtitle {
        Some(subtitle) => boxed(
            Column::new(vec![
                boxed(title),
                boxed(
                    Text::new(subtitle.clone())
                        .style(sys::r#type::caption())
                        .color(sys::color::content::secondary())
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
        .on_click(move |_| run_action(&action))
        .child(
            Row::new(vec![leading, text])
                .gap(sys::space::gap::xl())
                .align(Center),
        )
}

/// What shows where nothing does: the apps not read yet, or a pill with
/// nothing under it.
pub(super) fn empty(message: &str) -> Rectangle {
    Rectangle::new()
        .width(WIDTH)
        .height(BODY)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(
            Text::new(message)
                .style(sys::r#type::label())
                .color(sys::color::content::secondary()),
        )
}
