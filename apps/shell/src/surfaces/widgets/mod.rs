//! Desktop widgets — cards on the wallpaper, for watching rather
//! than switching: the batteries, the processor, memory, temperature, the
//! network and the disks. Quick Settings' tiles are the controls; these
//! are the readings.
//!
//! One clear surface per monitor on the background layer, under the
//! windows, holding frosted cards laid out on Quick Settings' grid (cells
//! of 74 px, 16 px between) across the whole screen; the surface itself is
//! only the grid and room for the cards' shadows. A card is 2 × 2
//! (Small), 4 × 2 (Medium), 4 × 4 (Large) or 8 × 4 (Extra Large) cells,
//! and the same kind can sit on the desktop more than once, each with its
//! own settings. Each monitor keeps its own layout (`store.rs`).
//!
//! A card is dragged to another place at any time, the others
//! making way as they do in Quick Settings; a click opens what it watches
//! (the battery's page, a system monitor); a right click opens its menu
//! (its sizes, what it shows, Edit Widgets…, Remove Widget), and one on the
//! desktop itself, Edit Widgets…, which opens the gallery (`edit.rs`). The
//! cards fade back while windows are open on the monitor's workspace.
//!
//! amane measures a drag against the hit areas of the last frame and finds
//! the dragged target by its place among them, so every drag is measured on
//! one still layer over the whole surface that comes first, and the cards
//! take no input of their own. See `docs/shell/widgets.md`.

mod catalog;
mod edit;
mod gallery;
mod input;
mod ipc;
mod kind;
mod menu;
mod store;
#[cfg(test)]
mod tests;
mod view;

use input::*;
pub use ipc::run;
use menu::*;
use view::*;

use std::cell::RefCell;
use std::collections::HashSet;

use amane::{
    Animation, BlurShape, Button, Color, Cursor, Desktop, Full, Horizontal, InputArea, Keyboard,
    Layer, LayerWindow, Margin, Monitor, Point, Rectangle, Service, Shadow, Stack, Vertical,
    Widget, WindowSize, WorkspaceSwitch, Workspaces, Zone,
};

use crate::chrome::bar;
use crate::chrome::per_output::PerOutput;
use crate::services::desktop::system_stats::SystemStats;
use crate::surfaces::status_bar;
use amane_kit::glass::{self, Rim};
use amane_kit::glass_region;
use amane_kit::grid::Layout;
use amane_kit::grid::drag::{Drag, Hit, Origin, Session, hit};
use amane_kit::grid::geometry::{Canvas, Metrics, Place, Rect, Slot, Span};
use amane_kit::layout::{blank, boxed};
use amane_kit::material::{self, Material};
use amane_kit::menu::layer as context_menu;
use amane_kit::menu::{ContextMenu, MenuItem};
use amane_kit::motion::{self, Arrive, Item, Pose};
use amane_kit::settings::{ACCENTS, WidgetStyle, desktop};
use amane_kit::shell_ipc::Widgets as Command;

use kind::{Category, WidgetKey, WidgetKind, WidgetSize};
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
/// The room the background surface keeps round the canvas for the cards'
/// shadows (`shadow()`: 24 blurred, 8 down), so the surface is the grid and
/// no more. A surface costs four or five times its pixels on the GPU (its
/// swapchain, canvas and vibrant layers), and the screen is mostly not grid.
const PAD: f32 = 32.0;
/// A card's corners.
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
/// margin, above the bar, centred in what's left. `origin`, `width` and
/// `height` are against the surface the cards are drawn on: the whole
/// monitor as [`Self::of`] gives it (the edit surface), or the part of it
/// the background surface covers, see [`Self::on_surface_around`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub canvas: Canvas,
    pub origin: (f32, f32),
    pub width: f32,
    pub height: f32,
    /// Where the surface is on the monitor: x, y, width, height.
    pub surface: (f32, f32, f32, f32),
}

impl Geometry {
    fn of(width: f32, height: f32) -> Self {
        let px = metrics();
        let area_w = width - 2.0 * MARGIN;
        let bar = bar::height();
        let area_h = height - bar - 2.0 * MARGIN;
        let canvas = Canvas::fit(&px, area_w, area_h);
        let (x, y) = px.centred_origin(&canvas, area_w, area_h);
        // below a bar along the top, rather than above one along the bottom
        let top = if bar::edge() == Vertical::Top {
            bar
        } else {
            0.0
        };
        Self {
            canvas,
            origin: (MARGIN + x, top + MARGIN + y),
            width,
            height,
            surface: (0.0, 0.0, width, height),
        }
    }

    /// The same canvas against a surface that covers only `extent` (x, y,
    /// width, height in the canvas's pixels) and [`PAD`] round it, kept
    /// within the monitor: the background surface, which is as big as the
    /// cards on it and no more.
    fn on_surface_around(self, extent: (f32, f32, f32, f32)) -> Self {
        let (x, y, w, h) = extent;
        let left = (self.origin.0 + x - PAD).max(0.0).floor();
        let top = (self.origin.1 + y - PAD).max(0.0).floor();
        let right = (self.origin.0 + x + w + PAD).min(self.width).ceil();
        let bottom = (self.origin.1 + y + h + PAD).min(self.height).ceil();
        let width = (right - left).max(1.0);
        let height = (bottom - top).max(1.0);
        Self {
            origin: (self.origin.0 - left, self.origin.1 - top),
            width,
            height,
            surface: (left, top, width, height),
            ..self
        }
    }

    /// The whole canvas, in its own pixels.
    fn whole(&self) -> (f32, f32, f32, f32) {
        let px = metrics();
        (
            0.0,
            0.0,
            px.extent(self.canvas.cols),
            px.extent(self.canvas.rows),
        )
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
    /// Nothing under the windows shows, so the surface keeps its last frame.
    frozen: bool,
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
            frozen: false,
        }
    }
}

thread_local! {
    static DESKS: PerOutput<Desk> = const { PerOutput::new() };
    /// The windows that show a system card, live, now.
    static WATCHING: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    /// The windows that show a temperature, live, now.
    static HEAT: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
}

/// Read by every widget window and written whenever the widgets change,
/// so a change redraws the desktops and their edit surfaces, not the bar
/// and the dock as `mark_all` would.
struct Desktops;

impl Service for Desktops {
    fn new() -> Self {
        Self
    }

    // only input changes it
    fn listen() {}
}

/// Whether the window `name` shows a system card, and a temperature: the
/// readings go on while any window does, and pause once none does.
fn watching(name: &'static str, shows: bool, heat: bool) {
    let note = |set: &RefCell<HashSet<&'static str>>, on: bool| {
        let mut set = set.borrow_mut();
        if on {
            set.insert(name);
        } else {
            set.remove(name);
        }
        !set.is_empty()
    };
    let any = WATCHING.with(|set| note(set, shows));
    let any_heat = HEAT.with(|set| note(set, heat));
    SystemStats::watch(any, any_heat);
}

/// The widgets changed: every monitor's desktop draws again.
fn changed() {
    drop(Desktops::write());
}

fn with_desk<R>(output: &str, f: impl FnOnce(&mut Desk) -> R) -> R {
    DESKS.with(|desks| desks.with(output, Desk::new, f))
}

/// The namespace `amane.widgets.<name>`: a monitor's desktop surface (its
/// name) or its edit surface (`edit.<name>`), kept for the run, as amane
/// takes a `&'static str`.
fn namespace(output: &str) -> &'static str {
    amane_kit::intern::intern(&format!("{WIDGETS_NS}{output}"))
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
    changed();
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The widgets on `monitor`, on the wallpaper under the windows.
/// Registered with `App::window_per_monitor`. While the monitor's desktop
/// is edited its cards are drawn by [`edit_surface`] instead.
pub fn surface(monitor: &Monitor) -> LayerWindow {
    desk_window(monitor, false)
}

/// The monitor's desktop while it's edited: the cards and the gallery
/// over the windows, the desktop cleared to edit it; hidden the
/// rest of the time. A surface of its own on the overlay layer rather than
/// the desktop's moved there: niri doesn't frost a surface moved off the
/// background layer.
pub fn edit_surface(monitor: &Monitor) -> LayerWindow {
    desk_window(monitor, true)
}

fn desk_window(monitor: &Monitor, edit_window: bool) -> LayerWindow {
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
    if !showing && edit::editing(&output) {
        edit::leave_all();
    }
    let editing = edit::editing(&output);
    if !showing || (edit_window && !editing) {
        watching(name, false, false);
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

    let layout = shown(&output, geometry.canvas);

    /*
     * the edit surface covers the monitor (the gallery, and cards dragged
     * anywhere); the background one covers the cards on it and the room for
     * their shadows, since a surface costs by its pixels, and the whole
     * canvas only while a card is dragged across it: the drag's points are
     * kept in the canvas's pixels, so the surface growing under it is fine
     */
    let (window, geometry) = if edit_window {
        let window = LayerWindow::new().width(Full).height(Full);
        (window, geometry)
    } else {
        let dragging = with_desk(&output, |desk| desk.session.is_some());
        let extent = if dragging {
            geometry.whole()
        } else {
            cards_extent(&layout)
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
    };
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
    // the window that shows the cards says where the pointer meets the grid
    with_desk(&output, |desk| desk.geometry = Some(geometry));

    // the gallery shows every kind live while editing
    let system = layout
        .placed()
        .any(|(key, _)| key.kind.category() == Category::System);
    let heat = layout
        .placed()
        .any(|(key, _)| key.kind == WidgetKind::Temperature);
    /*
     * nothing under the windows shows (niri's overview fully open, or windows
     * over the whole screen): the surface keeps its last frame and draws
     * nothing, reading only what says when it shows again. The readings pause
     * too, so a covered desktop costs no wake-ups; the graphs carry on from
     * where they were when it shows again (AMANE_STATS_WHILE_HIDDEN=1 keeps
     * gathering history under the windows, as before)
     */
    let hidden = !editing && Desktop::read().obscured(&output);
    let gather = !hidden || stats_while_hidden();
    // the gallery and the settings' sensor list read every sensor
    watching(
        name,
        (editing || system) && gather,
        (editing || heat) && gather,
    );
    let thawed = with_desk(&output, |desk| std::mem::replace(&mut desk.frozen, hidden)) && !hidden;
    if hidden {
        return window
            .keyboard(Keyboard::None)
            .input_region(vec![full])
            .frozen(true)
            .child(Stack::new(Vec::new()));
    }
    // shown again: the cards come back as they are now, nothing animating
    if thawed {
        motion::forget(&scope(&output));
        for (key, _) in layout.placed() {
            if key.kind == WidgetKind::Clock {
                catalog::settle_clock(key.id);
            }
        }
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
        window.on_key(move |key| edit::key(&on, key))
    } else {
        window
    }
}

struct View {
    root: Stack,
    frost: Vec<BlurShape>,
}

// ───── menus ─────────────────────────────────────────────────────────────────

const WIDGET_MENU_KEY: &str = "amane.widgets.widget-menu";
const DESKTOP_MENU_KEY: &str = "amane.widgets.desktop-menu";

type Field = fn(&mut Settings) -> &mut Option<String>;

/// Whether the system readings go on while the desktop is covered (`AMANE_STATS_WHILE_HIDDEN=1`).
fn stats_while_hidden() -> bool {
    static KEEP: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *KEEP.get_or_init(|| std::env::var_os("AMANE_STATS_WHILE_HIDDEN").is_some_and(|v| v == "1"))
}
