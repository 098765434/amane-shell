//! How Customize draws: the window unfolding, its sidebar, the pane of controls to add, and
//! its footer.

use super::*;
use crate::chrome::catalog as sheet;
use amane_kit::layout::frame;
use amane_kit::reveal::{self, Swell};

/// Steps the window's opening and gives how far it is, a little past 1 on
/// the spring; `None` once it has gone.
pub(super) fn unfold() -> Option<f32> {
    let opening = OPENING.get();
    let value = OPEN.with_borrow_mut(|open| open.value());
    if !reveal::showing(opening, value) {
        return None;
    }
    Some(value)
}

/// The window on a surface `screen` big, beside the `card`, or nothing
/// while it's closed.
pub(crate) fn view(screen: (f32, f32), card: Area) -> Option<Built> {
    let unfolded = unfold()?;
    let area = place(screen, card);
    let radius = quick::card::radius();

    let sheet = tile::sheet(
        area.width,
        area.height,
        radius,
        Material::Sheet,
        Material::Sheet.tint(),
        content(&area),
    );

    // it swells in about its centre, the frost with it
    let swell = Swell::new(OPENING.get(), unfolded, OPEN_SCALE);
    let glass = Material::Sheet.shape(area.x, area.y, area.width, area.height, radius, 1.0);
    let centre = (area.x + area.width / 2.0, area.y + area.height / 2.0);
    let frost = swell.frost(vec![glass], centre);
    let widget = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .align_child(Start, Start)
        .child(
            sheet
                .opacity(swell.opacity)
                .scale(swell.scale)
                .translate(area.x, area.y),
        );
    Some(Built {
        widget: boxed(widget),
        frost,
        area,
    })
}

/// The sheet: the categories beside the controls, over the footer.
const SHEET: sheet::Sheet = sheet::Sheet {
    id: "amane.qs.customize",
    sidebar: SIDEBAR,
    inset: INSET,
};

/// What's on the sheet: the sidebar, a hairline, the pane over the footer.
pub(super) fn content(area: &Area) -> Rectangle {
    let (field, picked) = with(|state| (state.field.clone(), state.category));
    let query = field.text().to_string();
    let search = sheet::Search {
        field: &field,
        placeholder: "Search Controls",
        clear: clear_query,
    };
    let sections = Category::ALL
        .into_iter()
        .map(|category| sheet::Section {
            key: category.key(),
            title: category.title(),
            glyph: category.glyph(),
            picked: category == picked,
            pick: Box::new(move || pick(category)),
        })
        .collect();
    // the rows scroll when the window is too short for them
    let scrolling = sheet::Scrolling {
        offset: Box::new(|max| {
            SIDE_SCROLL.with_borrow_mut(|scroll| {
                scroll.set_max(max);
                scroll.settle_stale();
                scroll.value()
            })
        }),
        on_scroll: on_side_scroll,
    };
    let sidebar = SHEET.sidebar(search, sections, area.height, Some(scrolling));
    let footer = sheet::Footer {
        hint: "Drag a control to place it in Quick Settings.",
        reset: "Reset Layout",
        undo: edit::can_undo().then_some(edit::undo),
        on_reset: edit::reset,
        on_done: edit::leave,
    };
    SHEET.build(
        area,
        sidebar,
        |viewport| pane(picked, &query, viewport, !query.trim().is_empty()),
        footer,
    )
}

/// The pane: the sections of controls, scrolling. `viewport` is where it
/// is on the surface, for the controls to record where they're drawn.
pub(super) fn pane(category: Category, query: &str, viewport: Area, searching: bool) -> Rectangle {
    let hidden: Vec<ControlId> = layout::with(|layout| layout.hidden().collect());
    let sections = catalog::sections(category, query, &hidden);
    let inner_w = (viewport.width - INSET * 2.0).max(1.0);
    let px = Metrics::tokens();
    let title_style = sys::r#type::title();
    let title_h = text_height(title_style);
    let gap_m = sys::space::gap::m();
    let gap_l = sys::space::gap::l();

    // laid out by hand, top to bottom, so every control's place is known
    let mut y = INSET;
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    // (id, rect in the pane's content)
    let mut placed: Vec<(ControlId, Area)> = Vec::new();
    let count = sections.len();
    for (i, section) in sections.into_iter().enumerate() {
        layers.push(boxed(
            frame(inner_w, title_h)
                .align_child(Start, Center)
                .translate(INSET, y)
                .child(
                    Text::new(section.title)
                        .style(title_style)
                        .color(material::label(Level::Primary)),
                ),
        ));
        y += title_h + gap_m;

        if section.ids.is_empty() {
            let note = if searching {
                "No controls match"
            } else {
                "Nothing here"
            };
            let note_h = text_height(sys::r#type::caption());
            layers.push(boxed(
                frame(inner_w, note_h)
                    .align_child(Start, Center)
                    .translate(INSET, y)
                    .child(caption(note)),
            ));
            y += note_h;
        } else {
            let mut flow = Flow::new(inner_w, px.gap, gap_l);
            let mut cells: Vec<(ControlId, Slot, f32)> = Vec::new();
            for id in &section.ids {
                let (item, item_w, item_h, slot) = catalog_item(*id, &px);
                flow = flow.item(item_w, item_h, item);
                cells.push((*id, slot, item_w));
            }
            let (flow, flow_h, spots) = flow.build();
            for ((id, slot, item_w), spot) in cells.into_iter().zip(spots) {
                placed.push((
                    id,
                    Area {
                        x: INSET + spot.x + (item_w - slot.width) / 2.0,
                        y: y + spot.y,
                        width: slot.width,
                        height: slot.height,
                    },
                ));
            }
            layers.push(boxed(flow.translate(INSET, y)));
            y += flow_h;
        }

        if i + 1 < count {
            y += gap_l;
            layers.push(boxed(
                frame(inner_w, Divider::height())
                    .translate(INSET, y)
                    .child(Divider::build()),
            ));
            y += Divider::height() + gap_l;
        }
    }
    let content_h = y + INSET;

    let scroll = SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max((content_h - viewport.height).max(0.0));
        scroll.settle_stale();
        if scroll.swiping() {
            moved();
        }
        scroll.value()
    });

    // where each control is on the surface, inside the pane
    with(|state| {
        state.tiles = placed
            .into_iter()
            .filter_map(|(id, at)| {
                let on_surface = Area {
                    x: viewport.x + at.x,
                    y: viewport.y + at.y - scroll,
                    ..at
                };
                on_surface.clipped(&viewport).map(|area| (id, area))
            })
            .collect();
    });

    let content = frame(viewport.width, content_h.max(viewport.height)).child(Stack::new(layers));
    ScrollView::new(Axis::Vertical, viewport.width, viewport.height)
        .content(content_h)
        .offset(scroll)
        .scrollbar(scrollbar())
        .on_scroll(on_pane_scroll)
        .build(content)
}

/// One control in the pane: the control at its default size on a plain
/// tint, taking nothing but the hand, with its name centred under it on
/// up to two lines. Gives the item, how wide and tall it is, and the
/// control's slot within it (at the top, centred).
pub(super) fn catalog_item(id: ControlId, px: &Metrics) -> (Rectangle, f32, f32, Slot) {
    let span = id.default_span();
    let slot = px.slot(Rect::new(Place::new(0, 0), span));
    // a round button's name needs more room than its disc
    let item_w = slot.width.max(px.cell + px.gap);
    let control = tile::plain(|| {
        controls::build(id, span, &Ctx::new(true)).unwrap_or_else(|| tiles::placeholder(id, slot))
    });
    let control = edit::body(control, slot);

    let style = quick::r#type::caption();
    let line_h = quick::r#type::line().max(text_height(style));
    let lines = wrapped_lines(id.title(), style, item_w, 2);
    let gap = sys::space::gap::xs();
    let caption_h = lines.len() as f32 * line_h;
    let item_h = slot.height + gap + caption_h;

    let name: Vec<Box<dyn Widget>> = lines
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

    let item = frame(item_w, item_h).child(Stack::new(vec![
        boxed(
            frame(slot.width, slot.height)
                .translate((item_w - slot.width) / 2.0, 0.0)
                .child(Stack::new(vec![control])),
        ),
        boxed(
            frame(item_w, caption_h)
                .translate(0.0, slot.height + gap)
                .child(amane::Column::new(name)),
        ),
    ]));
    (item, item_w, item_h, slot)
}

/// How much the scrollbar shows: while the pane moves, and a little after.
pub(super) fn scrollbar() -> f32 {
    SCROLLBAR.with_borrow_mut(|bar| bar.opacity::<super::super::QuickSettings>())
}
