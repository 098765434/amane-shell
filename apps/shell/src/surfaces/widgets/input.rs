//! The pointer on a desktop: pressing, dragging and letting go of a card, and clicks.

use super::*;
use crate::chrome::grid_edit::{self, Landing};

/// The still layer over the whole surface that every press, drag and
/// click lands on (the cards take nothing): it never moves, and comes
/// first, so the target amane keeps for a drag in flight stays put.
pub(super) fn still_layer(output: &str, geometry: Geometry, holding: bool) -> Rectangle {
    let (move_on, drag_on, release_on, click_on) = (
        output.to_string(),
        output.to_string(),
        output.to_string(),
        output.to_string(),
    );
    let layer = blank()
        .on_track(move |point: Point| with_desk(&move_on, |desk| desk.pointer = point))
        .on_drag(move |point: Point| {
            with_desk(&drag_on, |desk| desk.pointer = point);
            pressed(&drag_on, geometry, point);
        })
        .on_release(move |_| released(&release_on))
        .on_click(move |button| clicked(&click_on, geometry, button));
    match drag(output) {
        Some(Drag::Resize { .. }) => layer.cursor(Cursor::ResizeBottomRight),
        _ if holding => layer.cursor(Cursor::Grabbing),
        _ => layer,
    }
}

/// The button is down with the pointer at `point` on the surface.
pub(super) fn pressed(output: &str, geometry: Geometry, point: Point) {
    let at = geometry.grid(point);
    let px = metrics();
    if with_desk(output, |desk| desk.cancelled) {
        return;
    }
    if edit::drag_at(output, &px, geometry, point) {
        return;
    }
    let begin = with_desk(output, |desk| {
        if let Some(session) = &mut desk.session {
            session.motion(at, &px, widget::drag::band());
            return None;
        }
        match desk.press {
            None => {
                // a new press: a drag before it that no click followed is over
                desk.dragged = false;
                let handle = edit::handle_side(output);
                match hit(&desk.slots, at, handle) {
                    // a handle resizes at once, while editing
                    Some(hit @ Hit::Handle(..)) if edit::editing(output) => {
                        return Some(grid_edit::begin(hit, at, at));
                    }
                    // a card moves once the pointer has gone far enough
                    // that the press isn't a click
                    Some(Hit::Body(key, slot) | Hit::Handle(key, slot)) => {
                        desk.press = Some((at, key, slot));
                    }
                    None => desk.press = None,
                }
                None
            }
            Some((from, key, slot)) => ((at.x - from.x).hypot(at.y - from.y)
                > widget::drag::threshold())
            .then(|| grid_edit::begin(Hit::Body(key, slot), from, at)),
        }
    });
    if let Some(drag) = begin {
        let base = shown(output, geometry.canvas);
        with_desk(output, |desk| {
            desk.press = None;
            let mut session = Session::new(base, drag);
            session.motion(at, &px, widget::drag::band());
            desk.session = Some(session);
        });
    }
    // the pointer's own window draws again for its input; nothing else shows the drag
}

/// The button came up: what the drag previewed is kept. A card let go off
/// the canvas goes back where it was, or, while editing, is removed.
pub(super) fn released(output: &str) {
    let editing = edit::editing(output);
    let (session, pointer) = with_desk(output, |desk| {
        desk.press = None;
        desk.cancelled = false;
        let session = desk.session.take();
        desk.dragged |= session.is_some();
        (session, desk.pointer)
    });
    let on_gallery = edit::over_gallery(output, pointer);
    if let Some(session) = session {
        let landing = match grid_edit::landing(&session, &metrics()) {
            // let go on the gallery: as good as off the canvas
            Landing::Moved { id, origin, .. } if on_gallery => Landing::Off { id, origin },
            landing => landing,
        };
        match landing {
            Landing::Off { id, origin } => {
                if editing && origin == Origin::Canvas {
                    edit::remove(output, id);
                }
            }
            Landing::Moved { id, at, origin } => {
                if let Some(layout) = session.preview {
                    motion::jump(
                        &scope(output),
                        u64::from(id.id),
                        Pose {
                            scale: quick::drag::scale(),
                            ..Pose::at(at.1).moved(at.0)
                        },
                    );
                    if origin == Origin::Window {
                        edit::placed(output, id, &layout);
                    } else {
                        commit(output, &layout);
                    }
                }
            }
            Landing::Resized => {
                if let Some(layout) = session.preview {
                    commit(output, &layout);
                }
            }
        }
    }
    edit::released(output);
    changed();
}

pub(super) fn clicked(output: &str, geometry: Geometry, button: Button) {
    let (dragged, pointer, under) = with_desk(output, |desk| {
        let at = geometry.grid(desk.pointer);
        let under = match hit(&desk.slots, at, 0.0) {
            Some(Hit::Body(key, _) | Hit::Handle(key, _)) => Some(key),
            None => None,
        };
        (std::mem::take(&mut desk.dragged), desk.pointer, under)
    });
    if dragged {
        return;
    }
    let at = (pointer.x as i32, pointer.y as i32);
    if edit::editing(output) {
        // a card clicked while editing shows what it can be set to
        if let (Button::Left | Button::Right, Some(key)) = (button, under) {
            widget_menu(output, key, at);
        }
        return;
    }
    match (button, under) {
        (Button::Left, Some(key)) => key.kind.open(),
        (Button::Right, Some(key)) => widget_menu(output, key, at),
        (Button::Right, None) => desktop_menu(output, at),
        _ => {}
    }
}
