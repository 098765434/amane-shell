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
//! own settings. Each monitor keeps its own layout (`store/`).
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
//! take no input of their own. See `docs/user/features/widgets.md`.

// a function over 100 lines is one to split (polish-plan.md, phase 3)
#![warn(clippy::too_many_lines)]

mod catalog;
mod edit;
mod gallery;
mod input;
mod ipc;
mod keys;
mod kind;
mod menu;
mod store;
#[cfg(test)]
mod tests;
mod view;
mod window;

use input::*;
pub use ipc::run;
use menu::*;
use view::*;
use window::desk_window;

use std::cell::RefCell;
use std::collections::HashSet;

use amane::{
    Animation, BlurShape, Button, Cursor, Desktop, Full, Horizontal, InputArea, Keyboard, Layer,
    LayerWindow, Margin, Monitor, Point, Rectangle, Service, Shadow, Stack, Vertical, Widget,
    WindowSize, WorkspaceSwitch, Workspaces, Zone,
};

use crate::chrome::bar;
use crate::chrome::per_output::PerOutput;
use crate::services::desktop::system_stats::SystemStats;
use crate::surfaces::status_bar;
use crate::tokens::{quick, widget};
use emaki_kit::glass::{self, Rim};
use emaki_kit::glass_region;
use emaki_kit::grid::Layout;
use emaki_kit::grid::drag::{Drag, Hit, Origin, Session, hit};
use emaki_kit::grid::geometry::{Canvas, Metrics, Place, Rect, Slot, Span};
use emaki_kit::layout::{blank, boxed};
use emaki_kit::material::{self, Material};
use emaki_kit::menu::layer as context_menu;
use emaki_kit::menu::{ContextMenu, MenuItem};
use emaki_kit::motion::{self, Arrive, Item, Pose};
use emaki_kit::settings::{WidgetStyle, desktop};
use emaki_kit::shell_ipc::Widgets as Command;

use kind::{Reads, SettingKind, SettingSpec, WidgetKey, WidgetKind, WidgetSize};

/// Every monitor's widget surface is `emaki.widgets.<output>`; the niri
/// `layer-rule` matches the prefix. Each has its own name, so a card's menu
/// hangs under the surface on its own monitor.
pub const WIDGETS_NS: &str = "emaki.widgets.";

/// The room the background surface keeps round the canvas for the cards'
/// shadows (`quick.shadow`: 24 blurred, 8 down), so the surface is the grid
/// and no more. A surface costs four or five times its pixels on the GPU
/// (its swapchain, canvas and vibrant layers), and the screen is mostly not
/// grid.
const SHADOW_ROOM: f32 = 32.0;

/// The room kept clear round the canvas, at the screen's edges: Quick
/// Settings' grid margin.
fn margin() -> f32 {
    quick::grid::margin()
}

/// A card's corners.
fn radius() -> f32 {
    widget::card::radius()
}

fn metrics() -> Metrics {
    Metrics::new(quick::grid::cell(), quick::grid::gap())
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
        let area_w = width - 2.0 * margin();
        let bar = bar::height();
        let area_h = height - bar - 2.0 * margin();
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
            origin: (margin() + x, top + margin() + y),
            width,
            height,
            surface: (0.0, 0.0, width, height),
        }
    }

    /// The same canvas against a surface that covers only `extent` (x, y,
    /// width, height in the canvas's pixels) and [`SHADOW_ROOM`] round it, kept
    /// within the monitor: the background surface, which is as big as the
    /// cards on it and no more.
    fn on_surface_around(self, extent: (f32, f32, f32, f32)) -> Self {
        let (x, y, w, h) = extent;
        let left = (self.origin.0 + x - SHADOW_ROOM).max(0.0).floor();
        let top = (self.origin.1 + y - SHADOW_ROOM).max(0.0).floor();
        let right = (self.origin.0 + x + w + SHADOW_ROOM).min(self.width).ceil();
        let bottom = (self.origin.1 + y + h + SHADOW_ROOM)
            .min(self.height)
            .ceil();
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
    /// A drag let go of with Escape while the button is still down: the
    /// pointer takes nothing more until the button comes up.
    cancelled: bool,
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
            cancelled: false,
            slots: Vec::new(),
            pointer: Point { x: 0.0, y: 0.0 },
            dim: motion::fade(0.0, motion::duration::fade()),
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

/// The namespace `emaki.widgets.<name>`: a monitor's desktop surface (its
/// name) or its edit surface (`edit.<name>`), kept for the run, as amane
/// takes a `&'static str`.
fn namespace(output: &str) -> &'static str {
    emaki_kit::intern::intern(&format!("{WIDGETS_NS}{output}"))
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

struct View {
    root: Stack,
    frost: Vec<BlurShape>,
}

// ───── menus ─────────────────────────────────────────────────────────────────

const WIDGET_MENU_KEY: &str = "emaki.widgets.widget-menu";
const DESKTOP_MENU_KEY: &str = "emaki.widgets.desktop-menu";

/// Whether the system readings go on while the desktop is covered (`EMAKI_STATS_WHILE_HIDDEN=1`).
fn stats_while_hidden() -> bool {
    static KEEP: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *KEEP.get_or_init(|| std::env::var_os("EMAKI_STATS_WHILE_HIDDEN").is_some_and(|v| v == "1"))
}
