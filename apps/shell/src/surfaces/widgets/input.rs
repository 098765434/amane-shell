//! The pointer on a desktop: pressing, dragging and letting go of a card, and clicks.

use super::*;

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
    if edit::drag_at(output, &px, geometry, point) {
        return;
    }
    let begin = with_desk(output, |desk| {
        if let Some(session) = &mut desk.session {
            session.motion(at, &px, BAND);
            return None;
        }
        match desk.press {
            None => {
                let handle = edit::handle_side(output);
                desk.press = match hit(&desk.slots, at, handle) {
                    Some(Hit::Body(key, slot)) => Some((at, key, slot)),
                    Some(Hit::Handle(key, slot)) if edit::editing(output) => {
                        return Some(Drag::Resize {
                            id: key,
                            grab: at,
                            base: slot,
                            pulled: (slot.width, slot.height),
                            span: None,
                        });
                    }
                    Some(Hit::Handle(key, slot)) => Some((at, key, slot)),
                    None => None,
                };
                None
            }
            Some((from, key, slot)) => ((at.x - from.x).hypot(at.y - from.y) > THRESHOLD)
                .then_some(Drag::Move {
                    id: key,
                    grab: Point {
                        x: from.x - slot.x,
                        y: from.y - slot.y,
                    },
                    at: (slot.x, slot.y),
                    place: None,
                    origin: Origin::Canvas,
                    pointer: at,
                }),
        }
    });
    if let Some(drag) = begin {
        let base = shown(output, geometry.canvas);
        with_desk(output, |desk| {
            desk.press = None;
            let mut session = Session::new(base, drag);
            session.motion(at, &px, BAND);
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
        let session = desk.session.take();
        desk.dragged |= session.is_some();
        (session, desk.pointer)
    });
    let on_gallery = edit::over_gallery(output, pointer);
    if let Some(session) = session {
        match session.drag {
            Drag::Move {
                id,
                place,
                at,
                origin,
                ..
            } => {
                if place.is_none() || on_gallery {
                    if editing && origin == Origin::Canvas {
                        edit::remove(output, id);
                    }
                } else if let Some(layout) = session.preview {
                    motion::jump(
                        &scope(output),
                        u64::from(id.id),
                        Pose {
                            scale: DRAG_SCALE,
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
            Drag::Resize { .. } => {
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
        let under = desk
            .slots
            .iter()
            .find(|(_, s)| {
                at.x >= s.x && at.y >= s.y && at.x < s.x + s.width && at.y < s.y + s.height
            })
            .map(|(k, _)| *k);
        (std::mem::take(&mut desk.dragged), desk.pointer, under)
    });
    if dragged || edit::editing(output) {
        return;
    }
    let at = (pointer.x as i32, pointer.y as i32);
    match (button, under) {
        (Button::Left, Some(key)) => key.kind.open(),
        (Button::Right, Some(key)) => widget_menu(output, key, at),
        (Button::Right, None) => desktop_menu(output, at),
        _ => {}
    }
}
