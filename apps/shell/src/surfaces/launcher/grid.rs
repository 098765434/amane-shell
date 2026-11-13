//! The grid of apps: a cell an app, its icon over its name, one highlight
//! gliding under the icons to the app picked, and a scrollbar while the
//! grid moves.
//!
//! Each category is a page of its own. At rest only the category showing
//! is drawn; while two fingers turn the page sideways, the one it's
//! turning to is drawn beside it, both sliding under the fingers. All
//! Applications leads with the apps used most, a hairline under them.

use amane::{
    Blend, Center, Column, DesktopApp, Padding, Rectangle, Row, Stack, Start, Text, Widget,
};
use emaki_kit::stagger::Stagger;

use crate::tokens::{Styled, sys};
use emaki_kit::app_icon;
use emaki_kit::kinetic::Axis;
use emaki_kit::layout::{boxed, frame, text_height, text_width};
use emaki_kit::scroll_view::ScrollView;

use super::choreo;
use super::results::{self, Empty};
use super::{
    BODY, CELL, COLS, GRID_W, ICON, ICON_BOX, Layout, View, apps_on, hover, on_app_click,
    on_body_scroll,
};
use emaki_kit::material::{self, Level};

// TODO(token): `launcher.highlight.radius` once Penpot has it (manual §1.1).
/// The highlight's corners.
pub(super) const HIGHLIGHT_RADIUS: f32 = 14.0;

/// Where this frame draws things: the body scrolled by `scroll`, the
/// highlight at `x`, `y` on item `selected` and as strong as `highlight`,
/// its rows rising in on `enter`, and the scrollbar at `scrollbar`;
/// `reveal` while Super shows the rows' fuller lines.
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) scroll: f32,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) highlight: f32,
    pub(super) enter: Stagger,
    pub(super) scrollbar: f32,
    pub(super) selected: usize,
    pub(super) reveal: bool,
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
            let suggested = page == 0 && view.suggested.len() == COLS;
            let body = if page == view.page {
                page_of(&view.shown, Some(at), suggested)
            } else {
                // the one turning in: at its top, not yet live
                let apps = apps_on(view.all, &view.buckets, page, &view.suggested);
                page_of(&apps, None, suggested)
            };
            boxed(body.translate((page as f32 - offset) * GRID_W, 0.0))
        })
        .collect();

    frame(GRID_W, BODY)
        .clip()
        // the strip takes the scrolls, not a page: a page sliding out from
        // under the fingers mustn't take the gesture with it
        .on_scroll(on_body_scroll)
        .align_child(Start, Start)
        .child(Stack::new(layers))
}

/// One category's page: its grid, or a note when it's empty, a hairline
/// under its first row when that's the `suggested` apps. Without `at` it's
/// one turning in, drawn at its top with no highlight, scrollbar or hover.
fn page_of(apps: &[&DesktopApp], at: Option<At>, suggested: bool) -> Rectangle {
    if apps.is_empty() {
        return results::empty(Empty::Category, GRID_W, BODY);
    }
    let live = at.is_some();
    let at = at.unwrap_or(At {
        scroll: 0.0,
        x: 0.0,
        y: 0.0,
        highlight: 0.0,
        enter: Stagger::new(),
        scrollbar: 0.0,
        selected: 0,
        reveal: false,
    });

    let highlight = Rectangle::new()
        .width(ICON_BOX)
        .height(ICON_BOX)
        .radius(HIGHLIGHT_RADIUS)
        .fill(sys::color::state::selected())
        .opacity(at.highlight * choreo::entered(at.enter, 1))
        .translate(at.x, at.y);

    // each row of apps rises in after the one over it (the pill bar first),
    // its icons swelling to their size; rows out of view arrive with the last
    let first = (at.scroll / CELL).max(0.0) as usize;
    let lines: Vec<Box<dyn Widget>> = apps
        .chunks(COLS)
        .enumerate()
        .map(|(row, line)| {
            let entered = choreo::entered(at.enter, 1 + row.saturating_sub(first));
            let cells = line
                .iter()
                .enumerate()
                .map(|(col, app)| boxed(rise(cell(app, row * COLS + col, live), entered)))
                .collect();
            boxed(Row::new(cells))
        })
        .collect();

    let mut layers = vec![boxed(highlight), boxed(Column::new(lines))];
    if suggested {
        // in the room over the second row's icons
        layers.push(boxed(
            Rectangle::new()
                .width(GRID_W)
                .height(sys::border::hairline())
                .fill(material::separator())
                .translate(0.0, CELL),
        ));
    }
    ScrollView::new(Axis::Vertical, GRID_W, BODY)
        .content(Layout::Grid.content(apps.len()))
        .offset(at.scroll)
        .scrollbar(at.scrollbar)
        .build(Stack::new(layers))
}

/// `cell` rising in `entered` far (0 … 1), swelling to its size.
fn rise(cell: Rectangle, entered: f32) -> Rectangle {
    if entered >= 1.0 {
        return cell;
    }
    let from = emaki_kit::motion::scale(choreo::ENTER_SCALE);
    cell.opacity(entered.clamp(0.0, 1.0))
        .scale(f32::blend(from, 1.0, entered))
        .translate(0.0, choreo::rise(entered))
}

/// One app: its icon in the highlight's box, its name under it on one
/// line. Hovering highlights it; a click launches it, a right-click opens
/// its menu.
fn cell(app: &DesktopApp, index: usize, live: bool) -> Rectangle {
    let gap = sys::space::gap::s();
    let icon = frame(ICON_BOX, ICON_BOX)
        .align_child(Center, Center)
        .child(app_icon::entry_tile(app, ICON));
    let style = sys::r#type::label();
    // elided text fills its box, so the box is only as wide as the name:
    // the column centres it
    let name = frame(
        text_width(app.name(), style).min(CELL - gap * 2.0),
        text_height(style),
    )
    .align_child(Center, Center)
    .clip()
    .child(
        Text::new(app.name())
            .style(style)
            .color(material::label(Level::Primary))
            .elide(),
    );

    let cell = frame(CELL, CELL)
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
