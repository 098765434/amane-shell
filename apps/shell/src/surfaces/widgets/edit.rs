//! Edit Widgets — the widget gallery, on one monitor at a time. The
//! desktop's surface moves over the windows (the overlay layer) and takes
//! the keyboard; each card gets a
//! remove badge and, when it comes in more than one size, a resize handle;
//! and the gallery (`gallery.rs`) opens at the bottom of the screen, where a
//! widget is dragged from onto the desktop, or clicked to go in the first
//! free cells. A card dragged onto the gallery, or off the canvas, is
//! removed; Undo Remove puts it back.
//!
//! Gallery and canvas share the surface, as amane can't carry a drag from
//! one surface to another, and every drag is measured on the surface's
//! still layer, the one moving cards between cells out of edit mode. A
//! widget lifted from the gallery is on the canvas under a provisional key
//! until it's let go there, when it becomes a widget of its own.

use std::cell::RefCell;

use amane::{Point, Stack, Widget};

use super::kind::{WidgetKey, WidgetKind, WidgetSize};
use super::store::{self, Settings};
use super::{Geometry, changed, commit, current, gallery, radius, shown, with_desk};
use crate::chrome::edit_marks as marks;
use crate::chrome::grid_edit;
use crate::tokens::{sys, widget};
use emaki_kit::control_size::ControlSize;
use emaki_kit::grid::Layout;
use emaki_kit::grid::drag::{Drag, Origin, Session, follow};
use emaki_kit::grid::geometry::{Metrics, Slot};
use emaki_kit::grid::{Entry, GridKey};
use emaki_kit::layout::{boxed, frame};
use emaki_kit::menu::layer as context_menu;
use emaki_kit::motion::Pose;

/// The key a widget lifted from the gallery has until it lands.
pub(super) const PROVISIONAL: u32 = u32::MAX;

/// A press on a widget in the gallery: what, where it's drawn (and at
/// what scale), and where the pointer took it.
#[derive(Clone, Copy)]
struct Press {
    kind: WidgetKind,
    area: gallery::Area,
    scale: f32,
    from: Point,
}

/// One removal, to be put back: the monitor, and each widget it took off
/// with where it was and its settings. Only what was removed is kept, not
/// the whole layout, so putting it back leaves what changed since alone.
type Removal = (String, Vec<(Entry<WidgetKey>, Settings)>);

thread_local! {
    // the monitor being edited
    static EDITING: RefCell<Option<String>> = const { RefCell::new(None) };
    static PRESS: RefCell<Option<Press>> = const { RefCell::new(None) };
    // the removals Undo Remove puts back, the latest last
    static UNDO: RefCell<Vec<Removal>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn editing(output: &str) -> bool {
    EDITING.with_borrow(|editing| editing.as_deref() == Some(output))
}

pub(super) fn editing_any() -> Option<String> {
    EDITING.with_borrow(Clone::clone)
}

/// Leaves edit mode when the monitor being edited was unplugged, as it
/// has no surface left to say Done on.
pub(super) fn forget_unplugged() {
    let gone = EDITING.with_borrow(|editing| {
        editing
            .as_deref()
            .is_some_and(|output| !crate::chrome::per_output::connected(output))
    });
    if gone {
        leave_all();
    }
}

/// Edits `output`'s desktop (and leaves any other being edited).
pub(super) fn enter(output: &str) {
    context_menu::close();
    let before = EDITING.replace(Some(output.to_string()));
    if before.as_deref() != Some(output) {
        UNDO.with_borrow_mut(Vec::clear);
        super::keys::forget();
        gallery::open();
    }
    changed();
}

/// Done: every desktop back on the wallpaper.
pub(super) fn leave_all() {
    if let Some(output) = EDITING.take() {
        with_desk(&output, |desk| {
            desk.session = None;
            desk.press = None;
        });
    }
    PRESS.with_borrow_mut(|press| *press = None);
    UNDO.with_borrow_mut(Vec::clear);
    super::keys::forget();
    gallery::close();
    changed();
}

/// How big a card's resize handle is: none out of edit mode.
pub(super) fn handle_side(output: &str) -> f32 {
    if editing(output) {
        ControlSize::Medium.height()
    } else {
        0.0
    }
}

/// The pointer is at `point` on the surface with the button down while
/// editing: a press on a widget in the gallery takes hold of it once the
/// pointer has moved from it. Whether the gallery took the press.
pub(super) fn drag_at(output: &str, px: &Metrics, geometry: Geometry, point: Point) -> bool {
    if !editing(output) || with_desk(output, |desk| desk.session.is_some()) {
        return false;
    }
    let Some(press) = PRESS.with_borrow(|press| *press) else {
        return match gallery::tile_at(point) {
            Some((kind, area, scale)) => {
                PRESS.with_borrow_mut(|press| {
                    *press = Some(Press {
                        kind,
                        area,
                        scale,
                        from: point,
                    })
                });
                true
            }
            // a press on the gallery's sheet itself, not on a widget,
            // takes nothing on the canvas under it
            None => gallery::contains(point),
        };
    };
    if (point.x - press.from.x).hypot(point.y - press.from.y) <= widget::drag::threshold() {
        return true;
    }
    // lifted: on the canvas under a provisional key, at its real size,
    // held where the pointer took its preview
    PRESS.with_borrow_mut(|press| *press = None);
    let key = WidgetKey {
        id: PROVISIONAL,
        kind: press.kind,
    };
    let mut base = shown(output, geometry.canvas);
    base.insert(Entry {
        id: key,
        span: gallery::size_of(press.kind).span(),
        at: None,
    });
    let grab = Point {
        x: (press.from.x - press.area.x) / press.scale,
        y: (press.from.y - press.area.y) / press.scale,
    };
    let at = geometry.grid(point);
    let mut session = Session::new(
        base,
        Drag::Move {
            id: key,
            grab,
            at: follow(grab, at),
            place: None,
            origin: Origin::Window,
            pointer: at,
        },
    );
    session.motion(at, px, widget::drag::band());
    with_desk(output, |desk| desk.session = Some(session));
    true
}

/// The button came up: a press on a gallery widget that never moved is a
/// click, which adds it in the first free cells.
pub(super) fn released(output: &str) {
    if let Some(press) = PRESS.with_borrow_mut(Option::take) {
        add_from_gallery(output, press.kind);
    }
}

/// Adds a `kind` from the gallery in the first free cells, at the size
/// picked under it, or says there's no room.
pub(super) fn add_from_gallery(output: &str, kind: WidgetKind) {
    if !super::add(output, kind, gallery::size_of(kind), None) {
        gallery::note("No room for it on this screen: move or remove a widget first.");
    }
}

/// Whether `point` on the surface is over the gallery, where a card let go
/// is removed.
pub(super) fn over_gallery(output: &str, point: Point) -> bool {
    editing(output) && gallery::contains(point)
}

/// A widget from the gallery let go on the canvas, where `layout` (the
/// drag's preview) has it under its provisional key: it becomes a widget
/// of its own there.
pub(super) fn placed(output: &str, provisional: WidgetKey, layout: &Layout<WidgetKey>) {
    let Some(rect) = layout.rect_of(provisional) else {
        return;
    };
    let mut rest = layout.clone();
    rest.remove(provisional);
    store::with(|store| {
        store.add(
            output,
            &rest,
            provisional.kind,
            WidgetSize::of(rect.span),
            Some(rect.at),
        )
    });
    changed();
}

/// Takes `key` off `output`, to be put back by Undo Remove.
pub(super) fn remove(output: &str, key: WidgetKey) {
    if key.id == PROVISIONAL {
        return;
    }
    if let Some(layout) = current(output) {
        let removed = removal(&layout, |e| e.id == key);
        if !removed.is_empty() {
            UNDO.with_borrow_mut(|undo| undo.push((output.to_string(), removed)));
        }
    }
    super::remove(output, key);
}

/// The entries of `layout` that `take` picks, with their settings.
fn removal(
    layout: &Layout<WidgetKey>,
    take: impl Fn(&Entry<WidgetKey>) -> bool,
) -> Vec<(Entry<WidgetKey>, Settings)> {
    layout
        .entries
        .iter()
        .filter(|e| take(e))
        .map(|e| (*e, store::with(|store| store.settings(e.id.id))))
        .collect()
}

pub(super) fn can_undo() -> bool {
    UNDO.with_borrow(|undo| !undo.is_empty())
}

/// Puts the last removal back, settings and all: each widget where it was
/// when that's still free, else in the first free cells, else hidden.
/// What was placed or moved since stays where it is.
pub(super) fn undo() {
    if let Some((output, removed)) = UNDO.with_borrow_mut(Vec::pop)
        && let Some(layout) = current(&output)
    {
        commit(&output, &restored(layout, &removed));
        store::with(|store| {
            for (entry, settings) in removed {
                store.set_settings(entry.id.id, settings);
            }
        });
    }
    changed();
}

/// `layout` with `removed` back in it, each where it was if that's free.
fn restored(
    mut layout: Layout<WidgetKey>,
    removed: &[(Entry<WidgetKey>, Settings)],
) -> Layout<WidgetKey> {
    for (entry, _) in removed {
        let free = entry
            .rect()
            .filter(|rect| layout.canvas.fits(rect) && layout.occupancy().free(rect, None));
        layout.insert(Entry {
            at: free.map(|rect| rect.at),
            ..*entry
        });
        if free.is_none() {
            layout.show(entry.id);
        }
    }
    layout
}

/// Remove All: every card on the monitor being edited goes, to be put
/// back by Undo Remove. Those this screen has no room for (shown on a
/// bigger one) aren't on it, so they stay.
pub(super) fn reset() {
    let Some(output) = editing_any() else {
        return;
    };
    let Some(mut layout) = current(&output) else {
        return;
    };
    let removed = removal(&layout, |entry| entry.at.is_some());
    if removed.is_empty() {
        return;
    }
    for (entry, _) in &removed {
        layout.remove(entry.id);
    }
    UNDO.with_borrow_mut(|undo| undo.push((output.clone(), removed)));
    commit(&output, &layout);
}

// ───── the layers ───────────────────────────────────────────────────────────

/// Where the card in hand will land: an outline of the cells the preview
/// gives it. Always one layer, empty when nothing is held.
pub(super) fn drop_outline(
    output: &str,
    px: &Metrics,
    geometry: &Geometry,
    layout: &Layout<WidgetKey>,
) -> Box<dyn Widget> {
    let held = with_desk(output, |desk| match desk.session.as_ref().map(|s| s.drag) {
        Some(Drag::Move {
            id, place: Some(_), ..
        }) => Some(id),
        _ => None,
    });
    let Some(rect) = held.and_then(|id| layout.rect_of(id)) else {
        return grid_edit::no_outline();
    };
    let slot = px.slot(rect);
    grid_edit::outline(
        geometry.origin.0 + slot.x,
        geometry.origin.1 + slot.y,
        slot.width,
        slot.height,
        radius(),
    )
}

/// What edit mode lays over card `key` at `slot`, drawn at `pose` (on the
/// surface) so it follows the card: its remove badge, and its resize
/// handle when it comes in more than one size.
pub(super) fn dressing(output: &str, key: WidgetKey, slot: Slot, pose: Pose) -> Box<dyn Widget> {
    let side = ControlSize::Medium.height();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    if key.supported().len() > 1 {
        // a pull in flight: how far the hand has come, before the card
        // snaps to the sizes it comes in
        let pulled = grid_edit::pulling(
            with_desk(output, |desk| desk.session.as_ref().map(|s| s.drag)),
            key,
        );
        if let Some(pulled) = pulled {
            layers.push(grid_edit::pull_outline(
                slot,
                pulled,
                &super::metrics(),
                radius(),
            ));
        }
        layers.push(boxed(marks::bottom_right(
            slot.width,
            slot.height,
            marks::resize_handle(side),
        )));
    }
    // the card the keyboard has, ringed
    if super::keys::focused(output) == Some(key) {
        let ring = sys::space::gap::xs();
        layers.push(grid_edit::outline(
            -ring,
            -ring,
            slot.width + ring * 2.0,
            slot.height + ring * 2.0,
            radius() + ring,
        ));
    }
    if key.id != PROVISIONAL {
        let on = output.to_string();
        let badge = marks::remove_badge(
            format!("emaki.widgets.edit.{}.remove", key.id),
            side,
            move || remove(&on, key),
        );
        layers.push(boxed(marks::top_left(slot.width, slot.height, badge)));
    }
    boxed(
        frame(slot.width, slot.height)
            .translate(pose.x, pose.y)
            .scale(pose.scale)
            // the marks come in with the gallery
            .opacity((pose.opacity * gallery::opened()).clamp(0.0, 1.0))
            .child(Stack::new(layers)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use emaki_kit::grid::geometry::{Canvas, Place};

    fn key(id: u32) -> WidgetKey {
        WidgetKey {
            id,
            kind: WidgetKind::Cpu,
        }
    }

    fn at(id: u32, col: u8, row: u8) -> Entry<WidgetKey> {
        Entry {
            id: key(id),
            span: WidgetSize::Small.span(),
            at: Some(Place::new(col, row)),
        }
    }

    #[test]
    fn undo_puts_back_what_was_removed_and_keeps_what_came_since() {
        let mut layout = Layout::new(Canvas { cols: 6, rows: 2 });
        // A was removed from (0, 0); B was added since, at (2, 0)
        layout.insert(at(2, 2, 0));
        let removed = vec![(at(1, 0, 0), Settings::default())];
        let back = restored(layout.clone(), &removed);
        assert_eq!(back.rect_of(key(1)).map(|r| r.at), Some(Place::new(0, 0)));
        assert_eq!(back.rect_of(key(2)).map(|r| r.at), Some(Place::new(2, 0)));
        // B landed where A was: A goes to the first free cells instead
        let mut layout = Layout::new(Canvas { cols: 6, rows: 2 });
        layout.insert(at(2, 0, 0));
        let back = restored(layout, &removed);
        assert_eq!(back.rect_of(key(2)).map(|r| r.at), Some(Place::new(0, 0)));
        assert_eq!(back.rect_of(key(1)).map(|r| r.at), Some(Place::new(2, 0)));
    }
}
