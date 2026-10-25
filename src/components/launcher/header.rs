//! The rows over the grid: the search field, drawn by hand (the text, the
//! caret and the selection's wash), and the bar of category pills, which
//! scrolls sideways when they don't fit.
//!
//! The picked pill's fill is one piece sliding under the pills, so it
//! follows the fingers as they turn the category; the pills' names take
//! the accent's colour as it reaches them.

use amane::{Blend, Center, Color, Rectangle, Row, Stack, Start, Text, Widget};

use crate::components::color::with_alpha;
use crate::components::icons::{ICON_APPS, ICON_CLOSE};
use crate::components::kinetic::Axis;
use crate::components::layout::{boxed, icon, slot, text_width};
use crate::components::scroll_view::ScrollView;
use crate::tokens::{Styled, TextStyle, sys};

use super::{HEADER, PILL_SCROLL, PILLS, WIDTH, model, on_pill_scroll, turn_to, type_into, with};
use crate::components::material::{self, FillLevel, Level};
use crate::components::text_edit::Field;

// TODO(token): `launcher.search.size`, `launcher.search.caret`,
// `launcher.logo.size` and `launcher.focus-ring.*` once Penpot has them
// (manual §1.1).
/// How big the search text is.
const SEARCH_TEXT: f32 = 18.0;
/// The caret's width and height.
const CARET: (f32, f32) = (2.0, 22.0);
/// How big the launcher's mark beside the field is, and the clear button.
const LOGO: f32 = 24.0;
/// The keyboard focus ring round the picked pill: how thick, how far out,
/// and how strong.
const RING: f32 = 2.0;
const RING_GAP: f32 = 2.0;
const RING_ALPHA: f32 = 0.55;

fn search_style() -> TextStyle {
    TextStyle {
        size: SEARCH_TEXT,
        ..sys::r#type::title()
    }
}

/// The launcher's mark, the search field beside it and, while there's a
/// query, a button that clears it. The caret shows while the keys go to
/// the field (`focused`).
pub(super) fn search_row(field: &Field, focused: bool) -> Rectangle {
    let gap = sys::space::gap::xl();
    let logo = icon(ICON_APPS, LOGO, material::label(Level::Primary));
    let typed = !field.text().is_empty();
    let field_width = WIDTH - LOGO - gap - if typed { LOGO + gap } else { 0.0 };
    let mut parts = vec![
        boxed(logo),
        boxed(search_field(field, focused, field_width)),
    ];
    if typed {
        parts.push(boxed(clear_button()));
    }
    Rectangle::new()
        .width(WIDTH)
        .height(HEADER)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .child(Row::new(parts).gap(gap).align(Center))
}

/// A round button that empties the search.
fn clear_button() -> Rectangle {
    slot(LOGO)
        .radius(LOGO / 2.0)
        .fill(material::fill_role(FillLevel::Secondary))
        .on_click(|_| {
            type_into(|field| {
                *field = Field::new();
                true
            });
            amane::mark_all();
        })
        .child(icon(
            ICON_CLOSE,
            LOGO * 0.6,
            material::label(Level::Secondary),
        ))
}

/// The query as runs of text with the caret between them and the
/// selection washed behind its run, or a placeholder while it's empty.
fn search_field(field: &Field, focused: bool, width: f32) -> Rectangle {
    let style = search_style();
    let fg = material::label(Level::Primary);
    let run = |text: String| Text::new(text).style(style).color(fg);
    let caret = || {
        Rectangle::new()
            .width(CARET.0)
            .height(CARET.1)
            .radius(CARET.0 / 2.0)
            .fill(if focused {
                sys::color::accent::bg()
            } else {
                Color::TRANSPARENT
            })
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if field.text().is_empty() {
        parts.push(boxed(caret()));
        parts.push(boxed(
            Text::new("Search")
                .style(style)
                .color(material::label(Level::Secondary)),
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
/// fill sliding under them to `offset` (a page, between two while they
/// turn). `focused` rings it while the arrow keys pick the category.
pub(super) fn pill_bar(buckets: &[usize], offset: f32, focused: bool) -> Rectangle {
    let gap = sys::space::gap::s();
    let height = sys::size::control::height();
    let widths: Vec<f32> = buckets
        .iter()
        .map(|&bucket| pill_width(model::CATEGORIES[bucket]))
        .collect();
    let lefts: Vec<f32> = widths
        .iter()
        .scan(0.0, |x, width| {
            let left = *x;
            *x += width + gap;
            Some(left)
        })
        .collect();
    let total = lefts.last().zip(widths.last()).map_or(0.0, |(x, w)| x + w);

    // the fill, between the two pills either side of `offset`
    let last = buckets.len().saturating_sub(1);
    let below = (offset.floor().max(0.0) as usize).min(last);
    let above = (below + 1).min(last);
    let along = (offset - below as f32).clamp(0.0, 1.0);
    let blend = |values: &[f32]| match (values.get(below), values.get(above)) {
        (Some(&from), Some(&to)) => f32::blend(from, to, along),
        _ => 0.0,
    };
    let (fill_x, fill_w) = (blend(&lefts), blend(&widths));

    let scroll = PILL_SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max(total - WIDTH);
        scroll.settle_stale();
        // the pill picked stays in view
        let picked = (offset.round().max(0.0) as usize).min(last);
        let moved_on = with(|state| state.pills_for.replace(picked)) != Some(picked);
        if moved_on && !scroll.swiping() && !buckets.is_empty() {
            let margin = gap * 2.0;
            let (left, right) = (
                lefts[picked] - margin,
                lefts[picked] + widths[picked] + margin,
            );
            let at = scroll.target();
            if left < at {
                scroll.go(left);
            } else if right > at + WIDTH {
                scroll.go(right - WIDTH);
            }
        }
        scroll.value()
    });

    let backs: Vec<Box<dyn Widget>> = buckets
        .iter()
        .enumerate()
        .map(|(page, _)| {
            boxed(
                Rectangle::new()
                    .width(widths[page])
                    .height(height)
                    .radius(sys::radius::raised())
                    .fill(material::fill_role(FillLevel::Secondary))
                    .on_click(move |_| turn_to(page)),
            )
        })
        .collect();
    let names: Vec<Box<dyn Widget>> = buckets
        .iter()
        .enumerate()
        .map(|(page, &bucket)| {
            // the accent's colour, as much as the fill is under it
            let near = (1.0 - (page as f32 - offset).abs()).clamp(0.0, 1.0);
            let fg = Color::blend(
                material::label(Level::Primary),
                sys::color::accent::fg(),
                near,
            );
            boxed(
                Rectangle::new()
                    .width(widths[page])
                    .height(height)
                    .fill(Color::TRANSPARENT)
                    .align_child(Center, Center)
                    .child(
                        Text::new(model::CATEGORIES[bucket])
                            .style(sys::r#type::caption())
                            .color(fg),
                    ),
            )
        })
        .collect();

    let mut layers: Vec<Box<dyn Widget>> = vec![boxed(Row::new(backs).gap(gap))];
    if !buckets.is_empty() {
        layers.push(boxed(
            Rectangle::new()
                .width(fill_w)
                .height(height)
                .radius(sys::radius::raised())
                .fill(sys::color::accent::bg())
                .translate(fill_x, 0.0),
        ));
        if focused {
            let out = RING + RING_GAP;
            layers.push(boxed(
                Rectangle::new()
                    .width(fill_w + out * 2.0)
                    .height(height + out * 2.0)
                    .radius(sys::radius::raised() + out)
                    .fill(Color::TRANSPARENT)
                    .border(RING, with_alpha(sys::color::accent::bg(), RING_ALPHA))
                    .translate(fill_x - out, -out),
            ));
        }
    }
    layers.push(boxed(Row::new(names).gap(gap)));

    ScrollView::new(Axis::Horizontal, WIDTH, PILLS)
        .content(total)
        .offset(scroll)
        .on_scroll(on_pill_scroll)
        // as tall as a pill, so the focus ring round one doesn't move them
        .build(Stack::new(layers).height(height))
}

fn pill_width(name: &str) -> f32 {
    text_width(name, sys::r#type::caption()) + sys::space::inset::m() * 2.0
}
