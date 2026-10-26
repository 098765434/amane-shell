//! Edit Widgets — macOS's widget gallery, on one monitor at a time. The
//! desktop's surface moves over the windows (the overlay layer) and takes
//! the keyboard; the windows fall back behind a scrim; each card gets a
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

use amane::{Color, Cursor, Key, Point, Radius, Rectangle, Size, Stack, Widget, mark_all};

use super::kind::{WidgetKey, WidgetKind, WidgetSize};
use super::store::{self, Settings};
use super::{BAND, Geometry, RADIUS, THRESHOLD, commit, current, gallery, shown, with_desk};
use crate::components::buttons::{IconButton, Shape, Style};
use crate::components::context_menu;
use crate::components::control_size::ControlSize;
use crate::components::grid::Layout;
use crate::components::grid::drag::{Drag, Origin, Session, follow};
use crate::components::grid::geometry::{Metrics, Slot};
use crate::components::grid::{Entry, GridKey};
use crate::components::icons::{ICON_REMOVE, ICON_RESIZE};
use crate::components::layout::{boxed, icon};
use crate::components::material::{self, FillLevel, Level};
use crate::components::motion::Pose;
use crate::tokens::sys;

/// The key a widget lifted from the gallery has until it lands.
pub(super) const PROVISIONAL: u32 = u32::MAX;

// TODO(token): `widget.edit.scrim` once Penpot has it (manual §1.1).
/// What the windows fall back behind while the desktop is edited.
const SCRIM: Color = Color::rgba(0, 0, 0, 72);

/// A press on a widget in the gallery: what, where it's drawn (and at
/// what scale), and where the pointer took it.
#[derive(Clone, Copy)]
struct Press {
    kind: WidgetKind,
    area: gallery::Area,
    scale: f32,
    from: Point,
}

/// The layout before a removal, its monitor, and the removed widgets'
/// settings, to put them back.
type Undo = (String, Layout<WidgetKey>, Vec<(u32, Settings)>);

thread_local! {
    // the monitor being edited
    static EDITING: RefCell<Option<String>> = const { RefCell::new(None) };
    static PRESS: RefCell<Option<Press>> = const { RefCell::new(None) };
    static UNDO: RefCell<Option<Undo>> = const { RefCell::new(None) };
}

pub(super) fn editing(output: &str) -> bool {
    EDITING.with_borrow(|editing| editing.as_deref() == Some(output))
}

pub(super) fn editing_any() -> Option<String> {
    EDITING.with_borrow(Clone::clone)
}

/// Edits `output`'s desktop (and leaves any other being edited).
pub(super) fn enter(output: &str) {
    context_menu::close();
    let before = EDITING.replace(Some(output.to_string()));
    if before.as_deref() != Some(output) {
        UNDO.with_borrow_mut(|undo| *undo = None);
        gallery::open();
    }
    mark_all();
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
    UNDO.with_borrow_mut(|undo| *undo = None);
    gallery::close();
    mark_all();
}

/// A key while editing: Escape lets go of a drag, else clears the search,
/// else is Done; the rest type into the gallery's search.
pub(super) fn key(output: &str, key: Key) {
    if matches!(key, Key::Escape) {
        let dropped = with_desk(output, |desk| desk.session.take().is_some());
        if !dropped && !gallery::clear_query() {
            leave_all();
        }
        mark_all();
    } else {
        gallery::key(key);
    }
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
    if (point.x - press.from.x).hypot(point.y - press.from.y) <= THRESHOLD {
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
        span: press.kind.default_size().span(),
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
    session.motion(at, px, BAND);
    with_desk(output, |desk| desk.session = Some(session));
    true
}

/// The button came up: a press on a gallery widget that never moved is a
/// click, which adds it in the first free cells.
pub(super) fn released(output: &str) {
    if let Some(press) = PRESS.with_borrow_mut(Option::take) {
        super::add(output, press.kind, press.kind.default_size(), None);
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
    mark_all();
}

/// Takes `key` off `output`, to be put back by Undo Remove.
pub(super) fn remove(output: &str, key: WidgetKey) {
    if key.id == PROVISIONAL {
        return;
    }
    if let Some(layout) = current(output) {
        let settings = store::with(|store| store.settings(key.id));
        UNDO.with_borrow_mut(|undo| {
            *undo = Some((output.to_string(), layout, vec![(key.id, settings)]))
        });
    }
    super::remove(output, key);
}

pub(super) fn can_undo() -> bool {
    UNDO.with_borrow(Option::is_some)
}

/// Puts the last widget removed back where it was, settings and all.
pub(super) fn undo() {
    if let Some((output, layout, settings)) = UNDO.with_borrow_mut(Option::take) {
        commit(&output, &layout);
        store::with(|store| {
            for (id, settings) in settings {
                store.set_settings(id, settings);
            }
        });
    }
    mark_all();
}

/// An empty desktop on the monitor being edited.
pub(super) fn reset() {
    let Some(output) = editing_any() else {
        return;
    };
    if let Some(layout) = current(&output) {
        let settings: Vec<(u32, Settings)> = layout
            .entries
            .iter()
            .map(|e| (e.id.id, store::with(|store| store.settings(e.id.id))))
            .collect();
        UNDO.with_borrow_mut(|undo| *undo = Some((output.clone(), layout, settings)));
    }
    store::with(|store| store.clear(&output));
    mark_all();
}

// ───── the layers ───────────────────────────────────────────────────────────

/// The scrim the windows fall back behind, over the whole surface.
pub(super) fn ghost_grid(
    _px: &Metrics,
    _geometry: &Geometry,
    _layout: &Layout<WidgetKey>,
) -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(amane::Parent)
            .height(amane::Parent)
            .fill(SCRIM),
    )
}

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
        return boxed(
            Rectangle::new()
                .width(Size::Fixed(0.0))
                .height(Size::Fixed(0.0)),
        );
    };
    let slot = px.slot(rect);
    boxed(
        Rectangle::new()
            .width(Size::Fixed(slot.width))
            .height(Size::Fixed(slot.height))
            .radius(RADIUS)
            .fill(Color::TRANSPARENT)
            .border(sys::border::hairline(), sys::color::accent::bg())
            .translate(geometry.origin.0 + slot.x, geometry.origin.1 + slot.y),
    )
}

/// What edit mode lays over card `key` at `slot`, drawn at `pose` (on the
/// surface) so it follows the card: its remove badge, and its resize
/// handle when it comes in more than one size.
pub(super) fn dressing(output: &str, key: WidgetKey, slot: Slot, pose: Pose) -> Box<dyn Widget> {
    let side = ControlSize::Medium.height();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    if key.supported().len() > 1 {
        let handle = Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .radius(Radius::Full)
            .fill(material::fill_role(FillLevel::Secondary))
            .align_child(amane::Center, amane::Center)
            .cursor(Cursor::ResizeBottomRight)
            .child(icon(
                ICON_RESIZE,
                sys::size::icon::s(),
                material::label(Level::Primary),
            ));
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(slot.width))
                .height(Size::Fixed(slot.height))
                .fill(Color::TRANSPARENT)
                .align_child(amane::End, amane::End)
                .child(handle),
        ));
    }
    if key.id != PROVISIONAL {
        let on = output.to_string();
        let badge = IconButton::new(format!("amane.widgets.edit.{}.remove", key.id), ICON_REMOVE)
            .style(Style::Tonal)
            .shape(Shape::Round)
            .size(sys::size::icon::s())
            .height(side)
            .width(side)
            .color(material::label(Level::Primary))
            .on_press(move || remove(&on, key))
            .build();
        layers.push(boxed(
            Rectangle::new()
                .width(Size::Fixed(slot.width))
                .height(Size::Fixed(slot.height))
                .fill(Color::TRANSPARENT)
                .align_child(amane::Start, amane::Start)
                .child(badge),
        ));
    }
    boxed(
        Rectangle::new()
            .width(Size::Fixed(slot.width))
            .height(Size::Fixed(slot.height))
            .fill(Color::TRANSPARENT)
            .translate(pose.x, pose.y)
            .scale(pose.scale)
            .opacity(pose.opacity.clamp(0.0, 1.0))
            .child(Stack::new(layers)),
    )
}
