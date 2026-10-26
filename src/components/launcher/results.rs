//! What a search shows under the search row: an answer in a card, chips
//! that narrow the search to an app, and a row an entry, its icon (the
//! app's, or a glyph on a raised tile, with a badge saying where it's
//! from), its title, a line under it (the fuller one while Super is held)
//! and a note at its end, one highlight gliding to the row picked. In
//! Actions the rows come in sections under headings, and each ends with
//! its quick keys, or an offer to set them. It scrolls as the grid does,
//! in a [`ScrollView`].

use amane::{
    Button, Center, Color, Column, End, Rectangle, Row, Stack, Start, Text, Widget, mark_all,
};

use crate::components::app_icon;
use crate::components::color::with_alpha;
use crate::components::icons::{ICON_APPS, ICON_SEARCH};
use crate::components::kinetic::Axis;
use crate::components::layout::{boxed, icon, pad, slot, text_width};
use crate::components::material::{self, FillLevel, Level};
use crate::components::scroll_view::ScrollView;
use crate::tokens::{Styled, TextStyle, sys};

use super::entries::{self, Action, Entry, Icon, Scope};
use super::grid::At;
use super::mode::Mode;
use super::{
    ANSWER_H, CHIP_H, Capture, HEADING, Layout, ROW, ROW_GAP, app_menu, enter_scope, header,
    headings, hover, on_body_scroll, start_capture, with,
};

// TODO(token): `launcher.result.icon`, `launcher.result.badge`,
// `launcher.answer.*` and `launcher.empty.icon` once Penpot has them
// (manual §1.1).
/// How big a row's icon is, and the badge on its corner.
const RESULT_ICON: f32 = 40.0;
const BADGE: f32 = 16.0;
/// How big an answer is in its card.
const ANSWER_TEXT: f32 = 30.0;
/// How big the glyph over an empty state's note is.
const EMPTY_ICON: f32 = 40.0;
/// How heavy a section's heading is.
const HEADING_WEIGHT: u16 = 600;
/// How tall the offer to set an action's quick keys is, and how wide the
/// box the keys are typed into.
const OFFER_H: f32 = 26.0;
const CAPTURE_W: f32 = 190.0;

const ADD_QUICK_KEYS: &str = "Add Quick Keys";

/// `entries` as rows in a list `width` wide and `height` tall in view,
/// what was searched for `query`, a heading over each section. `assign`
/// offers to set each action's quick keys (Actions).
pub(super) fn list(
    entries: &[Entry],
    at: At,
    width: f32,
    height: f32,
    layout: &Layout,
    query: &str,
    assign: bool,
) -> Rectangle {
    // over the answer's card, the highlight in the list has nothing to show
    let on_row = layout.span(at.selected).1 > 0.0;
    let skip = layout.skip();
    let highlight = Rectangle::new()
        .width(width)
        .height(ROW)
        .radius(sys::radius::popover())
        .fill(sys::color::state::selected())
        .opacity(if on_row { at.highlight } else { 0.0 })
        .translate(0.0, at.y);
    let capture = with(|state| state.capture.clone());
    let starts = headings(entries);
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        if starts.contains(&index)
            && let Some(section) = &entry.section
        {
            rows.push(boxed(heading(section, width)));
        }
        let trailing = trailing(entry, assign, capture.as_ref());
        rows.push(boxed(row(
            entry,
            index + skip,
            width,
            at.reveal,
            query,
            trailing,
        )));
    }

    ScrollView::new(Axis::Vertical, width, height)
        .content(layout.content(entries.len()))
        .offset(at.scroll)
        .scrollbar(at.scrollbar)
        .on_scroll(on_body_scroll)
        .build(Stack::new(vec![
            boxed(highlight),
            boxed(Column::new(rows).gap(ROW_GAP)),
        ]))
        .opacity(at.fade)
}

/// A section's name over its rows, a hairline under it, as Spotlight's.
fn heading(name: &str, width: f32) -> Rectangle {
    let style = TextStyle {
        weight: HEADING_WEIGHT,
        ..sys::r#type::label()
    };
    let inset = sys::space::inset::l();
    Rectangle::new()
        .width(width)
        .height(HEADING - ROW_GAP)
        .fill(Color::TRANSPARENT)
        .padding(pad(inset, 0.0))
        .align_child(Start, End)
        .child(
            Column::new(vec![
                boxed(
                    Text::new(name)
                        .style(style)
                        .color(material::label(Level::Primary)),
                ),
                boxed(
                    Rectangle::new()
                        .width(width - inset * 2.0)
                        .height(sys::border::hairline())
                        .fill(material::separator()),
                ),
            ])
            .gap(sys::space::gap::xs()),
        )
}

/// What ends a row, and how wide it is: the keys being typed for it, its
/// quick keys, an offer to set them, or its note.
fn trailing(
    entry: &Entry,
    assign: bool,
    capture: Option<&Capture>,
) -> Option<(Box<dyn Widget>, f32)> {
    let title = entry.action_title.clone();
    if let (Some(capture), Some(_)) = (capture, &title)
        && capture.action == entry.key
    {
        return Some((boxed(capture_box(capture)), CAPTURE_W));
    }
    if let (Some(keys), Some(title)) = (&entry.quick_keys, &title) {
        let gap = sys::space::gap::_2xs();
        let caps: Vec<Box<dyn Widget>> = keys
            .chars()
            .map(|key| boxed(header::keycap(&key.to_uppercase().to_string())))
            .collect();
        let width = keys
            .chars()
            .map(|key| header::keycap_width(&key.to_uppercase().to_string()))
            .sum::<f32>()
            + gap * keys.chars().count().saturating_sub(1) as f32;
        let action = entry.key.clone();
        let (title, keys) = (title.clone(), keys.clone());
        // a click on them changes them
        let caps = Rectangle::new()
            .width(width)
            .height(ROW)
            .fill(Color::TRANSPARENT)
            .align_child(Start, Center)
            .on_click(move |_| start_capture(&action, &title, &keys))
            .child(Row::new(caps).gap(gap).align(Center));
        return Some((boxed(caps), width));
    }
    if assign
        && let Some(title) = title
        && !matches!(entry.action, Action::Hint)
    {
        let style = sys::r#type::caption();
        let inset = sys::space::inset::s();
        let width = text_width(ADD_QUICK_KEYS, style) + inset * 2.0;
        let action = entry.key.clone();
        return Some((
            boxed(
                Rectangle::new()
                    .width(width)
                    .height(OFFER_H)
                    .radius(sys::radius::raised() / 2.0)
                    .fill(Color::TRANSPARENT)
                    .border(sys::border::hairline(), material::separator())
                    .align_child(Center, Center)
                    .on_click(move |_| start_capture(&action, &title, ""))
                    .child(
                        Text::new(ADD_QUICK_KEYS)
                            .style(style)
                            .color(material::label(Level::Tertiary)),
                    ),
            ),
            width,
        ));
    }
    entry.trailing.as_ref().map(|note| {
        let style = sys::r#type::caption();
        let text = Text::new(note.clone())
            .style(style)
            .color(material::label(Level::Secondary));
        (boxed(text) as Box<dyn Widget>, text_width(note, style))
    })
}

/// The quick keys being typed for an action: what's typed so far, or why
/// it can't be saved.
fn capture_box(capture: &Capture) -> Rectangle {
    let style = sys::r#type::caption();
    let (note, color) = match &capture.error {
        Some(error) => (error.clone(), sys::color::status::critical::bg()),
        None if capture.keys.is_empty() => (
            String::from("Type keys, then Return"),
            material::label(Level::Secondary),
        ),
        None => (
            format!("{} · Return to save", capture.keys.to_uppercase()),
            material::label(Level::Primary),
        ),
    };
    Rectangle::new()
        .width(CAPTURE_W)
        .height(OFFER_H)
        .radius(sys::radius::raised() / 2.0)
        .fill(Color::TRANSPARENT)
        .border(2.0, sys::color::accent::bg())
        .padding(pad(sys::space::inset::s(), 0.0))
        .align_child(Center, Center)
        .clip()
        .child(Text::new(note).style(style).color(color).elide())
}

/// A row's icon: the app's, or a glyph on a raised tile, with `badge` on
/// its corner.
fn leading(entry: &Entry) -> Rectangle {
    let tile: Box<dyn Widget> = match &entry.icon {
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
    let mut layers = vec![tile];
    if let Some(badge) = entry.badge {
        layers.push(boxed(
            slot(BADGE)
                .radius(BADGE / 2.0)
                .fill(sys::color::accent::bg())
                .child(icon(badge, BADGE * 0.7, sys::color::accent::fg()))
                .translate(RESULT_ICON - BADGE + 2.0, RESULT_ICON - BADGE + 2.0),
        ));
    }
    slot(RESULT_ICON)
        .align_child(Start, Start)
        .child(Stack::new(layers))
}

fn row(
    entry: &Entry,
    index: usize,
    width: f32,
    reveal: bool,
    query: &str,
    trailing: Option<(Box<dyn Widget>, f32)>,
) -> Rectangle {
    let clicked = entry.clone();
    let query = query.to_string();
    let title = Text::new(entry.title.clone())
        .style(sys::r#type::title())
        .color(material::label(Level::Primary))
        .elide();
    let line = if reveal {
        entry.detail.as_ref().or(entry.subtitle.as_ref())
    } else {
        entry.subtitle.as_ref()
    };
    let mut lines: Vec<Box<dyn Widget>> = vec![boxed(title)];
    if let Some(line) = line {
        lines.push(boxed(
            Text::new(line.clone())
                .style(sys::r#type::caption())
                .color(material::label(Level::Secondary))
                .elide(),
        ));
    }
    let text = Column::new(lines).gap(sys::space::gap::_2xs());

    let inset = sys::space::inset::l();
    let gap = sys::space::gap::xl();
    let trailing_w = trailing.as_ref().map_or(0.0, |(_, width)| width + gap);
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(leading(entry))];
    let text_w = (width - inset * 2.0 - RESULT_ICON - gap - trailing_w).max(0.0);
    parts.push(boxed(
        Rectangle::new()
            .width(text_w)
            .height(ROW)
            .fill(Color::TRANSPARENT)
            .clip()
            .align_child(Start, Center)
            .child(text),
    ));
    if let Some((trailing, _)) = trailing {
        parts.push(trailing);
    }

    Rectangle::new()
        .width(width)
        .height(ROW)
        .fill(Color::TRANSPARENT)
        .radius(sys::radius::popover())
        .padding(pad(inset, 0.0))
        .align_child(Start, Center)
        .on_hover(hover(index))
        .on_click(move |button| match (button, &clicked.action) {
            (Button::Left, _) => entries::run(&clicked, &query),
            (Button::Right, Action::Launch(app)) => app_menu::open(app),
            _ => {}
        })
        .child(Row::new(parts).gap(gap).align(Center))
}

/// The answer to a sum or a conversion, in a card over the list, `width`
/// wide; `selected` rings it as what Enter copies.
pub(super) fn answer_card(answer: &Entry, width: f32, selected: bool, query: &str) -> Rectangle {
    let clicked = answer.clone();
    let query = query.to_string();
    let accent = sys::color::accent::bg();
    let on = sys::color::accent::fg();
    let big = TextStyle {
        size: ANSWER_TEXT,
        ..sys::r#type::numeric()
    };
    let mut lines: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(asked) = &answer.subtitle {
        lines.push(boxed(
            Text::new(format!("{asked} ="))
                .style(sys::r#type::caption())
                .color(with_alpha(on, 0.8)),
        ));
    }
    lines.push(boxed(
        Text::new(answer.title.clone()).style(big).color(on).elide(),
    ));
    let copy_hint = Text::new("Return to copy")
        .style(sys::r#type::caption())
        .color(with_alpha(on, 0.8));

    let inset = sys::space::inset::l();
    Rectangle::new()
        .width(width)
        .height(ANSWER_H)
        .radius(sys::radius::popover())
        .fill(accent)
        .border(
            if selected { 2.0 } else { 0.0 },
            with_alpha(on, if selected { 0.7 } else { 0.0 }),
        )
        .padding(pad(inset, 0.0))
        .align_child(Start, Center)
        .on_click(move |_| entries::run(&clicked, &query))
        .child(
            Row::new(vec![
                boxed(
                    Rectangle::new()
                        .width(
                            (width
                                - inset * 2.0
                                - text_width("Return to copy", sys::r#type::caption()))
                            .max(0.0),
                        )
                        .height(ANSWER_H)
                        .fill(Color::TRANSPARENT)
                        .align_child(Start, Center)
                        .child(Column::new(lines).gap(sys::space::gap::_2xs())),
                ),
                boxed(
                    Rectangle::new()
                        .width(text_width("Return to copy", sys::r#type::caption()))
                        .height(ANSWER_H)
                        .fill(Color::TRANSPARENT)
                        .align_child(End, Center)
                        .child(copy_hint),
                ),
            ])
            .align(Center),
        )
}

/// Up to four places to narrow the search to, as chips of one width
/// across a row `width` wide.
pub(super) fn scope_chips(scopes: &[Scope], width: f32) -> Rectangle {
    let gap = sys::space::gap::s();
    let count = scopes.len().max(4) as f32;
    let chip_w = (width - gap * (count - 1.0)) / count;
    let chips: Vec<Box<dyn Widget>> = scopes
        .iter()
        .map(|scope| {
            let picked = scope.clone();
            boxed(
                Rectangle::new()
                    .width(chip_w)
                    .height(CHIP_H)
                    .radius(sys::radius::raised())
                    .fill(material::fill_role(FillLevel::Secondary))
                    .align_child(Center, Center)
                    .on_click(move |_| {
                        enter_scope(picked.clone());
                        mark_all();
                    })
                    // elided text fills its box, so the box is only as wide
                    // as the name: the chip centres it
                    .child(
                        Rectangle::new()
                            .width(
                                text_width(scope.name(), sys::r#type::caption())
                                    .min(chip_w - sys::space::inset::m() * 2.0),
                            )
                            .height(CHIP_H)
                            .fill(Color::TRANSPARENT)
                            .align_child(Center, Center)
                            .clip()
                            .child(
                                Text::new(scope.name())
                                    .style(sys::r#type::caption())
                                    .color(material::label(Level::Secondary))
                                    .elide(),
                            ),
                    ),
            )
        })
        .collect();
    Rectangle::new()
        .width(width)
        .height(CHIP_H)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Center)
        .child(Row::new(chips).gap(gap))
}

/// Why nothing shows.
#[derive(Clone, Copy)]
pub(super) enum Empty {
    /// The apps haven't been read yet.
    Loading,
    /// The category has nothing under it.
    Category,
    /// A mode that has nothing to show yet.
    Soon(Mode),
}

/// What shows where nothing does, `width` by `height`: a glyph over a note
/// saying why.
pub(super) fn empty(why: Empty, width: f32, height: f32) -> Rectangle {
    let (glyph, note) = match why {
        Empty::Loading => (ICON_SEARCH, "Loading…"),
        Empty::Category => (ICON_APPS, "Nothing in this category"),
        Empty::Soon(mode) => (mode.glyph(), "Not available yet"),
    };
    let secondary = material::label(Level::Secondary);
    Rectangle::new()
        .width(width)
        .height(height)
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
