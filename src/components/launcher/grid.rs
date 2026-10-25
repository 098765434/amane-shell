//! The grid of apps: a cell an app, its icon over its name, one highlight
//! gliding under the icons to the app picked, and a scrollbar while there
//! are more rows than fit.
//!
//! The grid scrolls by a negative top padding inside a clipped viewport,
//! as amane's `ScrollArea` does: the cells really move, so the clip keeps
//! just the ones in view and their hover and click areas stay where they
//! are drawn.

use amane::{
    Center, Color, Column, DesktopApp, Padding, Rectangle, Row, Stack, Start, Text, Widget,
};

use crate::components::app_icon;
use crate::components::layout::{boxed, text_height, text_width};
use crate::components::panel::close;
use crate::tokens::{Styled, sys};

use super::{BODY, CELL, COLS, ICON, ICON_BOX, WIDTH, hover, on_scroll};

// TODO(token): `launcher.highlight.radius` and `launcher.scrollbar.*` once
// Penpot has them (manual §1.1).
/// The highlight's corners.
pub(super) const HIGHLIGHT_RADIUS: f32 = 14.0;
/// How wide the scrollbar is, and how short its thumb gets.
const SCROLLBAR: f32 = 6.0;
const THUMB_MIN: f32 = 32.0;

/// Where this frame draws things: the body scrolled by `scroll`, the
/// highlight at `x`, `y`, the whole body at `fade`.
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) scroll: f32,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) fade: f32,
}

/// Content scrolled up by `scroll`, as geometry.
pub(super) fn scrolled_by(scroll: f32) -> Padding {
    Padding {
        top: -scroll,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    }
}

pub(super) fn grid(apps: &[&DesktopApp], at: At) -> Rectangle {
    let rows = apps.len().div_ceil(COLS).max(1);
    let content = rows as f32 * CELL;

    let highlight = Rectangle::new()
        .width(ICON_BOX)
        .height(ICON_BOX)
        .radius(HIGHLIGHT_RADIUS)
        .fill(sys::color::state::selected())
        .translate(at.x, at.y);

    let lines: Vec<Box<dyn Widget>> = apps
        .chunks(COLS)
        .enumerate()
        .map(|(row, line)| {
            let cells = line
                .iter()
                .enumerate()
                .map(|(col, app)| boxed(cell(app, row * COLS + col)))
                .collect();
            boxed(Row::new(cells))
        })
        .collect();

    let viewport = Rectangle::new()
        .width(WIDTH)
        .height(BODY)
        .fill(Color::TRANSPARENT)
        .clip()
        .padding(scrolled_by(at.scroll))
        .on_scroll(on_scroll)
        .align_child(Start, Start)
        .child(
            Rectangle::new()
                .width(WIDTH)
                .height(content)
                .fill(Color::TRANSPARENT)
                .align_child(Start, Start)
                .child(Stack::new(vec![
                    boxed(highlight),
                    boxed(Column::new(lines)),
                ])),
        );

    let max = content - BODY;
    if max <= 0.0 {
        return viewport.opacity(at.fade);
    }
    let thumb_height = (BODY * BODY / content).max(THUMB_MIN);
    let thumb = Rectangle::new()
        .width(SCROLLBAR)
        .height(thumb_height)
        .radius(SCROLLBAR / 2.0)
        .fill(sys::color::content::secondary())
        .translate(WIDTH - SCROLLBAR, at.scroll / max * (BODY - thumb_height));
    Rectangle::new()
        .width(WIDTH)
        .height(BODY)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .opacity(at.fade)
        .child(Stack::new(vec![boxed(viewport), boxed(thumb)]))
}

/// One app: its icon in the highlight's box, its name under it on one
/// line. Hovering highlights it; a click launches it.
fn cell(app: &DesktopApp, index: usize) -> Rectangle {
    let launched = app.clone();
    let gap = sys::space::gap::s();
    let icon = Rectangle::new()
        .width(ICON_BOX)
        .height(ICON_BOX)
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .child(app_icon::entry_tile(app, ICON));
    let style = sys::r#type::label();
    // elided text fills its box, so the box is only as wide as the name:
    // the column centres it
    let name = Rectangle::new()
        .width(text_width(app.name(), style).min(CELL - gap * 2.0))
        .height(text_height(style))
        .fill(Color::TRANSPARENT)
        .align_child(Center, Center)
        .clip()
        .child(
            Text::new(app.name())
                .style(style)
                .color(sys::color::surface::panel::fg())
                .elide(),
        );

    Rectangle::new()
        .width(CELL)
        .height(CELL)
        .fill(Color::TRANSPARENT)
        .padding(Padding {
            top: super::BOX_Y,
            right: gap,
            bottom: gap,
            left: gap,
        })
        .align_child(Center, Start)
        .on_hover(hover(index))
        .on_click(move |_| {
            close();
            launched.launch();
        })
        .child(
            Column::new(vec![boxed(icon), boxed(name)])
                .gap(gap)
                .align(Center),
        )
}
