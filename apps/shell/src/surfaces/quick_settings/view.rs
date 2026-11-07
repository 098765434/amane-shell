//! How Quick Settings draws: the card unfolding over the bar, the grid or a page sliding in,
//! the sheet behind them, and the surface that holds it.

use super::*;
use amane_kit::layout::frame;
use amane_kit::reveal::{self, Swell};

/// Steps the card's opening towards `open` and gives how far it is, a
/// little past 1 on the spring. Closing is quicker than opening, and a
/// change of mind turns it around at the speed it had.
pub(super) fn unfold(open: bool) -> f32 {
    OPEN.with_borrow_mut(|reveal| {
        reveal.step_then(open, |was| {
            // opening from nothing onto the grid, the modules arrive in a
            // wave; frost that can only show or hide would pop in module by
            // module, and an edit or a page has better things to show
            let cascade = open
                && was < 0.05
                && PAGE.get() == Page::Grid
                && !edit::editing()
                && amane::blur_fades();
            if cascade {
                OPENED_AT.set(Some(Instant::now()));
            }
        })
    })
}

/// The surface: hidden while closed, else the card, as tall as the grid.
pub fn surface() -> LayerWindow {
    QuickSettings::watch();
    let open = is_open(Panel::QuickSettings);
    let unfolded = unfold(open);
    let shown = reveal::showing(open, unfolded);

    if !shown {
        hidden();
        return window(surface_width(), 1.0)
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    }

    // a page whose setting has gone gives way to the grid, or closes when
    // there's no grid behind it
    if open && PAGE.get() != Page::Grid && !pages::available(PAGE.get()) {
        if ALONE.get() {
            close();
        } else {
            back();
        }
    }
    let page = PAGE.get();
    // the zoom's spring overshoots a little; what it draws at is clamped
    let slide_raw = SLIDE.with_borrow_mut(|slide| slide.value());
    let slide = slide_raw.clamp(0.0, 1.0);
    if slide_raw >= 0.999 {
        SWAP.set(Page::Grid);
    }
    let detail = if page == Page::Grid {
        DETAIL.get()
    } else {
        page
    };
    let zooming = detail != Page::Grid && (page != Page::Grid || slide > 0.001);
    let swap = SWAP.get();
    // the canvas sits a margin in from the surface's edge: room for the
    // modules' shadows
    let inset = quick::grid::margin();

    // the grid sets the card's size, so it's always built; only what's on
    // screen is drawn, so a page hidden behind the grid takes no clicks and
    // starts no searches. The tile a page grows out of fades into it, and
    // gives the page its frost.
    let grid = tiles::grid(tiles::Motion {
        opened: OPENED_AT.get(),
        focus: (zooming && swap == Page::Grid)
            .then(|| (detail.controls(), 1.0 - ramp(slide, 0.0, TILE_FADE))),
    });
    // in edit mode the Add list beside the canvas widens the card to the left
    let width = grid.width + inset * 2.0;
    let grid_w = grid::Metrics::tokens().extent(grid::Canvas::tokens().cols);
    // once a page has been on the card, it's at least a page tall, so the
    // surface isn't resized as pages come and go
    let page_min = if page != Page::Grid || DETAIL.get() != Page::Grid {
        grid::Metrics::tokens().extent(MIN_PAGE_ROWS)
    } else {
        0.0
    };
    let inner = grid.height.max(page_min).max(1.0);
    // the grid sits at the bottom of that
    let grid_top = inner - grid.height.max(1.0);

    // a change of height at rest: the grid starts where it was and slides
    // to where the new height puts it
    let shift = SHIFT.with_borrow_mut(|shift| {
        let last = LAST_INNER.replace(inner);
        if open && last > 0.0 && (inner - last).abs() > 0.5 {
            *shift = motion::spring(shift.value() + (inner - last));
            shift.to(0.0);
        }
        shift.value()
    });

    // Closing, the card keeps the size it had, shrinking into the bar's
    // corner as it was. Nor does it resize while a control is held: the rows
    // may change under the drag, and the one size holds until the drop.
    let outer = if open && edit::drag().is_none() {
        (inner + inset * 2.0).max(1.0)
    } else {
        LAST_HEIGHT.get()
    };
    LAST_HEIGHT.set(outer);

    // the page fills the card; it grows out of its tile on the grid, or out
    // of the bar's corner when it was opened from the bar
    let full = Sheet {
        x: inset,
        y: inset,
        width: grid_w,
        height: inner,
        radius: quick::card::radius(),
        tint: Material::Popover.tint(),
    };
    let origin = match grid.rest(detail.controls()) {
        Some((slot, radius)) => Sheet {
            x: inset + slot.x,
            y: inset + grid_top + shift + slot.y,
            width: slot.width,
            height: slot.height,
            radius,
            tint: Material::Control.tint(),
        },
        None => {
            let cell = grid::Metrics::tokens().extent(1);
            Sheet {
                x: inset + grid_w - cell,
                y: inset + inner - cell,
                width: cell,
                height: cell,
                radius: cell / 2.0,
                tint: Material::Control.tint(),
            }
        }
    };

    let mut shapes: Vec<BlurShape> = Vec::new();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();

    if page == Page::Grid || slide < 0.999 {
        // the grid stays where it is and sinks back from the tile its page
        // grows out of, fading as the page covers it; from one page to
        // another, the old page sinks back in its place
        let mut frost: Vec<BlurShape> = Vec::new();
        let receding: Box<dyn Widget> = if swap != Page::Grid {
            frost.push(full.frost());
            boxed(sheet_layer(swap, full, inner, 1.0))
        } else {
            frost.extend(glass_region::transformed(
                grid.shapes,
                1.0,
                (0.0, 0.0),
                (inset, inset + grid_top + shift),
            ));
            boxed(
                frame(grid.width, inner)
                    .align_child(Start, End)
                    .translate(0.0, shift)
                    .child(Stack::new(vec![grid.widget])),
            )
        };
        let shrink = f32::blend(1.0, quick::page::recede(), slide_raw);
        // scaled about the origin's centre: a layer scales about its own,
        // then moves by how far apart the two are, scaled down
        let (cx, cy) = origin.centre();
        let moved = (
            (cx - width / 2.0) * (1.0 - shrink),
            (cy - outer / 2.0) * (1.0 - shrink),
        );
        let shows = 1.0 - ramp(slide, 0.0, GRID_FADE);
        // the frost sinks and fades with it
        shapes.extend(glass_region::faded(
            glass_region::transformed(frost, shrink, (cx, cy), (0.0, 0.0)),
            shows,
        ));
        layers.push(boxed(
            Rectangle::new()
                .width(Parent)
                .height(outer)
                .padding(pad(inset, inset))
                .align_child(Start, End)
                .clip()
                .opacity(shows)
                .translate(moved.0, moved.1)
                .scale(shrink)
                .child(Stack::new(vec![receding])),
        ));
    }
    if zooming {
        // a page is one glass sheet that grows from its tile's glass to the
        // card's, the frost with it at full strength: it takes over from
        // the tile's own, so the card never shows through half-frosted
        // where motion is reduced it fades in at its size instead, its frost
        // with it, rather than animating out of the tile's (no animating
        // into and out of blurs)
        let reduced = crate::theme::reduced_motion();
        let from = if reduced { full } else { origin };
        let sheet = Sheet::blend(from, full, slide);
        let frost = sheet.frost();
        shapes.push(if reduced { frost.fade(slide) } else { frost });
        layers.push(boxed(sheet_layer(detail, sheet, inner, slide)));
    }

    let content = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .clip()
        .child(Stack::new(layers).width(Parent).height(Parent));

    // The card swells in from the bar's corner, rising out of the bar:
    // scaled about its centre, then moved so its bottom-right corner stays
    // put and lowered by what's left of the rise, the frost swelling and
    // fading with it. A compositor that can't fade frost only shows or
    // hides it, so there the card arrives whole (frost and modules in the
    // same frame) rather than fading in over frost already at full
    // strength; going, the frost goes at once and the modules fade out over
    // the sharp desktop.
    let swell = Swell::new(open, unfolded, quick::open::scale());
    let scale = swell.scale;
    // under a bar along the top it drops out of the bar instead, its
    // top-right corner staying put
    let down = if bar::edge() == Vertical::Top {
        -1.0
    } else {
        1.0
    };
    let lift = down * (1.0 - unfolded) * motion::travel(OPEN_LIFT);
    let card = content.opacity(swell.opacity).scale(scale).translate(
        (1.0 - scale) * width / 2.0,
        down * (1.0 - scale) * outer / 2.0 + lift,
    );
    let pinned = if down > 0.0 { outer } else { 0.0 };
    let mut frost = swell.frost_moved(shapes, (width, pinned), (0.0, lift));

    // Where the card is on the surface. The surface covers the output while
    // the card shows, and the card keeps its place at the bar's right-hand
    // corner, the same place its margins would give it. Worked out from the
    // surface's size, so the one frame amane draws at the hidden size as it
    // shows (the view runs before the new size is sent) draws the card at
    // its corner too.
    let (screen_w, screen_h) = window_size();
    let margin = popover_margin();
    let card_at = (
        (screen_w - margin.right as f32 - width).max(0.0),
        if bar::edge() == Vertical::Top {
            margin.top as f32
        } else {
            (screen_h - margin.bottom as f32 - outer).max(0.0)
        },
    );
    let canvas_at = (card_at.0 + inset, card_at.1 + inset + grid_top + shift);
    customize::set_canvas_origin(canvas_at.0, canvas_at.1);

    frost = glass_region::transformed(frost, 1.0, (0.0, 0.0), card_at);
    let card = frame(width, outer)
        .translate(card_at.0, card_at.1)
        .child(card);
    let card_area = customize::Area {
        x: card_at.0,
        y: card_at.1,
        width,
        height: outer,
    };

    let editing = edit::editing() || customize::visible();
    let (root, input): (Vec<Box<dyn Widget>>, Option<Vec<InputArea>>) = if editing {
        // the Customize window beside the card, the control in hand over
        // both, and the one still layer every drag is measured on under all
        // of it (first, so nothing that changes under a drag comes before
        // it); clicks outside the two go through to what's under them
        let mut root: Vec<Box<dyn Widget>> = vec![customize::drag_layer(), boxed(card)];
        let mut input = vec![input_area(card_area)];
        if let Some(built) = customize::view((screen_w, screen_h), card_area) {
            root.push(built.widget);
            frost.extend(built.frost);
            input.push(input_area(built.area));
        }
        if let Some((ghost, shape)) = customize::ghost(canvas_at) {
            root.push(ghost);
            frost.push(shape);
        }
        (root, Some(input))
    } else {
        // a click anywhere outside the card closes it; one on the card,
        // between its controls or on a slider, stays with the card. Closing,
        // clicks go through to what's under it.
        let root = vec![
            boxed(
                Rectangle::new()
                    .width(Parent)
                    .height(Parent)
                    .on_click(|_| close()),
            ),
            boxed(
                frame(width, outer)
                    .translate(card_at.0, card_at.1)
                    .on_click(|_| {}),
            ),
            boxed(card),
        ];
        (root, (!open).then(Vec::new))
    };

    let surface = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .child(Stack::new(root).width(Parent).height(Parent));
    let mut layer = full_window().visible(true).blur_shapes(frost);
    if let Some(input) = input {
        layer = layer.input_region(input);
    }
    layer.child(track_cursor(surface))
}

/// `area` as a part of the surface that takes the pointer, rounded
/// outwards so no rim of it is missed.
pub(super) fn input_area(area: customize::Area) -> InputArea {
    let x = area.x.floor();
    let y = area.y.floor();
    InputArea {
        x: x as i32,
        y: y as i32,
        width: ((area.x + area.width).ceil() - x) as i32,
        height: ((area.y + area.height).ceil() - y) as i32,
    }
}

/// 0 before `from`, 1 after `to`, easing in and out between: one part of a
/// motion that runs across a larger one.
pub(in crate::surfaces) fn ramp(value: f32, from: f32, to: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `page` on its glass sheet at `sheet`, laid out for the card it fills,
/// `height` tall (and clipped to the sheet while it grows), `reveal` of the
/// way in.
pub(super) fn sheet_layer(page: Page, sheet: Sheet, height: f32, reveal: f32) -> Rectangle {
    let pad_in = quick::tile::pad();
    let grid_w = grid::Metrics::tokens().extent(grid::Canvas::tokens().cols);
    let content = frame(grid_w, height)
        .padding(pad(pad_in, pad_in))
        .align_child(Start, Start)
        .child(pages::page(page).build((height - pad_in * 2.0).max(1.0), reveal));
    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .align_child(Start, Start)
        .child(
            tile::sheet(
                sheet.width,
                sheet.height,
                sheet.radius,
                Material::Popover,
                sheet.tint,
                content,
            )
            .translate(sheet.x, sheet.y),
        )
}

/// How wide the surface is at rest: the canvas and the margin either side.
pub(super) fn surface_width() -> f32 {
    let px = grid::Metrics::tokens();
    px.extent(grid::Canvas::tokens().cols) + quick::grid::margin() * 2.0
}

/// The overlay surface in the bar's right-hand corner, `height` tall: what
/// stays mapped, hidden, while the card is closed.
pub(super) fn window(width: f32, height: f32) -> LayerWindow {
    let base = LayerWindow::new()
        .width(width)
        .height(height)
        .margin(popover_margin());
    overlay(keyed(base), QS_NS, bar::edge(), Horizontal::Right).output(panel::on_output())
}

/// The surface over the whole output while the card shows (clear of what
/// the bar reserves, as the card is): the card, and the Customize window
/// while editing.
pub(super) fn full_window() -> LayerWindow {
    let base = LayerWindow::new()
        .width(WindowSize::Full)
        .height(WindowSize::Full);
    overlay(keyed(base), QS_NS, bar::edge(), Horizontal::Right).output(panel::on_output())
}

/// `base` taking the keyboard: the card is modal while open, so Escape
/// works from the first moment, however it was opened; while editing, the
/// rest of the keys type into the Customize window's search.
pub(super) fn keyed(base: LayerWindow) -> LayerWindow {
    base.keyboard(Keyboard::Exclusive).on_key(|key| {
        if matches!(key, Key::Escape) {
            escape();
        } else if edit::editing() {
            customize::key(key);
        }
    })
}
