//! Dock — a row of app icons, for the windows open on every workspace.
//!
//! One frosted glass card over the bar, on every monitor, holding a section
//! for each of the monitor's workspaces, side by side with a rule between
//! them: an icon for each window open on it, in niri's column order, then
//! the apps pinned to it that aren't open there. Clicking a window's icon
//! focuses it; clicking a pinned app's focuses its latest window, or opens
//! it on its workspace. A right-click opens the icon's menu, which keeps
//! the dock up while it's open.
//!
//! It hides below the bottom edge ([`EdgeReveal`]) and comes up when the
//! pointer touches the edge; it goes back down a moment after the pointer
//! leaves. While niri's overview opens it rises in step with the zoom. The
//! icons swell under the pointer ([`magnify`]), with the name
//! over the one under it, and shrink to fit when there are too many for
//! the monitor.
//!
//! The glass is Quick Settings': the compositor frosts behind the card
//! where the surface says ([`glass_region`]), or the shader draws its lit
//! edge where the compositor has no glass material.
//!
//! The surface spans the monitor and doesn't change size: a resize every
//! frame costs a compositor round-trip and a second draw. It takes the
//! pointer only where the card is, and along the bottom edge. Every
//! animation is in [`motion::DOCK`], each with its own switch, and
//! `AMANE_DOCK_ANIMATIONS=off` turns them all off. See `docs/user/features/dock.md`.

mod drag;
mod input;
mod item;
mod layout;
mod magnify;
mod model;
pub mod motion;
mod pins;
#[cfg(test)]
mod tests;
mod view;
mod window_menu;

use input::*;
use layout::*;
use view::*;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use amane::{
    Animation, BlurShape, Button, Cursor, Full, InputArea, Keyboard, Layer, LayerWindow, Monitor,
    Overview, Parent, Point, Rectangle, Row, Service, Stack, Start, Vertical, Widget, Windows,
    Workspaces, Zone,
};

use amane_kit::settings::dock as dock_settings;

use crate::chrome::bar;
use crate::chrome::per_output::PerOutput;
use crate::tokens::sys;
use amane_kit::app_icon;
use amane_kit::divider::Divider;
use amane_kit::edge_reveal::EdgeReveal;
use amane_kit::glass_region;
use amane_kit::layout::{blank, boxed};
use amane_kit::material::{self, Material};
use amane_kit::menu::layer as context_menu;
use amane_kit::motion::redraw_after;

use drag::{Drag, Drop, Settle};
use magnify::{Rest, Swollen};
use model::{Icon, Section, Space, Win};
use motion::{DockMotion, anim};
use window_menu::{open_pin_menu, open_window_menu};

amane_kit::marker! {
    /// The dock's own state changed (its pins, a spring-load running
    /// out): every monitor's dock draws again, nothing else.
    pub Docks
}

/// The dock's layer-surface namespace; the niri `layer-rule` matches it
/// (a clear surface that says where to frost), and an icon's menu hangs
/// under it.
pub const DOCK_NS: &str = "amane.dock";

// TODO(token): `dock.icon.least`, `dock.radius` and `dock.reveal.strip`
// once Penpot has them (manual §1.1). How big an icon is at rest, with room
// for them all, is the dock settings' `icon_size`.
/// The least an icon shrinks to, to fit the row on the monitor.
const LEAST_ICON: f32 = 20.0;
/// The card's corners.
const RADIUS: f32 = 18.0;
/// How tall the strip along the bottom edge that brings the dock up is.
const STRIP: f32 = 2.0;

/// One monitor's dock, kept across rebuilds.
struct Dock {
    // the motion its animations were made with, to make them again when
    // the settings change it
    motion: DockMotion,
    reveal: EdgeReveal,
    // 0 … 1: the swell coming on with the pointer and going with it
    swell: Animation,
    // the pointer on the surface
    pointer: Option<(f32, f32)>,
    // where the pointer last was along the row, for the swell to go down
    // where it was
    last_at: Option<f32>,
    // the name label coming in, for the icon it's over
    tip: Animation,
    tip_for: Option<String>,
    // each icon's and rule's slot and growth, by key
    items: HashMap<String, Item>,
    // the card's width at rest
    width: Option<Animation>,
    // whether it was out last frame
    out: bool,
    // the icon the pointer holds, and where its window is going
    drag: Option<Drag>,
    target: Option<(Drop, usize)>,
    // a window let go of, shown where it went until niri has it there
    settle: Option<Settle>,
    // a drag just ended, so the click that follows isn't one
    dragged: bool,
    // files from another program held over the dock, and where they were
    // last, for when they're let go
    files: Option<FilesOver>,
    files_at: Option<(f32, f32)>,
}

/// Files held over the dock: the icon they're over, since when, whether
/// its app opens them, and whether its window has come into view for them.
#[derive(Default)]
struct FilesOver {
    icon: Option<(String, Instant)>,
    opens: bool,
    sprung: bool,
}

/// An icon's or rule's motion: its slot along the row, and how far it has
/// grown in.
struct Item {
    slot: Animation,
    presence: Animation,
}

impl Dock {
    fn new(motion: &DockMotion) -> Self {
        Self {
            motion: *motion,
            reveal: EdgeReveal::new(anim(&motion.reveal, 0.0), motion.hide_delay),
            swell: anim(&swell_anim(motion), 0.0),
            pointer: None,
            last_at: None,
            tip: anim(&motion.tooltip.unwrap_or(motion.reveal), 0.0),
            tip_for: None,
            items: HashMap::new(),
            width: None,
            out: false,
            drag: None,
            target: None,
            settle: None,
            dragged: false,
            files: None,
            files_at: None,
        }
    }
}

thread_local! {
    static DOCKS: PerOutput<Dock> = const { PerOutput::new() };

    // the windows by when they last had the focus, latest first, for which
    // of an app's windows its pin brings up
    static RECENT: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };

    // the windows seen so far, to tell a new one
    static SEEN: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());

    // the monitor whose dock opened an icon's menu, which keeps that dock
    // up while the menu is open
    static MENU_ON: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Runs `f` on monitor `output`'s dock, made afresh when the settings
/// changed its motion.
fn with_dock<R>(output: &str, f: impl FnOnce(&mut Dock) -> R) -> R {
    let motion = motion::motion();
    DOCKS.with(|docks| {
        docks.with(
            output,
            || Dock::new(&motion),
            |dock| {
                if dock.motion != motion {
                    *dock = Dock::new(&motion);
                }
                f(dock)
            },
        )
    })
}

// ───── measures ──────────────────────────────────────────────────────────────

/// The dock's measures, from the tokens, its motion and how much it holds.
#[derive(Clone, Copy)]
struct Measures {
    // the surface
    width: f32,
    height: f32,
    // the card's top, and how tall it is
    top: f32,
    card: f32,
    // an icon at rest, and an icon and its gap
    side: f32,
    pitch: f32,
    gap: f32,
    pad: f32,
    // how many slots a rule between two workspaces takes
    rule: f32,
    // how far over the card the swollen icons and the name reach
    headroom: f32,
}

impl Measures {
    /// The measures on `monitor` for a row of `icons` icons and `rules`
    /// rules: icons as big as fits, up to the settings' size.
    fn new(monitor: &Monitor, motion: &DockMotion, icons: usize, rules: usize) -> Self {
        let gap = sys::space::gap::xs();
        let pad = sys::space::inset::s();
        // a rule keeps a small gap either side, where an icon keeps an xs
        let rule_px = Divider::width() + sys::space::gap::s() * 2.0 - gap;
        let room = monitor.width as f32 - sys::space::inset::m() * 2.0;
        let most = dock_settings().icon();
        let fit = if icons == 0 {
            most
        } else {
            (room - pad * 2.0 + gap - rules as f32 * rule_px) / icons as f32 - gap
        };
        let side = fit.clamp(LEAST_ICON.min(most), most);

        let (scale, lift) = motion.magnify.map_or((1.0, 0.0), |magnify| magnify.most());
        let tip = if motion.tooltip.is_some() {
            item::label_height() + sys::space::gap::s()
        } else {
            0.0
        };
        let headroom = |side: f32| side * (scale - 1.0) + lift + tip + sys::space::gap::s();
        // over the bar, the gap a popover keeps from it; a bar along the top
        // isn't in the way, nor is one a dock that stays out keeps above
        let over_bar = bar::edge() == Vertical::Bottom && dock_settings().auto_hide;
        let below = if over_bar {
            bar::height() + sys::space::gap::m()
        } else {
            sys::space::gap::m()
        };
        // the surface is as tall as the biggest icons need, whatever size
        // they are now, so it never resizes
        let height = (headroom(most) + most + pad * 2.0 + below).ceil();
        let card = side + pad * 2.0;
        Self {
            width: monitor.width as f32,
            height,
            top: height - below - card,
            card,
            side,
            pitch: side + gap,
            gap,
            pad,
            rule: rule_px / (side + gap),
            headroom: headroom(side),
        }
    }

    /// How wide a card with icons running `slots` long is.
    fn card_width(&self, slots: f32) -> f32 {
        let row = (slots * self.pitch - self.gap).max(0.0);
        self.pad + row + self.pad
    }

    /// Where a card `width` wide starts, centred on the monitor.
    fn card_left(&self, width: f32) -> f32 {
        (self.width - width) / 2.0
    }

    /// How far down the dock is while `progress` of the way out: all the
    /// way under the bottom edge while hidden.
    fn sunk(&self, progress: f32) -> f32 {
        (1.0 - progress) * (self.height - self.top)
    }
}

// ───── the row ───────────────────────────────────────────────────────────────

/// What a place along the row holds.
#[derive(Clone, Debug)]
enum Kind {
    Icon(Icon),
    /// The rule before a workspace's section.
    Rule,
    /// The place of a workspace with nothing on it (the one showing).
    Empty,
}

/// One place along the row: what it holds, the section it's in, and how
/// many slots it takes.
#[derive(Clone, Debug)]
struct Entry {
    key: String,
    kind: Kind,
    section: usize,
    width: f32,
}

impl Entry {
    /// Whether the pointer can be over it.
    fn pointable(&self) -> bool {
        !matches!(self.kind, Kind::Rule)
    }
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The dock on `monitor`. Registered with `App::window_per_monitor`.
pub fn surface(monitor: &Monitor) -> LayerWindow {
    Docks::watch();
    let motion = motion::motion();

    let windows: Vec<Win> = Windows::read().list().iter().map(Win::from).collect();
    let focused = windows.iter().find(|window| window.focused).map(|w| w.id);
    RECENT.with_borrow_mut(|recent| model::remember(recent, focused, &windows));
    place_new(&windows);
    let spaces: Vec<Space> = Workspaces::read().list().iter().map(Space::from).collect();
    let sections = model::sections(&spaces, &windows, &pins::all(), &monitor.name);
    let hold = if motion.follow_overview {
        Overview::read().progress() as f32
    } else {
        0.0
    };

    let name = monitor.name.clone();
    let (view, at) = with_dock(&monitor.name, |dock| {
        let sections = settled(dock, sections, &spaces, &name);
        let icons = sections.iter().map(|s| s.icons.len().max(1)).sum();
        let at = Measures::new(monitor, &motion, icons, sections.len().saturating_sub(1));
        let (sections, held) = arrange(dock, sections, at);
        let entries = row(&sections, at.rule);
        // the card is small glass, light or dark with its backdrop
        let view = Material::Control.adapted(glass_id(&name), || {
            build(
                dock, &name, &sections, entries, &spaces, held, hold, at, &motion,
            )
        });
        (view, at)
    });

    // hiding, it covers the bar to the bottom edge and keeps nothing out;
    // staying out, it keeps windows clear of the card, above the bar
    let zone = if dock_settings().auto_hide {
        Zone::Ignore
    } else {
        Zone::Thickness((at.height - at.top) as u32)
    };

    /*
     * hidden all the way, the surface is only the strip that brings it up, so
     * a dock that hides holds a few rows of pixels instead of a full-width
     * vibrant surface; touching the strip grows it back before it slides out
     * (AMANE_DOCK_FULL_WHILE_HIDDEN=1 keeps it full size, as before)
     */
    if view.tucked && !full_while_hidden() {
        return LayerWindow::new()
            .width(Full)
            .height(STRIP)
            .anchor_vertical(Vertical::Bottom)
            .layer(Layer::Overlay)
            .space(zone)
            .keyboard(Keyboard::None)
            .namespace(DOCK_NS)
            .visible(true)
            .input_region(vec![InputArea {
                x: 0,
                y: 0,
                width: at.width.max(1.0) as i32,
                height: STRIP as i32,
            }])
            .blur_shapes(Vec::new())
            .child(view.root);
    }

    LayerWindow::new()
        .width(Full)
        .height(at.height)
        .vibrant(material::vibrant())
        .anchor_vertical(Vertical::Bottom)
        .layer(Layer::Overlay)
        .space(zone)
        // clicking an icon focuses its window, so the dock never takes keys
        .keyboard(Keyboard::None)
        .namespace(DOCK_NS)
        .visible(true)
        .input_region(view.input)
        .blur_shapes(view.frost)
        .child(view.root)
}

/// The held icon, drawn where the pointer has it rather than in its place.
struct Held {
    key: String,
    icon: Icon,
    x: f32,
    y: f32,
    // pulled up off the dock: a pinned app let go there comes out of it
    off: bool,
}

/// What one frame of the dock draws, and where it takes the pointer.
struct View {
    root: Stack,
    frost: Vec<BlurShape>,
    input: Vec<InputArea>,
    /// All the way in and nothing bringing it out: the surface shrinks to the strip.
    tucked: bool,
}

/// The row laid out: its places, each at rest and how far grown in, and
/// the card's width for them at rest.
#[derive(Clone)]
struct Laid {
    entries: Vec<Entry>,
    rests: Vec<Rest>,
    shown: Vec<f32>,
    width: f32,
}

impl Laid {
    /// Which place the pointer is over at `along` slots along the row at
    /// rest, never a rule.
    fn under(&self, along: f32) -> Option<usize> {
        self.entries
            .iter()
            .zip(&self.rests)
            .position(|(entry, rest)| {
                entry.pointable() && along >= rest.slot && along < rest.slot + entry.width
            })
    }
}

/// Whether a hidden dock keeps its full surface (`AMANE_DOCK_FULL_WHILE_HIDDEN=1`).
fn full_while_hidden() -> bool {
    static FULL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FULL.get_or_init(|| std::env::var_os("AMANE_DOCK_FULL_WHILE_HIDDEN").is_some_and(|v| v == "1"))
}
