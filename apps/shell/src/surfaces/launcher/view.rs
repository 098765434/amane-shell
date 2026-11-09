//! How the launcher draws: its panel, the glass behind it, the scrollbar, and which apps show.

use super::*;
use emaki_kit::layout::frame;

/// Where `rows` start a new section: a heading goes over each.
pub(super) fn headings(rows: &[Entry]) -> Vec<usize> {
    let mut last: Option<&str> = None;
    let mut at = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let section = row.section.as_deref();
        if section.is_some() && section != last {
            at.push(index);
        }
        last = section;
    }
    at
}

/// The apps used most, filled out to a row with the first of the rest:
/// none until any has been used from the launcher.
pub(super) fn suggestions(all: &[DesktopApp]) -> Vec<&DesktopApp> {
    let top = usage::read(|usage| usage.top("app:", COLS, emaki_kit::time::unix_s()));
    let mut picked: Vec<&DesktopApp> = top
        .iter()
        .filter_map(|key| all.iter().find(|app| entries::app_key(app) == *key))
        .collect();
    if picked.is_empty() {
        return picked;
    }
    for app in all {
        if picked.len() >= COLS {
            break;
        }
        if !picked.iter().any(|shown| shown.id() == app.id()) {
            picked.push(app);
        }
    }
    picked
}

/// The apps on category page `page`, the suggested ones over the first.
pub(super) fn apps_on<'a>(
    all: &'a [DesktopApp],
    buckets: &[usize],
    page: usize,
    suggested: &[&'a DesktopApp],
) -> Vec<&'a DesktopApp> {
    let mut apps = if page == 0 && suggested.len() == COLS {
        suggested.to_vec()
    } else {
        Vec::new()
    };
    apps.extend(under(all, bucket(buckets, page)));
    apps
}

/// The category on page `page`: All Applications while none are known.
pub(super) fn bucket(buckets: &[usize], page: usize) -> usize {
    buckets.get(page).copied().unwrap_or(0)
}

/// The apps under category `bucket`, in the list's order.
pub(super) fn under(apps: &[DesktopApp], bucket: usize) -> Vec<&DesktopApp> {
    apps.iter()
        .filter(|app| model::in_bucket(model::mask(app.categories(), app.exec()), bucket))
        .collect()
}

/// Steps the glass's opening towards `open` and gives how far it is, a
/// little past 1 on the spring. Closing is quicker than opening.
pub(super) fn unfold(open: bool) -> f32 {
    OPEN.with_borrow_mut(|reveal| reveal.step(open))
}

/// Steps `anim` towards `to` and gives where it is.
pub(super) fn ease(anim: &'static std::thread::LocalKey<RefCell<Animation>>, to: f32) -> f32 {
    anim.with_borrow_mut(|anim| {
        anim.to(to);
        anim.value()
    })
}

/// The sheet at `geometry`, and what's on it: laid out for the size it's
/// heading to and clipped to the size it is, so growing reveals the rows
/// rather than squeezing them.
pub(super) fn glass(view: &View, geometry: Geometry) -> Rectangle {
    let (sheet, radius) = geometry.drawn();
    let content = frame(view.width(), view.height().max(sheet.height))
        .align_child(Start, Start)
        .child(panel(view));
    let glass = Rectangle::new()
        .width(sheet.width)
        .height(sheet.height)
        .radius(radius)
        .fill(Material::Sheet.fill())
        .clip()
        .align_child(Start, Start)
        // the glass keeps its clicks from the surface under it, which
        // closes the launcher
        .on_click(|_| {})
        .child(content);
    Material::Sheet
        .lit(glass, radius)
        .translate(sheet.x, sheet.y)
}

/// The search row and, under it, what this moment shows.
pub(super) fn panel(view: &View) -> Column {
    let layout = view.layout();
    let count = view.count();
    let pages = view.buckets.len();
    let width = view.inner();

    let offset = PAGER.with_borrow_mut(|pager| {
        pager.settle_stale(pages, &paging_motion());
        if pager.page() >= pages && pages > 0 {
            pager.jump(pages - 1, &paging_motion());
        }
        pager.offset(pages, &paging_motion())
    });

    // what the body shows: a change starts the list afresh, snapping the
    // highlight and the scroll there rather than sliding them
    let showing = (
        view.kind,
        if view.kind == Kind::Apps {
            view.page
        } else {
            0
        },
    );
    let (before, switched) = with(|state| {
        state.pages = pages;
        state.selected = view.selected;
        let before = std::mem::replace(&mut state.shown, showing);
        (before, before != showing)
    });
    let selected = if switched { 0 } else { view.selected };
    let (to_x, to_y) = layout.highlight(selected);
    if switched {
        with(|state| state.selected = 0);
        HIGHLIGHT_X.with_borrow_mut(|x| *x = motion::fade(to_x, motion::duration::hover()));
        HIGHLIGHT_Y.with_borrow_mut(|y| *y = motion::fade(to_y, motion::duration::hover()));
        SCROLL.with_borrow_mut(|scroll| scroll.jump(0.0));
        // into or out of a search, the body fades in; a category slides
        if before.0 != showing.0 {
            SWITCH.with_borrow_mut(|switch| *switch = motion::fade(0.0, motion::duration::fade()));
        }
    }

    // how far the body scrolls
    let body_height = view.list_height();
    let scroll = SCROLL.with_borrow_mut(|scroll| {
        scroll.set_max(layout.content(count.saturating_sub(layout.skip())) - body_height);
        scroll.settle_stale();
        if scroll.swiping() {
            moved();
        }
        scroll.value()
    });

    // Super, pressed since the launcher opened, shows the rows' fuller lines
    let held = modifiers().logo;
    let reveal = with(|state| {
        if !held {
            state.logo_armed = true;
        }
        state.logo_armed && held
    });

    // a search's results are what Enter runs, whichever zone has the keys
    let focused = view.focus == Zone::Body || view.kind == Kind::Results;
    let at = grid::At {
        scroll,
        x: ease(&HIGHLIGHT_X, to_x),
        y: ease(&HIGHLIGHT_Y, to_y),
        highlight: ease(&HIGHLIGHT_ALPHA, if focused { 1.0 } else { UNFOCUSED }),
        fade: ease(&SWITCH, 1.0),
        scrollbar: scrollbar(),
        selected,
        reveal,
    };

    let (hover_mode, back_hover) = with(|state| (state.hover_mode, state.back_hover));
    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(header::search_row(
        view,
        view.focus == Zone::Search,
        hover_mode,
        back_hover,
    ))];
    if view.kind == Kind::Idle {
        return Column::new(parts).width(view.width());
    }
    parts.push(boxed(Divider::build()));

    let query = view.field.text().to_string();
    let mut body: Vec<Box<dyn Widget>> = Vec::new();
    match view.kind {
        Kind::Results => {
            if let Some(answer) = &view.found.answer {
                body.push(boxed(results::answer_card(
                    answer,
                    width,
                    selected == 0,
                    &query,
                )));
            }
            if !view.found.scopes.is_empty() {
                body.push(boxed(results::scope_chips(&view.found.scopes, width)));
            }
            let list_w = view.list_width();
            let nothing = match view.mode {
                Mode::Clipboard if query.is_empty() => Some(results::Empty::NothingCopied),
                Mode::Clipboard => Some(results::Empty::NoMatch),
                Mode::Files => Some(results::Empty::NoFiles),
                _ => None,
            }
            .filter(|_| view.found.rows.is_empty());
            let list: Box<dyn Widget> = if let Some(nothing) = nothing {
                boxed(results::empty(nothing, width, body_height))
            } else {
                boxed(results::list(
                    &view.found.rows,
                    at,
                    list_w,
                    body_height,
                    &layout,
                    &query,
                    view.mode == Mode::Actions,
                ))
            };
            if view.previews() {
                // the copy or file picked, beside the list
                let preview = view.entry(selected).and_then(|entry| entry.preview.clone());
                body.push(boxed(
                    Row::new(vec![
                        list,
                        boxed(results::preview(
                            preview.as_ref(),
                            width - list_w - GAP,
                            body_height,
                        )),
                    ])
                    .gap(GAP),
                ));
            } else {
                body.push(list);
            }
        }
        Kind::Ask => body.push(boxed(results::ask_clipboard(width, EMPTY_H - INSET * 2.0))),
        Kind::Apps => {
            body.push(boxed(header::pill_bar(
                &view.buckets,
                offset,
                view.focus == Zone::Pills,
                width,
            )));
            body.push(if view.all.is_empty() {
                boxed(results::empty(results::Empty::Loading, width, BODY))
            } else if view.grid() {
                boxed(grid::pages(view, offset, at))
            } else {
                boxed(results::list(
                    &view.found.rows,
                    at,
                    width,
                    BODY,
                    &layout,
                    "",
                    false,
                ))
            });
        }
        Kind::Idle => {}
    }
    parts.push(boxed(
        frame(view.width(), view.height() - PILL_H - Divider::height())
            .padding(pad(INSET, INSET))
            .align_child(Start, Start)
            .child(Column::new(body).gap(GAP)),
    ));
    Column::new(parts).width(view.width())
}

/// How much the scrollbar shows: while the body moves, and a little after.
pub(super) fn scrollbar() -> f32 {
    SCROLLBAR.with_borrow_mut(|bar| bar.opacity::<Launcher>())
}
