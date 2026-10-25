//! The rows over the grid: the search field, drawn by hand (the text, the
//! caret and the selection's wash), and the bar of category pills, which
//! scrolls sideways when they don't fit.

use amane::{Center, Color, Padding, Rectangle, Row, Start, Text, Widget};

use crate::components::icons::ICON_APPS;
use crate::components::layout::{boxed, icon, text_width};
use crate::tokens::{Styled, TextStyle, sys};

use super::edit::Field;
use super::{HEADER, PILLS, WIDTH, model, on_pill_scroll, pick, with};

// TODO(token): `launcher.search.size`, `launcher.search.caret` and
// `launcher.logo.size` once Penpot has them (manual §1.1).
/// How big the search text is.
const SEARCH_TEXT: f32 = 18.0;
/// The caret's width and height.
const CARET: (f32, f32) = (2.0, 22.0);
/// How big the launcher's mark beside the field is.
const LOGO: f32 = 24.0;

fn search_style() -> TextStyle {
    TextStyle {
        size: SEARCH_TEXT,
        ..sys::r#type::title()
    }
}

/// The launcher's mark and the search field beside it.
pub(super) fn search_row(field: &Field) -> Rectangle {
    let gap = sys::space::gap::xl();
    let logo = icon(ICON_APPS, LOGO, sys::color::surface::panel::fg());
    Rectangle::new()
        .width(WIDTH)
        .height(HEADER)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .child(
            Row::new(vec![
                boxed(logo),
                boxed(search_field(field, WIDTH - LOGO - gap)),
            ])
            .gap(gap)
            .align(Center),
        )
}

/// The query as runs of text with the caret between them and the
/// selection washed behind its run, or a placeholder while it's empty.
fn search_field(field: &Field, width: f32) -> Rectangle {
    let style = search_style();
    let fg = sys::color::surface::panel::fg();
    let run = |text: String| Text::new(text).style(style).color(fg);
    let caret = || {
        Rectangle::new()
            .width(CARET.0)
            .height(CARET.1)
            .radius(CARET.0 / 2.0)
            .fill(sys::color::accent::bg())
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if field.text().is_empty() {
        parts.push(boxed(caret()));
        parts.push(boxed(
            Text::new("Search")
                .style(style)
                .color(sys::color::content::secondary()),
        ));
    } else {
        let len = field.len();
        let caret_at = field.caret();
        let (from, to) = field.selection();
        if from > 0 {
            parts.push(boxed(run(field.slice(0, from))));
        }
        if caret_at <= from {
            parts.push(boxed(caret()));
        }
        if to > from {
            let selected = field.slice(from, to);
            parts.push(boxed(
                Rectangle::new()
                    .width(text_width(&selected, style) + sys::space::gap::_2xs())
                    .height(CARET.1)
                    .radius(sys::radius::raised() / 2.0)
                    .fill(sys::color::state::selected())
                    .align_child(Center, Center)
                    .child(run(selected)),
            ));
            if caret_at >= to {
                parts.push(boxed(caret()));
            }
        }
        if to < len {
            parts.push(boxed(run(field.slice(to, len))));
        }
    }

    Rectangle::new()
        .width(width)
        .height(HEADER)
        .fill(Color::TRANSPARENT)
        .clip()
        .align_child(Start, Center)
        .child(Row::new(parts).gap(sys::space::gap::_2xs()).align(Center))
}

/// The pills worth showing (`buckets`, indices into the categories), the
/// `active` one picked. The row scrolls by a negative left padding, which
/// moves its pills' hit areas with them, inside a clipped track.
pub(super) fn pill_bar(active: usize, buckets: &[usize]) -> Rectangle {
    let gap = sys::space::gap::s();
    let mut pills: Vec<Box<dyn Widget>> = Vec::new();
    let mut total = 0.0;
    for (shown, &bucket) in buckets.iter().enumerate() {
        let name = model::CATEGORIES[bucket];
        total += pill_width(name) + if shown > 0 { gap } else { 0.0 };
        pills.push(boxed(pill(name, bucket, bucket == active)));
    }

    let offset = with(|state| {
        state.pills_max = (total - WIDTH).max(0.0);
        state.pills = state.pills.min(state.pills_max);
        state.pills
    });

    Rectangle::new()
        .width(WIDTH)
        .height(PILLS)
        .fill(Color::TRANSPARENT)
        .clip()
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: -offset,
        })
        .on_scroll(on_pill_scroll)
        .align_child(Start, Center)
        .child(Row::new(pills).gap(gap).align(Center))
}

fn pill_width(name: &str) -> f32 {
    text_width(name, sys::r#type::caption()) + sys::space::inset::m() * 2.0
}

fn pill(name: &'static str, bucket: usize, picked: bool) -> Rectangle {
    let (bg, fg) = if picked {
        (sys::color::accent::bg(), sys::color::accent::fg())
    } else {
        (
            sys::color::surface::raised::bg(),
            sys::color::surface::panel::fg(),
        )
    };
    Rectangle::new()
        .width(pill_width(name))
        .height(sys::size::control::height())
        .radius(sys::radius::raised())
        .fill(bg)
        .align_child(Center, Center)
        .on_click(move |_| pick(bucket))
        .child(Text::new(name).style(sys::r#type::caption()).color(fg))
}
