//! The widget gallery — a frosted sheet at the bottom of the
//! screen while the desktop is edited, with a search field over the
//! categories (All Widgets, System, Batteries) beside a pane of every kind
//! of widget, drawn live and scaled to fit, named under it. A widget is
//! dragged from the pane onto the desktop, or clicked to go in the first
//! free cells; a footer carries the hint, Undo Remove, Reset and Done.
//!
//! It's drawn on the desktop's own surface (see `edit.rs`), which records
//! where each widget in the pane is for a press to find what's under it.

use amane_kit::reveal::Swell;
use std::cell::{Cell, RefCell};

use amane::{
    Animation, BlurShape, Center, Key, Rectangle, Stack, Start, Text, Vertical, Widget, modifiers,
};

use super::catalog;
use super::edit;
use super::kind::{Category, WidgetKey, WidgetKind};
use super::store::Settings;
use super::{Geometry, MARGIN, RADIUS, changed};
use crate::chrome::bar;
use crate::chrome::catalog as sheet;
use crate::tokens::{Styled, sys};
use amane_kit::flow::Flow;
use amane_kit::layout::{boxed, caption, frame, pad, text_height, wrapped_lines};
use amane_kit::material::{self, FillLevel, Level, Material};
use amane_kit::motion;
use amane_kit::text_edit::Field;

// TODO(token): `widget.gallery.{width,height,sidebar,inset,preview}` once
// Penpot has them (manual §1.1).
const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 420.0;
const SIDEBAR: f32 = 240.0;
const INSET: f32 = 20.0;
/// The box each widget's preview is scaled to fit.
const PREVIEW: f32 = 120.0;
/// How small the sheet is as it swells in.
const OPEN_SCALE: f32 = 0.96;

pub(super) use amane_kit::area::Area;

/// The sidebar's rows: every kind, or one category's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
    All,
    Only(Category),
}

impl Section {
    const ALL: [Section; 4] = [
        Self::All,
        Self::Only(Category::Clock),
        Self::Only(Category::System),
        Self::Only(Category::Battery),
    ];

    fn title(self) -> &'static str {
        match self {
            Self::All => "All Widgets",
            Self::Only(category) => category.title(),
        }
    }

    fn glyph(self) -> char {
        match self {
            Self::All => amane_kit::icons::ICON_APPS,
            Self::Only(category) => category.glyph(),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Only(Category::Clock) => "clock",
            Self::Only(Category::System) => "system",
            Self::Only(Category::Battery) => "battery",
        }
    }
}

/// The kinds the pane shows for `section` and `query`: a query searches
/// every kind by its name, what it shows and its category.
pub(super) fn kinds(section: Section, query: &str) -> Vec<WidgetKind> {
    let query = query.trim().to_lowercase();
    WidgetKind::ALL
        .into_iter()
        .filter(|kind| {
            if query.is_empty() {
                section == Section::All || section == Section::Only(kind.category())
            } else {
                [kind.title(), kind.blurb(), kind.category().title()]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&query))
            }
        })
        .collect()
}

struct State {
    field: Field,
    section: Section,
    /// The sheet this frame, on the surface.
    area: Option<Area>,
    /// The widgets in the pane this frame: what, where drawn, at what scale.
    tiles: Vec<(WidgetKind, Area, f32)>,
}

impl State {
    const fn new() -> Self {
        Self {
            field: Field::new(),
            section: Section::All,
            area: None,
            tiles: Vec::new(),
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
    static OPEN: RefCell<Animation> = RefCell::new(motion::pop(0.0));
    static OPENING: Cell<bool> = const { Cell::new(false) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(f)
}

/// Opens afresh: All Widgets, no query.
pub(super) fn open() {
    with(|state| *state = State::new());
    OPEN.with_borrow_mut(|open| {
        *open = motion::pop(0.0);
        open.to(1.0);
    });
    OPENING.set(true);
}

pub(super) fn close() {
    OPENING.set(false);
    with(|state| *state = State::new());
}

/// Clears the search; whether there was one.
pub(super) fn clear_query() -> bool {
    with(|state| {
        let had = !state.field.text().is_empty();
        state.field = Field::new();
        had
    })
}

/// A key for the search field.
pub(super) fn key(key: Key) {
    let pasted = |text: String| {
        with(|state| state.field.insert(&text));
        changed();
    };
    if with(|state| state.field.handle(key, modifiers(), pasted)) {
        changed();
    }
}

fn pick(section: Section) {
    with(|state| {
        state.section = section;
        state.field = Field::new();
    });
    changed();
}

/// Whether `point` on the surface is on the sheet.
pub(super) fn contains(point: amane::Point) -> bool {
    with(|state| {
        state
            .area
            .is_some_and(|area| area.contains(point.x, point.y))
    })
}

/// The widget in the pane under `point` on the surface: its kind, where
/// its preview is drawn, and the preview's scale.
pub(super) fn tile_at(point: amane::Point) -> Option<(WidgetKind, Area, f32)> {
    with(|state| {
        state
            .tiles
            .iter()
            .find(|(_, area, _)| area.contains(point.x, point.y))
            .copied()
    })
}

/// Where the sheet sits: centred at the bottom of the screen, over the
/// bar's edge when the bar is there, no wider or taller than the screen
/// allows.
fn place(geometry: &Geometry) -> Area {
    let width = WIDTH.min(geometry.width - 2.0 * MARGIN).max(1.0);
    let height = HEIGHT.min(geometry.height * 0.5).max(1.0);
    let bar = if bar::edge() == Vertical::Bottom {
        bar::height()
    } else {
        0.0
    };
    Area {
        x: ((geometry.width - width) / 2.0).round(),
        y: (geometry.height - bar - MARGIN - height).round(),
        width,
        height,
    }
}

/// The sheet over the whole surface, and the frost behind it.
pub(super) fn sheet(_output: &str, geometry: &Geometry) -> (Box<dyn Widget>, Vec<BlurShape>) {
    let area = place(geometry);
    let radius = sys::radius::popover().max(RADIUS);
    let opened = OPEN.with_borrow_mut(|open| open.value());
    // its frost always shows: the surface goes once it has closed
    let swell = Swell::new(true, opened, OPEN_SCALE);
    let content = content(&area);
    with(|state| state.area = Some(area));

    let surface = Rectangle::new()
        .width(area.width)
        .height(area.height)
        .radius(radius)
        .fill(Material::Sheet.fill())
        .clip()
        .child(content);
    let surface = Material::Sheet.lit(surface, radius);
    let centre = (area.x + area.width / 2.0, area.y + area.height / 2.0);
    let glass = Material::Sheet.shape(area.x, area.y, area.width, area.height, radius, 1.0);
    let frost = swell.frost(vec![glass], centre);
    let widget = Rectangle::new()
        .width(amane::Parent)
        .height(amane::Parent)
        .align_child(Start, Start)
        .child(
            surface
                .opacity(opened.clamp(0.0, 1.0))
                .scale(swell.scale)
                .translate(area.x, area.y),
        );
    (boxed(widget), frost)
}

/// A widget lifted from the gallery that has no cells yet (held off the
/// canvas, or over the sheet): drawn where the pointer has it, over
/// everything. Once the preview gives it cells, the desktop draws it.
pub(super) fn ghost(
    output: &str,
    geometry: &Geometry,
) -> Option<(Box<dyn Widget>, Vec<BlurShape>)> {
    let (key, at) = super::held(output)?;
    if key.id != edit::PROVISIONAL {
        return None;
    }
    let placed = super::with_desk(output, |desk| {
        desk.session
            .as_ref()
            .and_then(|s| s.preview.as_ref())
            .is_some_and(|preview| preview.rect_of(key).is_some())
    });
    if placed {
        return None;
    }
    let size = key.kind.default_size();
    let px = super::metrics();
    let (width, height) = (px.width(size.span()), px.height(size.span()));
    let (x, y) = (geometry.origin.0 + at.0, geometry.origin.1 + at.1);
    let card = super::card(
        width,
        height,
        catalog::build(key, size, width, height, &Settings::default()),
    );
    let frost = Material::Control.shape(x, y, width, height, RADIUS, 1.0);
    let widget = frame(width, height)
        .translate(x, y)
        .scale(super::DRAG_SCALE)
        .child(card);
    Some((boxed(widget), vec![frost]))
}

/// The sheet: the sections beside every kind of widget, over the footer.
const SHEET: sheet::Sheet = sheet::Sheet {
    id: "amane.widgets.gallery",
    sidebar: SIDEBAR,
    inset: INSET,
};

/// The sidebar, a hairline, the pane over the footer.
fn content(area: &Area) -> Rectangle {
    let (field, picked) = with(|state| (state.field.clone(), state.section));
    let query = field.text().to_string();
    let search = sheet::Search {
        field: &field,
        placeholder: "Search Widgets",
        clear: || {
            clear_query();
            changed();
        },
    };
    let sections = Section::ALL
        .into_iter()
        .map(|section| sheet::Section {
            key: section.key(),
            title: section.title(),
            glyph: section.glyph(),
            picked: section == picked,
            pick: Box::new(move || pick(section)),
        })
        .collect();
    let sidebar = SHEET.sidebar(search, sections, area.height, None);
    let footer = sheet::Footer {
        hint: "Drag a widget onto the desktop, or click it to add it.",
        reset: "Remove All",
        undo: edit::can_undo().then_some(edit::undo),
        on_reset: edit::reset,
        on_done: edit::leave_all,
    };
    SHEET.build(
        area,
        sidebar,
        |viewport| pane(&kinds(picked, &query), viewport, !query.trim().is_empty()),
        footer,
    )
}

/// Every kind in `kinds`, its preview live at its default size scaled to
/// fit [`PREVIEW`], named under it. `viewport` is where the pane is on the
/// surface, for each preview to record where it's drawn.
fn pane(kinds: &[WidgetKind], viewport: Area, searching: bool) -> Rectangle {
    let inner_w = (viewport.width - INSET * 2.0).max(1.0);
    let px = super::metrics();
    let style = sys::r#type::caption();
    let line_h = text_height(style);
    let gap = sys::space::gap::xs();
    let item_w = PREVIEW + sys::space::gap::m();
    let mut flow = Flow::new(inner_w, sys::space::gap::m(), sys::space::gap::l());
    let mut cells = Vec::new();
    for kind in kinds {
        let size = kind.default_size();
        let (width, height) = (px.width(size.span()), px.height(size.span()));
        let scale = (PREVIEW / width.max(height)).min(1.0);
        let key = WidgetKey { id: 0, kind: *kind };
        let card = super::card(
            width,
            height,
            catalog::build(key, size, width, height, &Settings::default()),
        )
        .fill(material::fill_role(FillLevel::Secondary));
        let drawn = (width * scale, height * scale);
        let lines = wrapped_lines(kind.title(), style, item_w, 2);
        let name_h = lines.len() as f32 * line_h;
        let names: Vec<Box<dyn Widget>> = lines
            .into_iter()
            .map(|line| {
                boxed(
                    frame(item_w, line_h).align_child(Center, Center).child(
                        Text::new(line)
                            .style(style)
                            .color(material::label(Level::Primary)),
                    ),
                )
            })
            .collect();
        // the card scales about its centre, so it's placed centred in the box
        let item = frame(item_w, PREVIEW + gap + name_h).child(Stack::new(vec![
            boxed(
                frame(width, height)
                    .translate((item_w - width) / 2.0, (PREVIEW - height) / 2.0)
                    .scale(scale)
                    .cursor(amane::Cursor::Grab)
                    .child(card),
            ),
            boxed(
                frame(item_w, name_h)
                    .translate(0.0, PREVIEW + gap)
                    .child(amane::Column::new(names)),
            ),
        ]));
        flow = flow.item(item_w, PREVIEW + gap + name_h, item);
        cells.push((*kind, drawn, scale));
    }
    let (flow, _flow_h, spots) = flow.build();
    with(|state| {
        state.tiles = cells
            .into_iter()
            .zip(spots)
            .map(|((kind, (w, h), scale), spot)| {
                let area = Area {
                    x: viewport.x + INSET + spot.x + (item_w - w) / 2.0,
                    y: viewport.y + INSET + spot.y + (PREVIEW - h) / 2.0,
                    width: w,
                    height: h,
                };
                (kind, area, scale)
            })
            .collect();
    });
    let body: Box<dyn Widget> = if kinds.is_empty() {
        boxed(caption(if searching {
            "No widgets match"
        } else {
            "Nothing here"
        }))
    } else {
        boxed(flow)
    };
    frame(viewport.width, viewport.height)
        .padding(pad(INSET, INSET))
        .align_child(Start, Start)
        .clip()
        .child(Stack::new(vec![body]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_shows_its_kinds_and_a_query_searches_them_all() {
        assert_eq!(kinds(Section::All, ""), WidgetKind::ALL.to_vec());
        assert_eq!(
            kinds(Section::Only(Category::Battery), ""),
            [WidgetKind::Battery]
        );
        assert!(!kinds(Section::Only(Category::System), "").contains(&WidgetKind::Battery));
        // a query looks past the section, at names, blurbs and categories
        assert_eq!(
            kinds(Section::Only(Category::Battery), "cpu"),
            [WidgetKind::Cpu, WidgetKind::System]
        );
        assert_eq!(kinds(Section::All, "  SWAP "), [WidgetKind::Memory]);
        assert!(kinds(Section::All, "teleport").is_empty());
    }
}
