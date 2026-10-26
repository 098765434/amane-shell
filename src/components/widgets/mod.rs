//! Desktop widgets — macOS's widgets on the wallpaper, for watching rather
//! than switching: the batteries, the processor, memory, temperature, the
//! network and the disks. Quick Settings' tiles are the controls; these
//! are the readings.
//!
//! One clear surface per monitor on the background layer, under the
//! windows, holding frosted cards laid out on Quick Settings' grid (cells
//! of 74 px, 16 px between) across the whole screen. A card is 2 × 2
//! (Small), 4 × 2 (Medium), 4 × 4 (Large) or 8 × 4 (Extra Large) cells,
//! and the same kind can sit on the desktop more than once, each with its
//! own settings. Each monitor keeps its own layout (`store.rs`).
//!
//! As on macOS, a card is dragged to another place at any time, the others
//! making way as they do in Quick Settings; a click opens what it watches
//! (the battery's page, a system monitor); a right click opens its menu
//! (its sizes, what it shows, Edit Widgets…, Remove Widget), and one on the
//! desktop itself, Edit Widgets…, which opens the gallery (`edit.rs`). The
//! cards fade back while windows are open on the monitor's workspace.
//!
//! amane measures a drag against the hit areas of the last frame and finds
//! the dragged target by its place among them, so every drag is measured on
//! one still layer over the whole surface that comes first, and the cards
//! take no input of their own. See `docs/widgets.md`.

mod catalog;
mod edit;
mod gallery;
mod kind;
mod store;

use std::cell::RefCell;
use std::collections::HashMap;

use amane::{
    Animation, BlurShape, Button, Color, Cursor, Full, InputArea, Keyboard, Layer, LayerWindow,
    Monitor, Point, Rectangle, Service, Shadow, Size, Stack, Widget, Workspaces, Zone, mark_all,
};

use crate::components::context_menu::{self, ContextMenu, Icon, MenuItem};
use crate::components::glass::{self, Rim};
use crate::components::glass_region;
use crate::components::grid::Layout;
use crate::components::grid::drag::{Drag, Hit, Origin, Session, hit};
use crate::components::grid::geometry::{Canvas, Metrics, Place, Rect, Slot, Span};
use crate::components::icons::ICON_CHECK;
use crate::components::layout::{blank, boxed};
use crate::components::material::{self, Material};
use crate::components::motion::{self, Arrive, Item, Pose};
use crate::services::system_stats::SystemStats;
use crate::tokens::sys;
use amane_kit::settings::{WidgetStyle, desktop};

use kind::{WidgetKey, WidgetKind, WidgetSize};
use store::Settings;

/// Every monitor's widget surface is `amane.widgets.<output>`; the niri
/// `layer-rule` matches the prefix. Each has its own name, so a card's menu
/// hangs under the surface on its own monitor.
pub const WIDGETS_NS: &str = "amane.widgets.";

// TODO(token): `widget.grid.cell`, `widget.grid.gap` (Quick Settings'
// `quick.grid.cell` and `.gap`, which code outside it can't read),
// `widget.canvas.margin`, `widget.card.radius`, `widget.shadow`,
// `widget.dim.opacity` and `widget.drag.scale` once Penpot has them
// (manual §1.1).
const CELL: f32 = 74.0;
const GAP: f32 = 16.0;
/// The room kept clear round the canvas, at the screen's edges.
const MARGIN: f32 = 24.0;
/// A card's corners: macOS's widgets'.
const RADIUS: f32 = 22.0;
/// How far a card fades back while windows are open.
const DIM: f32 = 0.45;
/// How much bigger the card in hand is drawn.
const DRAG_SCALE: f32 = 1.04;
/// How far a card goes past the middle of the next cell before it changes
/// cell, in cells (Quick Settings' dead band).
const BAND: f32 = 0.1;
/// How far the pointer moves with the button down before a press is a drag.
const THRESHOLD: f32 = 6.0;

fn metrics() -> Metrics {
    Metrics::new(CELL, GAP)
}

/// Where a monitor's canvas is: as many whole cells as fit inside the
/// margin, above the bar, centred in what's left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub canvas: Canvas,
    pub origin: (f32, f32),
    pub width: f32,
    pub height: f32,
}

impl Geometry {
    fn of(width: f32, height: f32) -> Self {
        let px = metrics();
        let area_w = width - 2.0 * MARGIN;
        let area_h = height - sys::size::panel::height() - 2.0 * MARGIN;
        let canvas = Canvas::fit(&px, area_w, area_h);
        let (x, y) = px.centred_origin(&canvas, area_w, area_h);
        Self {
            canvas,
            origin: (MARGIN + x, MARGIN + y),
            width,
            height,
        }
    }

    /// `point` on the surface, in the canvas's pixels.
    fn grid(&self, point: Point) -> Point {
        Point {
            x: point.x - self.origin.0,
            y: point.y - self.origin.1,
        }
    }
}

/// One monitor's desktop, kept across frames.
struct Desk {
    geometry: Option<Geometry>,
    /// A press on a card not yet far enough to be a drag: where, on what.
    press: Option<(Point, WidgetKey, Slot)>,
    /// The drag in flight: the layout it began from and its preview.
    session: Option<Session<WidgetKey>>,
    /// A drag just ended, so the click that follows isn't one.
    dragged: bool,
    /// Where each card is this frame, in the canvas's pixels.
    slots: Vec<(WidgetKey, Slot)>,
    /// The pointer on the surface.
    pointer: Point,
    /// 0 … 1: the cards fading back while windows are open.
    dim: Animation,
}

impl Desk {
    fn new() -> Self {
        Self {
            geometry: None,
            press: None,
            session: None,
            dragged: false,
            slots: Vec::new(),
            pointer: Point { x: 0.0, y: 0.0 },
            dim: motion::fade(0.0, motion::FADE),
        }
    }
}

thread_local! {
    static DESKS: RefCell<HashMap<String, Desk>> = RefCell::new(HashMap::new());
    static NAMES: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
}

fn with_desk<R>(output: &str, f: impl FnOnce(&mut Desk) -> R) -> R {
    DESKS.with_borrow_mut(|desks| f(desks.entry(output.to_string()).or_insert_with(Desk::new)))
}

/// The namespace `amane.widgets.<name>`: a monitor's desktop surface (its
/// name) or its edit surface (`edit.<name>`). Leaked once per monitor ever
/// seen (a handful), as amane takes a `&'static str`.
fn namespace(output: &str) -> &'static str {
    NAMES.with_borrow_mut(|names| {
        *names
            .entry(output.to_string())
            .or_insert_with(|| Box::leak(format!("{WIDGETS_NS}{output}").into_boxed_str()))
    })
}

fn glass_id(output: &str, key: WidgetKey) -> u32 {
    glass_region::shape_id(&format!("{WIDGETS_NS}{output}.{}", key.id))
}

fn scope(output: &str) -> String {
    format!("{WIDGETS_NS}{output}")
}

/// `output`'s layout as it shows: the drag's preview while one is in
/// flight, else what's kept, on the canvas it has now.
fn shown(output: &str, canvas: Canvas) -> Layout<WidgetKey> {
    with_desk(output, |desk| {
        desk.session.as_ref().and_then(|s| s.preview.clone())
    })
    .unwrap_or_else(|| store::with(|store| store.layout_for(output, canvas)))
}

/// The layout as it shows on `output` now, from the last frame's canvas.
fn current(output: &str) -> Option<Layout<WidgetKey>> {
    let canvas = with_desk(output, |desk| desk.geometry.map(|g| g.canvas))?;
    Some(shown(output, canvas))
}

/// Keeps `layout` as `output`'s.
fn commit(output: &str, layout: &Layout<WidgetKey>) {
    store::with(|store| store.commit(output, layout));
    mark_all();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The widgets on `monitor`, on the wallpaper under the windows.
/// Registered with `App::window_per_monitor`. While the monitor's desktop
/// is edited its cards are drawn by [`edit_surface`] instead.
pub fn surface(monitor: &Monitor) -> LayerWindow {
    desk_window(monitor, false)
}

/// The monitor's desktop while it's edited: the cards and the gallery
/// over the windows, as macOS clears the desktop to edit it; hidden the
/// rest of the time. A surface of its own on the overlay layer rather than
/// the desktop's moved there: niri doesn't frost a surface moved off the
/// background layer.
pub fn edit_surface(monitor: &Monitor) -> LayerWindow {
    desk_window(monitor, true)
}

fn desk_window(monitor: &Monitor, edit_window: bool) -> LayerWindow {
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
    if !showing && edit::editing(&output) {
        edit::leave_all();
    }
    let editing = edit::editing(&output);
    if !showing || (edit_window && !editing) {
        // hidden, taking nothing
        return LayerWindow::new()
            .width(Full)
            .height(Full)
            .layer(layer)
            .space(Zone::Ignore)
            .keyboard(Keyboard::None)
            .namespace(name)
            .visible(false)
            .click_through()
            .child(Stack::new(Vec::new()));
    }
    with_desk(&output, |desk| desk.geometry = Some(geometry));

    let full = InputArea {
        x: 0,
        y: 0,
        width: monitor.width as i32,
        height: monitor.height as i32,
    };
    let window = LayerWindow::new()
        .width(Full)
        .height(Full)
        .vibrant(material::vibrant())
        .layer(layer)
        .space(Zone::Ignore)
        .namespace(name)
        .visible(true);
    if editing && !edit_window {
        // the edit surface has the cards for now
        return window
            .keyboard(Keyboard::None)
            .click_through()
            .blur_shapes(Vec::new())
            .child(Stack::new(Vec::new()));
    }

    let layout = shown(&output, geometry.canvas);
    let dim = dimming(&output, editing);
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
        window.on_key(move |key| edit::key(&on, key))
    } else {
        window
    }
}

/// Whether the cards fade back now, as the widget style says: by default
/// while a window is open on the monitor's workspace, never while editing.
/// 0 … 1, faded.
fn dimming(output: &str, editing: bool) -> f32 {
    let style = desktop().widget_style;
    let busy = !editing
        && match style {
            WidgetStyle::Full => false,
            WidgetStyle::Dim => true,
            WidgetStyle::Automatic => Workspaces::read()
                .list()
                .iter()
                .any(|ws| ws.output() == Some(output) && ws.active() && ws.windows() > 0),
        };
    with_desk(output, |desk| {
        desk.dim.to(if busy { 1.0 } else { 0.0 });
        desk.dim.value().clamp(0.0, 1.0)
    })
}

struct View {
    root: Stack,
    frost: Vec<BlurShape>,
}

/// The soft shadow under every card (Quick Settings' `quick.shadow`).
fn shadow() -> Shadow {
    Shadow::drop(Color::rgba(0, 0, 0, 0x29))
        .blur(24.0)
        .offset(0.0, 8.0)
}

/// A card's glass, `width` by `height`, holding `content`.
fn card(width: f32, height: f32, content: Box<dyn Widget>) -> Rectangle {
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(RADIUS)
        .fill(Material::Control.fill())
        .shadow(shadow())
        .clip()
        .child(Stack::new(vec![content]))
}

fn view(
    output: &str,
    geometry: &Geometry,
    layout: &Layout<WidgetKey>,
    dim: f32,
    editing: bool,
) -> View {
    let px = metrics();
    let (ox, oy) = geometry.origin;
    let placed: Vec<(WidgetKey, Rect)> = layout.placed().collect();
    let slots: Vec<(WidgetKey, Slot)> = placed.iter().map(|(k, r)| (*k, px.slot(*r))).collect();
    with_desk(output, |desk| desk.slots = slots.clone());
    let held = held(output);

    let items: Vec<Item<(WidgetKey, Rect, Slot)>> = placed
        .iter()
        .zip(&slots)
        .map(|((key, rect), (_, slot))| {
            let pose = Pose::at(slot.y).moved(slot.x);
            let away = Pose {
                scale: motion::scale(motion::ENTER_SCALE),
                ..pose.faded()
            };
            Item {
                key: u64::from(key.id),
                value: (*key, *rect, *slot),
                height: slot.height,
                pose,
                enter: away,
                exit: away,
            }
        })
        .collect();
    let mut shown = motion::animate(&scope(output), items, Arrive::Rest);
    // the card in hand draws over the rest
    shown.sort_by_key(|item| held.map(|(k, _)| k) == Some(item.value.0));

    let mut layers: Vec<Box<dyn Widget>> =
        vec![boxed(still_layer(output, *geometry, held.is_some()))];
    if editing {
        layers.push(edit::drop_outline(output, &px, geometry, layout));
    }
    let mut frost = Vec::new();
    let mut rims: Vec<Rim> = Vec::new();
    let mut dressings = Vec::new();
    // the card in hand, over everything, the gallery too
    let mut over: Vec<Box<dyn Widget>> = Vec::new();
    let fade = 1.0 - DIM * dim;
    for item in shown {
        let (key, rect, slot) = item.value;
        let mut pose = item.pose;
        if let Some((held_key, at)) = held
            && held_key == key
            && !item.leaving
        {
            pose = Pose {
                scale: DRAG_SCALE,
                ..Pose::at(at.1).moved(at.0)
            };
        }
        let opacity = (pose.opacity * fade).clamp(0.0, 1.0);
        let size = WidgetSize::of(rect.span);
        let settings = store::with(|store| store.settings(key.id));
        let glass = glass_id(output, key);
        let built = Material::Control.adapted(glass, || {
            card(
                slot.width,
                slot.height,
                catalog::build(key, size, slot.width, slot.height, &settings),
            )
        });
        let x = ox + pose.x;
        let y = oy + pose.y;
        let drawn = Rim {
            x: x + slot.width * (1.0 - pose.scale) / 2.0,
            y: y + slot.height * (1.0 - pose.scale) / 2.0,
            width: slot.width * pose.scale,
            height: slot.height * pose.scale,
            radius: RADIUS * pose.scale,
            opacity,
        };
        frost.push(
            Material::Control
                .adapted(glass, || {
                    Material::Control.shape(
                        drawn.x,
                        drawn.y,
                        drawn.width,
                        drawn.height,
                        drawn.radius,
                        if amane::blur_fades() {
                            opacity
                        } else {
                            pose.opacity
                        },
                    )
                })
                .id(glass),
        );
        let in_hand = held.is_some_and(|(k, _)| k == key) && !item.leaving;
        let drawn_card = boxed(
            Rectangle::new()
                .width(Size::Fixed(slot.width))
                .height(Size::Fixed(slot.height))
                .fill(Color::TRANSPARENT)
                .translate(x, y)
                .scale(pose.scale)
                .opacity(opacity)
                .child(built),
        );
        if in_hand {
            over.push(drawn_card);
        } else {
            rims.push(drawn);
            layers.push(drawn_card);
        }
        if editing && !item.leaving {
            dressings.push(edit::dressing(output, key, slot, Pose { x, y, ..pose }));
        }
    }
    if !Material::Control.compositor_draws() {
        layers.extend(glass::rim_layers(&rims).into_iter().map(boxed));
    }
    layers.extend(dressings);
    if editing {
        let (gallery, gallery_frost) = gallery::sheet(output, geometry);
        layers.push(gallery);
        frost.extend(gallery_frost);
        if let Some((ghost, ghost_frost)) = gallery::ghost(output, geometry) {
            layers.push(ghost);
            frost.extend(ghost_frost);
        }
    }
    layers.extend(over);

    View {
        root: Stack::new(layers),
        frost,
    }
}

// ───── the pointer ───────────────────────────────────────────────────────────

/// The card in hand on `output`, and where its top-left is in the
/// canvas's pixels.
fn held(output: &str) -> Option<(WidgetKey, (f32, f32))> {
    with_desk(output, |desk| match desk.session.as_ref().map(|s| s.drag) {
        Some(Drag::Move { id, at, .. }) => Some((id, at)),
        _ => None,
    })
}

/// The drag in flight on `output`.
fn drag(output: &str) -> Option<Drag<WidgetKey>> {
    with_desk(output, |desk| desk.session.as_ref().map(|s| s.drag))
}

/// The still layer over the whole surface that every press, drag and
/// click lands on (the cards take nothing): it never moves, and comes
/// first, so the target amane keeps for a drag in flight stays put.
fn still_layer(output: &str, geometry: Geometry, holding: bool) -> Rectangle {
    let (move_on, drag_on, release_on, click_on) = (
        output.to_string(),
        output.to_string(),
        output.to_string(),
        output.to_string(),
    );
    let layer = blank()
        .on_move(move |point: Point| with_desk(&move_on, |desk| desk.pointer = point))
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
fn pressed(output: &str, geometry: Geometry, point: Point) {
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
    mark_all();
}

/// The button came up: what the drag previewed is kept. A card let go off
/// the canvas goes back where it was, or, while editing, is removed.
fn released(output: &str) {
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
    mark_all();
}

fn clicked(output: &str, geometry: Geometry, button: Button) {
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

// ───── menus ─────────────────────────────────────────────────────────────────

const WIDGET_MENU_KEY: &str = "amane.widgets.widget-menu";
const DESKTOP_MENU_KEY: &str = "amane.widgets.desktop-menu";

fn checked(item: MenuItem, on: bool) -> MenuItem {
    if on {
        item.icon(Icon::glyph(ICON_CHECK.to_string()))
    } else {
        item
    }
}

/// A card's menu: the sizes it comes in, what it shows, Edit Widgets… and
/// Remove Widget.
fn widget_menu(output: &str, key: WidgetKey, at: (i32, i32)) {
    let Some(layout) = current(output) else {
        return;
    };
    let span = layout
        .span_of(key)
        .unwrap_or(key.kind.default_size().span());
    let mut menu = ContextMenu::new();
    for size in key.kind.sizes() {
        let item = checked(MenuItem::label(size.title()), size.span() == span);
        let fits = size.span() == span || layout.resized(key, size.span()).is_some();
        let on = output.to_string();
        menu = menu.item(if fits {
            item.on_select(move || resize(&on, key, size.span()))
        } else {
            item.disabled()
        });
    }
    if let Some(show) = choices(output, key) {
        menu = menu.separator().item(MenuItem::label("Show").submenu(show));
    }
    let (on_edit, on_remove) = (output.to_string(), output.to_string());
    menu = menu
        .separator()
        .item(MenuItem::label("Edit Widgets…").on_select(move || edit::enter(&on_edit)))
        .separator()
        .item(
            MenuItem::label("Remove Widget")
                .destructive()
                .on_select(move || remove(&on_remove, key)),
        );
    context_menu::show_under(WIDGET_MENU_KEY, menu, namespace(output), at);
}

/// What a card can be set to show, and which it shows, as a submenu:
/// none for a kind with nothing to pick.
fn choices(output: &str, key: WidgetKey) -> Option<ContextMenu> {
    let settings = store::with(|store| store.settings(key.id));
    type Field = fn(&mut Settings) -> &mut Option<String>;
    let (options, field, automatic): (Vec<(String, String)>, Field, &str) = match key.kind {
        WidgetKind::Battery => (
            catalog::devices()
                .into_iter()
                .map(|d| (d.id, d.name))
                .collect(),
            |s| &mut s.device,
            "First Device",
        ),
        WidgetKind::Network => (
            SystemStats::read()
                .interfaces()
                .into_iter()
                .map(|n| (n.to_string(), n.to_string()))
                .collect(),
            |s| &mut s.interface,
            "Default Route",
        ),
        WidgetKind::Disk => (
            SystemStats::read()
                .disks
                .iter()
                .map(|d| (d.mount.clone(), d.mount.clone()))
                .collect(),
            |s| &mut s.mount,
            "Main Disk",
        ),
        WidgetKind::Temperature => (
            SystemStats::read()
                .sensors
                .iter()
                .map(|s| (s.label.clone(), s.label.clone()))
                .collect(),
            |s| &mut s.sensor,
            "Processor",
        ),
        _ => return None,
    };
    let mut current = settings.clone();
    let now = field(&mut current).clone();
    let pick = |value: Option<String>| {
        let output = output.to_string();
        let mut next = settings.clone();
        *field(&mut next) = value;
        move || {
            store::with(|store| store.set_settings(key.id, next.clone()));
            let _ = &output;
            mark_all();
        }
    };
    let mut menu = ContextMenu::new()
        .item(checked(MenuItem::label(automatic), now.is_none()).on_select(pick(None)));
    if !options.is_empty() {
        menu = menu.separator();
    }
    for (id, name) in options {
        let on = now.as_deref() == Some(id.as_str());
        menu = menu.item(checked(MenuItem::label(name), on).on_select(pick(Some(id))));
    }
    Some(menu)
}

/// The desktop's menu: Edit Widgets….
fn desktop_menu(output: &str, at: (i32, i32)) {
    let on = output.to_string();
    let menu = ContextMenu::new()
        .item(MenuItem::label("Edit Widgets…").on_select(move || edit::enter(&on)));
    context_menu::show_under(DESKTOP_MENU_KEY, menu, namespace(output), at);
}

fn resize(output: &str, key: WidgetKey, span: Span) {
    if let Some(next) = current(output).and_then(|layout| layout.resized(key, span)) {
        commit(output, &next);
    }
}

fn remove(output: &str, key: WidgetKey) {
    if let Some(mut layout) = current(output) {
        layout.remove(key);
        commit(output, &layout);
    }
}

/// Adds a `kind` widget to `output` at `size`, in the first free cells or
/// at `at`; whether there was room.
fn add(output: &str, kind: WidgetKind, size: WidgetSize, at: Option<Place>) -> bool {
    let Some(layout) = current(output) else {
        return false;
    };
    let added = store::with(|store| store.add(output, &layout, kind, size, at)).is_some();
    mark_all();
    added
}

// ───── IPC ───────────────────────────────────────────────────────────────────

/// The monitor a command means: the one named, else the focused
/// workspace's, else the first with widgets.
fn output_for(name: Option<&String>) -> Option<String> {
    if let Some(name) = name {
        return Some(name.clone());
    }
    let focused = Workspaces::read()
        .list()
        .iter()
        .find(|ws| ws.focused())
        .and_then(|ws| ws.output().map(String::from));
    focused.or_else(|| DESKS.with_borrow(|desks| desks.keys().next().cloned()))
}

/// `widgets list`, `add <kind> [size] [output]`, `remove <id>`,
/// `reset-layout [output]`, `edit [output]` and `done`, one argument a line:
/// `printf 'widgets\nadd\ncpu\nmedium\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock`.
pub fn ipc(args: &[String]) -> String {
    let arg = |i: usize| args.get(i);
    match arg(0).map(String::as_str) {
        Some("list") => store::with(|store| {
            store
                .all()
                .map(|(output, entry)| {
                    let at = entry.at.map_or_else(
                        || String::from("hidden"),
                        |at| format!("{},{}", at.col, at.row),
                    );
                    format!(
                        "{output} {} {} {} {at}",
                        entry.id.id,
                        entry.id.kind.name(),
                        WidgetSize::of(entry.span).name()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }),
        Some("add") => {
            let Some(kind) = arg(1).and_then(|k| WidgetKind::named(k)) else {
                let kinds: Vec<&str> = WidgetKind::ALL.iter().map(|k| k.name()).collect();
                return format!("error: widgets add <{}> [size] [output]", kinds.join("|"));
            };
            let (size, output) = match arg(2).and_then(|s| WidgetSize::named(s)) {
                Some(size) => (size, arg(3)),
                None => (kind.default_size(), arg(2)),
            };
            match output_for(output) {
                Some(output) if add(&output, kind, size, None) => String::from("ok"),
                Some(_) => String::from("error: no room for it"),
                None => String::from("error: no monitor"),
            }
        }
        Some("remove") => {
            let Some(id) = arg(1).and_then(|id| id.parse::<u32>().ok()) else {
                return String::from("error: widgets remove <id>");
            };
            let found = store::with(|store| {
                store
                    .all()
                    .find(|(_, entry)| entry.id.id == id)
                    .map(|(output, entry)| (output.to_string(), entry.id))
            });
            match found {
                Some((output, key)) => {
                    remove(&output, key);
                    String::from("ok")
                }
                None => format!("error: no widget {id}"),
            }
        }
        Some("reset-layout") => match output_for(arg(1)) {
            Some(output) => {
                store::with(|store| store.clear(&output));
                mark_all();
                String::from("ok")
            }
            None => String::from("error: no monitor"),
        },
        Some("edit") => match output_for(arg(1)) {
            Some(output) => {
                edit::enter(&output);
                String::from("ok")
            }
            None => String::from("error: no monitor"),
        },
        Some("done") => {
            edit::leave_all();
            String::from("ok")
        }
        _ => String::from(
            "usage: widgets list | add <kind> [size] [output] | remove <id> | \
             reset-layout [output] | edit [output] | done",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_is_whole_cells_inside_the_margin_and_over_the_bar() {
        let geometry = Geometry::of(1920.0, 1080.0);
        let px = metrics();
        let bar = sys::size::panel::height();
        assert!(px.extent(geometry.canvas.cols) <= 1920.0 - 2.0 * MARGIN);
        assert!(px.extent(geometry.canvas.cols + 1) > 1920.0 - 2.0 * MARGIN);
        assert!(
            geometry.origin.1 + px.extent(geometry.canvas.rows) <= 1080.0 - bar - MARGIN + 0.01
        );
        // centred: as much room left as right
        let right = 1920.0 - geometry.origin.0 - px.extent(geometry.canvas.cols);
        assert!((right - geometry.origin.0).abs() < 0.01);
    }
}
