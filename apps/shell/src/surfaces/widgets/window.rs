//! A monitor's widget window, each frame: hidden, handing the cards to
//! the edit surface, frozen under the windows, or the cards themselves.

use super::*;

/// `monitor`'s desktop surface, or its edit surface when `edit_window`.
pub(super) fn desk_window(monitor: &Monitor, edit_window: bool) -> LayerWindow {
    // a change to the widgets anywhere redraws this window, hidden or not
    drop(Desktops::read());
    let output = monitor.name.clone();
    let geometry = Geometry::of(monitor.width as f32, monitor.height as f32);
    let name = if edit_window {
        namespace(&format!("edit.{output}"))
    } else {
        namespace(&output)
    };
    let layer = if edit_window {
        Layer::Overlay
    } else {
        Layer::Background
    };
    let showing = desktop().show_widgets;
    edit::forget_unplugged();
    if !showing && edit::editing(&output) {
        edit::leave_all();
    }
    let editing = edit::editing(&output);
    if !showing || (edit_window && !editing) {
        watching(name, false, false);
        return hidden_window(layer, name);
    }

    let layout = shown(&output, geometry.canvas);
    let (window, geometry) = surface_for(&output, edit_window, geometry, &layout);
    let full = InputArea {
        x: 0,
        y: 0,
        width: geometry.width.ceil() as i32,
        height: geometry.height.ceil() as i32,
    };
    let window = window
        .vibrant(material::vibrant())
        .layer(layer)
        .space(Zone::Ignore)
        .namespace(name)
        .visible(true);
    if editing && !edit_window {
        watching(name, false, false);
        // the edit surface has the cards for now
        return window
            .keyboard(Keyboard::None)
            .click_through()
            .blur_shapes(Vec::new())
            .child(Stack::new(Vec::new()));
    }
    let layout = if note_geometry(&output, geometry) {
        shown(&output, geometry.canvas)
    } else {
        layout
    };

    /*
     * nothing under the windows shows (niri's overview fully open, or windows
     * over the whole screen): the surface keeps its last frame and draws
     * nothing, reading only what says when it shows again
     */
    let hidden = !editing && Desktop::read().obscured(&output);
    gate_readings(name, &layout, editing, hidden);
    let thawed = with_desk(&output, |desk| std::mem::replace(&mut desk.frozen, hidden)) && !hidden;
    if hidden {
        return window
            .keyboard(Keyboard::None)
            .input_region(vec![full])
            .frozen(true)
            .child(Stack::new(Vec::new()));
    }
    if thawed {
        settle(&output, &layout);
    }

    let dim = dimming(&output, editing, thawed);
    let view = view(&output, &geometry, &layout, dim, editing);
    let window = window
        .keyboard(if editing {
            Keyboard::Exclusive
        } else {
            Keyboard::None
        })
        .input_region(vec![full])
        .blur_shapes(view.frost)
        .child(view.root);
    if editing {
        let on = output.clone();
        window.on_key(move |key| keys::key(&on, key))
    } else {
        window
    }
}

/// A window that shows nothing and takes nothing.
fn hidden_window(layer: Layer, name: &'static str) -> LayerWindow {
    LayerWindow::new()
        .width(Full)
        .height(Full)
        .layer(layer)
        .space(Zone::Ignore)
        .keyboard(Keyboard::None)
        .namespace(name)
        .visible(false)
        .click_through()
        .child(Stack::new(Vec::new()))
}

/// The window's size and place, and the canvas against it. The edit
/// surface covers the monitor (the gallery, and cards dragged anywhere);
/// the background one covers the cards on it and the room for their
/// shadows, since a surface costs by its pixels, and the whole canvas only
/// while a card is dragged across it: the drag's points are kept in the
/// canvas's pixels, so the surface growing under it is fine.
fn surface_for(
    output: &str,
    edit_window: bool,
    geometry: Geometry,
    layout: &Layout<WidgetKey>,
) -> (LayerWindow, Geometry) {
    if edit_window {
        return (LayerWindow::new().width(Full).height(Full), geometry);
    }
    let dragging = with_desk(output, |desk| desk.session.is_some());
    let extent = if dragging {
        geometry.whole()
    } else {
        cards_extent(layout)
    };
    let geometry = geometry.on_surface_around(extent);
    let (left, top, width, height) = geometry.surface;
    let window = LayerWindow::new()
        .width(WindowSize::Fixed(width))
        .height(WindowSize::Fixed(height))
        .anchor_vertical(Vertical::Top)
        .anchor_horizontal(Horizontal::Left)
        .margin(Margin {
            top: top as i32,
            left: left as i32,
            ..Margin::default()
        });
    (window, geometry)
}

/// Keeps `geometry` as where the pointer meets `output`'s grid; whether
/// the canvas changed size, when a drag measured on the old one is let go
/// of.
fn note_geometry(output: &str, geometry: Geometry) -> bool {
    with_desk(output, |desk| {
        let before = desk.geometry.replace(geometry).map(|g| g.canvas);
        let resized = before.is_some_and(|canvas| canvas != geometry.canvas);
        if resized && desk.session.take().is_some() {
            desk.cancelled = true;
            desk.dragged = true;
        }
        resized
    })
}

/// Says whether window `name` needs the system's readings: while it shows
/// a card that reads them, or the gallery (every kind, live, and the
/// settings' sensor list). A covered desktop pauses them, so it costs no
/// wake-ups and the graphs carry on from where they were when it shows
/// again (`EMAKI_STATS_WHILE_HIDDEN=1` keeps gathering under the windows).
fn gate_readings(name: &'static str, layout: &Layout<WidgetKey>, editing: bool, hidden: bool) {
    let reads =
        |wanted: fn(Reads) -> bool| layout.placed().any(|(key, _)| wanted(key.kind.reads()));
    let system = reads(|r| r != Reads::Nothing);
    let heat = reads(|r| r == Reads::Temperatures);
    let gather = !hidden || stats_while_hidden();
    watching(
        name,
        (editing || system) && gather,
        (editing || heat) && gather,
    );
}

/// Shown again after being covered: the cards come back as they are now,
/// nothing animating, the clock at the time without drawing its changes.
fn settle(output: &str, layout: &Layout<WidgetKey>) {
    motion::forget(&scope(output));
    for (key, _) in layout.placed() {
        if key.kind == WidgetKind::Clock {
            catalog::settle_clock(key.id);
        }
    }
}
