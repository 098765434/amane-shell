//! How Customize draws: the window unfolding, its sidebar, the pane of controls to add, and
//! its footer.

use super::*;

/// Steps the window's opening and gives how far it is, a little past 1 on
/// the spring; `None` once it has gone.
pub(super) fn unfold() -> Option<f32> {
    let opening = OPENING.get();
    let value = OPEN.with_borrow_mut(|open| open.value());
    if !opening && value <= 0.001 {
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

    // it swells in about its centre, the frost with it (see the launcher)
    let fades = amane::blur_fades();
    let scale = f32::blend(motion::scale(OPEN_SCALE), 1.0, unfolded);
    let opacity = if OPENING.get() && !fades {
        1.0
    } else {
        unfolded.clamp(0.0, 1.0)
    };
    let frost = if OPENING.get() || fades {
        let glass = Material::Sheet.shape(area.x, area.y, area.width, area.height, radius, 1.0);
        let centre = (area.x + area.width / 2.0, area.y + area.height / 2.0);
        glass_region::faded(
            glass_region::transformed(vec![glass], scale, centre, (0.0, 0.0)),
            opacity,
        )
    } else {
        Vec::new()
    };
    let widget = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .child(
            sheet
                .opacity(opacity)
                .scale(scale)
                .translate(area.x, area.y),
        );
    Some(Built {
        widget: boxed(widget),
        frost,
        area,
    })
}

/// What's on the sheet: the sidebar, a hairline, the pane over the footer.
pub(super) fn content(area: &Area) -> Rectangle {
    let (field, category) = with(|state| (state.field.clone(), state.category));
    let query = field.text().to_string();
    let searching = !query.trim().is_empty();

    let pane_x = SIDEBAR + Divider::width();
    let pane_w = (area.width - pane_x).max(1.0);
    let footer_h = sys::size::control::height() + sys::space::gap::m() * 2.0;
    let pane_h = (area.height - footer_h - Divider::height()).max(1.0);

    let sidebar = sidebar(&field, category, area.height);
    let pane = pane(
        category,
        &query,
        Area {
            x: area.x + pane_x,
            y: area.y,
            width: pane_w,
            height: pane_h,
        },
        searching,
    );
    let right = amane::Column::new(vec![
        boxed(pane),
        boxed(Divider::build()),
        boxed(footer(pane_w, footer_h)),
    ])
    .width(Size::Fixed(pane_w));

    Rectangle::new()
        .width(Size::Fixed(area.width))
        .height(Size::Fixed(area.height))
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .child(
            Row::new(vec![
                boxed(sidebar),
                boxed(Divider::vertical(area.height)),
                boxed(right),
            ])
            .align(Start),
        )
}

/// The search field over the category rows, scrolling when they don't fit.
pub(super) fn sidebar(field: &Field, picked: Category, height: f32) -> Rectangle {
    let inner_w = SIDEBAR - INSET * 2.0;
    let gap = sys::space::gap::m();
    let search = SearchField::new("amane.qs.customize.search", field, true)
        .placeholder("Search Controls")
        .width(inner_w)
        .on_clear(clear_query)
        .build();

    let items: Vec<ListItem> = Category::ALL
        .into_iter()
        .map(|category| {
            ListItem::new(
                format!("amane.qs.customize.category.{}", category.key()),
                category.title(),
            )
            .leading_tile(category.glyph())
            .bare(true)
            .selected(category == picked)
            .on_press(move || pick(category))
        })
        .collect();
    let rows = Category::ALL.len() as f32;
    let rows_h = rows * sys::size::row::height() + (rows - 1.0) * sys::space::gap::xs();
    let list = list::with_row_width(inner_w, || List::new(items).build());

    let list_h = (height - INSET * 2.0 - SearchField::height() - gap).max(1.0);
    let scroll = SIDE_SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max((rows_h - list_h).max(0.0));
        scroll.settle_stale();
        scroll.value()
    });
    let list = ScrollView::new(Axis::Vertical, inner_w, list_h)
        .content(rows_h)
        .offset(scroll)
        .on_scroll(on_side_scroll)
        .build(list);

    Rectangle::new()
        .width(Size::Fixed(SIDEBAR))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(INSET, INSET))
        .align_child(Start, Start)
        .child(
            amane::Column::new(vec![boxed(search), boxed(list)])
                .gap(gap)
                .width(Size::Fixed(inner_w)),
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
            Rectangle::new()
                .width(Size::Fixed(inner_w))
                .height(Size::Fixed(title_h))
                .fill(Color::TRANSPARENT)
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
                Rectangle::new()
                    .width(Size::Fixed(inner_w))
                    .height(Size::Fixed(note_h))
                    .fill(Color::TRANSPARENT)
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
                Rectangle::new()
                    .width(Size::Fixed(inner_w))
                    .height(Size::Fixed(Divider::height()))
                    .fill(Color::TRANSPARENT)
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

    let content = Rectangle::new()
        .width(Size::Fixed(viewport.width))
        .height(Size::Fixed(content_h.max(viewport.height)))
        .fill(Color::TRANSPARENT)
        .child(Stack::new(layers));
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
                Rectangle::new()
                    .width(Size::Fixed(item_w))
                    .height(Size::Fixed(line_h))
                    .fill(Color::TRANSPARENT)
                    .align_child(Center, Center)
                    .child(
                        Text::new(line)
                            .style(style)
                            .color(material::label(Level::Primary)),
                    ),
            )
        })
        .collect();

    let item = Rectangle::new()
        .width(Size::Fixed(item_w))
        .height(Size::Fixed(item_h))
        .fill(Color::TRANSPARENT)
        .child(Stack::new(vec![
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(slot.width))
                    .height(Size::Fixed(slot.height))
                    .fill(Color::TRANSPARENT)
                    .translate((item_w - slot.width) / 2.0, 0.0)
                    .child(Stack::new(vec![control])),
            ),
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(item_w))
                    .height(Size::Fixed(caption_h))
                    .fill(Color::TRANSPARENT)
                    .translate(0.0, slot.height + gap)
                    .child(amane::Column::new(name)),
            ),
        ]));
    (item, item_w, item_h, slot)
}

/// The hint and, at the end, Undo Remove (while there's something to
/// undo), Reset Layout and Done, the one main action, in the accent.
pub(super) fn footer(width: f32, height: f32) -> Rectangle {
    let mut actions: Vec<Box<dyn Widget>> = Vec::new();
    if edit::can_undo() {
        actions.push(boxed(
            PushButton::new("amane.qs.customize.undo", "Undo Remove")
                .style(Style::Plain)
                .shape(Shape::Round)
                .on_press(edit::undo)
                .build(),
        ));
    }
    actions.push(boxed(
        PushButton::new("amane.qs.customize.reset", "Reset Layout")
            .style(Style::Plain)
            .shape(Shape::Round)
            .on_press(edit::reset)
            .build(),
    ));
    actions.push(boxed(
        PushButton::new("amane.qs.customize.done", "Done")
            .style(Style::Filled)
            .shape(Shape::Round)
            .text(sys::r#type::label())
            .on_press(edit::leave)
            .build(),
    ));
    let hint = caption("Drag a control to place it in Quick Settings.");
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(INSET, 0.0))
        .align_child(Start, Center)
        .child(split_row(hint, actions))
}

/// How much the scrollbar shows: while the pane moves, and a little after.
pub(super) fn scrollbar() -> f32 {
    SCROLLBAR.with_borrow_mut(|bar| bar.opacity::<super::super::QuickSettings>())
}
