//! The grid of apps: a cell an app, its icon over its name, one highlight
//! gliding under the icons to the app picked, and a scrollbar while the
//! grid moves.
//!
//! Each category is a page of its own. At rest only the category showing
//! is drawn; while two fingers turn the page sideways, the one it's
//! turning to is drawn beside it, both sliding under the fingers.

use amane::{
    Center, Color, Column, DesktopApp, Padding, Rectangle, Row, Stack, Start, Text, Widget,
};

use crate::components::app_icon;
use crate::components::kinetic::Axis;
use crate::components::layout::{boxed, text_height, text_width};
use crate::components::scroll_view::ScrollView;
use crate::tokens::{Styled, sys};

use super::results::{self, Empty};
use super::{
    BODY, CELL, COLS, ICON, ICON_BOX, Layout, View, WIDTH, bucket, hover, on_app_click,
    on_body_scroll, under,
};

// TODO(token): `launcher.highlight.radius` once Penpot has it (manual §1.1).
/// The highlight's corners.
pub(super) const HIGHLIGHT_RADIUS: f32 = 14.0;

/// Where this frame draws things: the body scrolled by `scroll`, the
/// highlight at `x`, `y` and as strong as `highlight`, the whole body at
/// `fade`, and the scrollbar at `scrollbar`.
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) scroll: f32,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) highlight: f32,
    pub(super) fade: f32,
    pub(super) scrollbar: f32,
}

/// The category showing and, while turning, the one beside it, `offset`
/// the page they've turned to.
pub(super) fn pages(view: &View, offset: f32, at: At) -> Rectangle {
    let last = view.buckets.len().saturating_sub(1);
    let below = (offset.floor().max(0.0) as usize).min(last);
    let above = (offset.ceil().max(0.0) as usize).min(last);
    let mut shown = vec![below];
    if above != below {
        shown.push(above);
    }

    let layers: Vec<Box<dyn Widget>> = shown
        .into_iter()
        .map(|page| {
            let body = if page == view.page {
                page_of(&view.shown, Some(at))
            } else {
                // the one turning in: at its top, not yet live
                page_of(&under(view.all, bucket(&view.buckets, page)), None)
            };
            boxed(body.translate((page as f32 - offset) * WIDTH, 0.0))
        })
        .collect();

    Rectangle::new()
        .width(WIDTH)
        .height(BODY)
        .fill(Color::TRANSPARENT)
        .clip()
        // the strip takes the scrolls, not a page: a page sliding out from
        // under the fingers mustn't take the gesture with it
        .on_scroll(on_body_scroll)
        .align_child(Start, Start)
        .opacity(at.fade)
        .child(Stack::new(layers))
}

/// One category's page: its grid, or a note when it's empty. Without `at`
/// it's one turning in, drawn at its top with no highlight, scrollbar or
/// hover.
fn page_of(apps: &[&DesktopApp], at: Option<At>) -> Rectangle {
    if apps.is_empty() {
        return results::empty(Empty::Category);
    }
    let live = at.is_some();
    let at = at.unwrap_or(At {
        scroll: 0.0,
        x: 0.0,
        y: 0.0,
        highlight: 0.0,
        fade: 1.0,
        scrollbar: 0.0,
    });

    let highlight = Rectangle::new()
        .width(ICON_BOX)
        .height(ICON_BOX)
        .radius(HIGHLIGHT_RADIUS)
        .fill(sys::color::state::selected())
        .opacity(at.highlight)
        .translate(at.x, at.y);

    let lines: Vec<Box<dyn Widget>> = apps
        .chunks(COLS)
        .enumerate()
        .map(|(row, line)| {
            let cells = line
                .iter()
                .enumerate()
                .map(|(col, app)| boxed(cell(app, row * COLS + col, live)))
                .collect();
            boxed(Row::new(cells))
        })
        .collect();

    ScrollView::new(Axis::Vertical, WIDTH, BODY)
        .content(Layout::Grid.content(apps.len()))
        .offset(at.scroll)
        .scrollbar(at.scrollbar)
        .build(Stack::new(vec![
            boxed(highlight),
            boxed(Column::new(lines)),
        ]))
}

/// One app: its icon in the highlight's box, its name under it on one
/// line. Hovering highlights it; a click launches it, a right-click opens
/// its menu.
fn cell(app: &DesktopApp, index: usize, live: bool) -> Rectangle {
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

    let cell = Rectangle::new()
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
        .child(
            Column::new(vec![boxed(icon), boxed(name)])
                .gap(gap)
                .align(Center),
        );
    if live {
        cell.on_hover(hover(index))
            .on_click(on_app_click(app.clone()))
    } else {
        cell
    }
}
