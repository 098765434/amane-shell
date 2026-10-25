//! Dock — the macOS Big Sur dock, for the windows open on every workspace.
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
//! icons swell under the pointer as on macOS ([`magnify`]), with the name
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
//! `AMANE_DOCK_ANIMATIONS=off` turns them all off. See `docs/dock.md`.

mod drag;
mod item;
mod magnify;
mod model;
pub mod motion;
mod pins;
mod window_menu;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use amane::{
    Animation, BlurShape, Button, Color, Cursor, Full, InputArea, Keyboard, Layer, LayerWindow,
    Monitor, Overview, Parent, Point, Rectangle, Row, Service, Stack, Start, Vertical, Widget,
    Windows, Workspaces, Zone, mark_all,
};

use crate::components::app_icon;
use crate::components::context_menu;
use crate::components::divider::Divider;
use crate::components::edge_reveal::EdgeReveal;
use crate::components::glass_region;
use crate::components::layout::{blank, boxed};
use crate::components::motion::redraw_after;
use crate::tokens::sys;

use drag::{Drag, Drop, Settle};
use magnify::{Rest, Swollen};
use model::{Icon, Section, Space, Win};
use motion::{DockMotion, anim};
use window_menu::{open_pin_menu, open_window_menu};

/// The dock's layer-surface namespace; the niri `layer-rule` matches it
/// (a clear surface that says where to frost), and an icon's menu hangs
/// under it.
pub const DOCK_NS: &str = "amane.dock";

// TODO(token): `dock.icon.size`, `dock.icon.least`, `dock.radius` and
// `dock.reveal.strip` once Penpot has them (manual §1.1).
/// How big an icon is at rest, with room for them all.
const ICON: f32 = 44.0;
/// The least an icon shrinks to, to fit the row on the monitor.
const LEAST_ICON: f32 = 20.0;
/// The card's corners.
const RADIUS: f32 = 18.0;
/// How tall the strip along the bottom edge that brings the dock up is.
const STRIP: f32 = 2.0;

/// One monitor's dock, kept across rebuilds.
struct Dock {
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
    static DOCKS: RefCell<HashMap<String, Dock>> = RefCell::new(HashMap::new());

    // the windows by when they last had the focus, latest first, for which
    // of an app's windows its pin brings up
    static RECENT: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };

    // the windows seen so far, to tell a new one
    static SEEN: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());

    // the monitor whose dock opened an icon's menu, which keeps that dock
    // up while the menu is open
    static MENU_ON: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Runs `f` on monitor `output`'s dock.
fn with_dock<R>(output: &str, f: impl FnOnce(&mut Dock) -> R) -> R {
    DOCKS.with_borrow_mut(|docks| {
        let dock = docks
            .entry(output.to_string())
            .or_insert_with(|| Dock::new(motion::motion()));
        f(dock)
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
    /// rules: icons as big as fits, up to [`ICON`].
    fn new(monitor: &Monitor, motion: &DockMotion, icons: usize, rules: usize) -> Self {
        let gap = sys::space::gap::xs();
        let pad = sys::space::inset::s();
        // a rule keeps a small gap either side, where an icon keeps an xs
        let rule_px = Divider::width() + sys::space::gap::s() * 2.0 - gap;
        let room = monitor.width as f32 - sys::space::inset::m() * 2.0;
        let fit = if icons == 0 {
            ICON
        } else {
            (room - pad * 2.0 + gap - rules as f32 * rule_px) / icons as f32 - gap
        };
        let side = fit.clamp(LEAST_ICON, ICON);

        let (scale, lift) = motion.magnify.map_or((1.0, 0.0), |magnify| magnify.most());
        let tip = if motion.tooltip.is_some() {
            item::label_height() + sys::space::gap::s()
        } else {
            0.0
        };
        let headroom = |side: f32| side * (scale - 1.0) + lift + tip + sys::space::gap::s();
        // over the bar, the gap a popover keeps from it
        let below = sys::size::panel::height() + sys::space::gap::m();
        // the surface is as tall as the biggest icons need, whatever size
        // they are now, so it never resizes
        let height = (headroom(ICON) + ICON + pad * 2.0 + below).ceil();
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

/// The row for `sections`: each section's icons, a rule `rule` slots wide
/// between one section and the next.
fn row(sections: &[Section], rule: f32) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let key = section.key();
        if index > 0 {
            entries.push(Entry {
                key: format!("rule:{key}"),
                kind: Kind::Rule,
                section: index,
                width: rule,
            });
        }
        if section.icons.is_empty() {
            entries.push(Entry {
                key: format!("empty:{key}"),
                kind: Kind::Empty,
                section: index,
                width: 1.0,
            });
        }
        for icon in &section.icons {
            entries.push(Entry {
                key: icon.key(&key),
                kind: Kind::Icon(icon.clone()),
                section: index,
                width: 1.0,
            });
        }
    }
    entries
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The dock on `monitor`. Registered with `App::window_per_monitor`.
pub fn surface(monitor: &Monitor) -> LayerWindow {
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
        let at = Measures::new(monitor, motion, icons, sections.len().saturating_sub(1));
        let (sections, held) = arrange(dock, sections, at);
        let entries = row(&sections, at.rule);
        let view = build(
            dock, &name, &sections, entries, &spaces, held, hold, at, motion,
        );
        (view, at)
    });

    LayerWindow::new()
        .width(Full)
        .height(at.height)
        .anchor_vertical(Vertical::Bottom)
        // over the bar, which it covers to the bottom edge
        .layer(Layer::Overlay)
        .space(Zone::Ignore)
        // clicking an icon focuses its window, so the dock never takes keys
        .keyboard(Keyboard::None)
        .namespace(DOCK_NS)
        .visible(true)
        .input_region(view.input)
        .blur_shapes(view.frost)
        .child(view.root)
}

/// `sections` with a window let go of where it went, until niri has it
/// there, and with the empty workspace after the last to drop on while a
/// window is held.
fn settled(
    dock: &mut Dock,
    mut sections: Vec<Section>,
    spaces: &[Space],
    output: &str,
) -> Vec<Section> {
    if let Some(settle) = &dock.settle {
        if settle.done(&sections) {
            dock.settle = None;
        } else {
            sections = drag::with(&sections, &settle.window, &settle.drop);
        }
    }
    if dock
        .drag
        .as_ref()
        .is_some_and(|drag| drag.moved && drag.window().is_some())
    {
        sections = model::with_last(sections, spaces, output);
    }
    sections
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

/// `sections` as they'd be with the held window let go where the pointer
/// is, the others making way, and the held icon; forgets a drag whose
/// icon has gone.
fn arrange(dock: &mut Dock, sections: Vec<Section>, at: Measures) -> (Vec<Section>, Option<Held>) {
    dock.target = None;
    let Some(drag) = dock.drag.as_mut().filter(|drag| drag.moved) else {
        return (sections, None);
    };
    // the icon as it is now (its window's title may have changed)
    let now = sections.iter().find_map(|section| {
        let key = section.key();
        section
            .icons
            .iter()
            .find(|icon| icon.key(&key) == drag.key)
            .cloned()
    });
    let Some(icon) = now else {
        dock.drag = None;
        return (sections, None);
    };
    drag.icon = icon.clone();
    let off = drag.pointer.1 < at.top - at.headroom;
    let held = Held {
        key: drag.key.clone(),
        icon,
        x: drag.pointer.0 - drag.grab.0,
        y: drag.pointer.1 - drag.grab.1,
        off,
    };
    let Some(window) = drag.window().cloned() else {
        return (sections, Some(held));
    };
    if off {
        return (sections, Some(held));
    }

    let base = drag::without(&sections, window.id);
    let slots: f32 = base
        .iter()
        .map(|s| s.icons.len().max(1) as f32)
        .sum::<f32>()
        + base.len().saturating_sub(1) as f32 * at.rule;
    let along = along(at, at.card_width(slots), drag.pointer.0);
    let Some(drop) = drag::drop_at(&base, at.rule, along) else {
        return (sections, Some(held));
    };
    let column = base
        .iter()
        .find(|section| section.workspace == Some(drop.workspace))
        .map_or(1, |section| drag::column_index(section, drop.place));
    let preview = drag::with(&base, &window, &drop);
    dock.target = Some((drop, column));
    (preview, Some(held))
}

/// The held icon let go at `y` on the surface: its window to where it was
/// going, or a pinned app pulled off the dock out of it.
fn let_go(dock: &mut Dock, y: f32, at: Measures) {
    let Some(drag) = dock.drag.take() else {
        return;
    };
    if !drag.moved {
        return;
    }
    dock.dragged = true;
    match &drag.icon {
        Icon::Window(window) => {
            if let Some((drop, column)) = dock.target.take() {
                let elsewhere =
                    (window.workspace != Some(drop.workspace)).then_some(drop.workspace);
                Windows::move_to(window.id, elsewhere, column);
                dock.settle = Some(Settle::new(window.clone(), drop));
            }
        }
        Icon::Pin(pin) => {
            if y < at.top - at.headroom {
                pins::remove(pin);
            }
        }
    }
}

/// Moves each window that's new since the last frame to the workspace a
/// pin opened its app for, if it turned up elsewhere.
fn place_new(windows: &[Win]) {
    SEEN.with_borrow_mut(|seen| {
        for window in windows {
            if seen.insert(window.id)
                && let Some(workspace) = pins::placed(&window.app_id)
                && window.workspace != Some(workspace)
            {
                Windows::move_to_workspace(window.id, workspace);
            }
        }
        seen.retain(|id| windows.iter().any(|window| window.id == *id));
    });
}

/// What one frame of the dock draws, and where it takes the pointer.
struct View {
    root: Stack,
    frost: Vec<BlurShape>,
    input: Vec<InputArea>,
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

#[allow(clippy::too_many_arguments)]
fn build(
    dock: &mut Dock,
    output: &str,
    sections: &[Section],
    entries: Vec<Entry>,
    spaces: &[Space],
    held: Option<Held>,
    hold: f32,
    at: Measures,
    motion: &'static DockMotion,
) -> View {
    let menu_open = context_menu::runtime::is_open_under(DOCK_NS)
        && MENU_ON.with_borrow(|on| on.as_deref() == Some(output));
    // a menu open, an icon held or files held keep it up wherever the
    // pointer goes
    dock.reveal
        .keep_out(menu_open || dock.drag.is_some() || dock.files.is_some());
    let progress = dock.reveal.progress(hold);
    let out = progress > 0.001 || dock.reveal.wanted();
    if out && !dock.out {
        dock.swell = anim(&swell_anim(motion), 0.0);
    }
    dock.out = out;

    let laid = lay_out(dock, entries, at, motion);
    spring_load(dock, &laid, spaces, motion);
    let sunk = at.sunk(progress);
    let top = at.top + sunk;

    // where the pointer is along the row at rest, while it's over the card
    // or the icons swelling above it
    // or nowhere while it holds an icon, which needs the row still
    let pointing = dock.pointer.filter(|_| held.is_none()).and_then(|(x, y)| {
        let over = y >= at.top - at.headroom && y <= at.top + at.card && progress > 0.5;
        over.then(|| along(at, laid.width, x))
    });
    dock.swell.to(if pointing.is_some() { 1.0 } else { 0.0 });
    if pointing.is_some() {
        dock.last_at = pointing;
    }
    let strength = dock.swell.value();
    let pointer_at = pointing.or(dock.last_at);

    let swollen: Swollen = magnify::swell(
        &laid.rests,
        at.side,
        at.pitch,
        pointer_at,
        strength,
        motion.magnify.as_ref(),
    );
    let card_x = at.card_left(laid.width) - swollen.before;
    let card_w = laid.width + swollen.before + swollen.after;
    let start = at.card_left(laid.width) + at.pad;

    let holding = held.is_some();
    let mut layers: Vec<Box<dyn Widget>> =
        vec![boxed(still_layer(output, &laid, spaces, at, holding))];
    let mut frost = Vec::new();

    if progress > 0.001 {
        // the card's glass
        let glass = Rectangle::new()
            .width(card_w.max(1.0))
            .height(at.card)
            .radius(RADIUS)
            .fill(glass_region::tint());
        layers.push(place(
            glass_region::lit(glass, RADIUS),
            card_x,
            top,
            progress,
        ));
        frost.push(glass_region::module(
            card_x, top, card_w, at.card, RADIUS, 1.0,
        ));

        // the workspace showing, lit behind its icons
        if let Some(active) = sections.iter().position(|section| section.active) {
            let span = laid
                .entries
                .iter()
                .zip(&swollen.icons)
                .filter(|(entry, _)| entry.section == active && entry.pointable())
                .fold(None, |span: Option<(f32, f32)>, (_, icon)| {
                    let (x0, x1) = (icon.x, icon.x + icon.side.max(at.side));
                    Some(span.map_or((x0, x1), |(a, b)| (a.min(x0), b.max(x1))))
                });
            if let Some((x0, x1)) = span {
                let inset = at.pad / 2.0;
                layers.push(place(
                    Rectangle::new()
                        .width(x1 - x0 + inset * 2.0)
                        .height(at.card - inset * 2.0)
                        .radius(RADIUS - inset)
                        .fill(sys::color::state::hover()),
                    start + x0 - inset,
                    top + inset,
                    progress,
                ));
            }
        }

        // the icon held files would open with, lit behind
        let opening = dock
            .files
            .as_ref()
            .filter(|files| files.opens)
            .and_then(|files| files.icon.as_ref())
            .and_then(|(key, _)| laid.entries.iter().position(|entry| entry.key == *key));
        if let Some(index) = opening {
            let icon = swollen.icons[index];
            let inset = at.pad / 2.0;
            layers.push(place(
                Rectangle::new()
                    .width(icon.side + inset * 2.0)
                    .height(icon.side + inset * 2.0)
                    .radius(icon.side / 4.0 + inset)
                    .fill(sys::color::state::selected()),
                start + icon.x - inset,
                top + at.pad + at.side - icon.side - icon.lift - inset,
                progress,
            ));
        }

        // the icons, the rules and the empty places
        let mut icons = Vec::new();
        for ((entry, icon), shown) in laid.entries.iter().zip(&swollen.icons).zip(&laid.shown) {
            let x = start + icon.x;
            match &entry.kind {
                // the held icon leaves its place empty
                Kind::Icon(_) if held.as_ref().is_some_and(|held| held.key == entry.key) => {}
                Kind::Icon(app) => icons.extend(icon_layers(app, icon, x, top, at, progress)),
                Kind::Rule => icons.push(place(
                    Divider::vertical(at.side),
                    x + sys::space::gap::s() - at.gap,
                    top + at.pad,
                    progress * shown,
                )),
                Kind::Empty if icon.side >= 1.0 => icons.push(place(
                    Rectangle::new()
                        .width(icon.side)
                        .height(icon.side)
                        .radius(icon.side / 4.0)
                        .fill(sys::color::state::hover()),
                    x,
                    top + at.pad + at.side - icon.side - icon.lift,
                    progress,
                )),
                Kind::Empty => {}
            }
        }
        // the icons slide down under the card's bottom edge and out
        layers.push(boxed(
            Rectangle::new()
                .width(Parent)
                .height((top + at.card).max(1.0))
                .fill(Color::TRANSPARENT)
                .align_child(Start, Start)
                .clip()
                .child(Stack::new(icons).width(Parent).height(Parent)),
        ));

        // the held icon, under the pointer; a pinned app pulled off fades,
        // as it comes out of the dock when let go there
        if let Some(held) = &held {
            let faded = if held.off && matches!(held.icon, Icon::Pin(_)) {
                0.5
            } else {
                1.0
            };
            layers.push(place(
                item::icon(&held.icon, at.side),
                held.x,
                held.y,
                progress * faded,
            ));
        }

        // the name of the icon under the pointer
        let under = pointing
            .and_then(|along| laid.under(along))
            .and_then(|index| match &laid.entries[index].kind {
                Kind::Icon(icon) => Some((index, icon)),
                _ => None,
            });
        match (motion.tooltip, under) {
            (Some(tooltip), Some((index, icon))) => {
                let key = &laid.entries[index].key;
                if dock.tip_for.as_ref() != Some(key) {
                    dock.tip_for = Some(key.clone());
                    dock.tip = anim(&tooltip, 0.0);
                }
                dock.tip.to(1.0);
                let placed = swollen.icons[index];
                let (label, width, height) = item::label(&item::name(icon));
                let x = (start + placed.x + placed.side / 2.0 - width / 2.0)
                    .clamp(0.0, (at.width - width).max(0.0));
                let y = top + at.pad + at.side
                    - placed.side
                    - placed.lift
                    - sys::space::gap::s()
                    - height;
                let shown = dock.tip.value() * progress;
                frost.push(glass_region::module(
                    x,
                    y,
                    width,
                    height,
                    sys::radius::raised(),
                    shown,
                ));
                layers.push(place(label, x, y, shown));
            }
            _ => dock.tip_for = None,
        }
    }

    // the frost comes and goes with the card, where the compositor can
    // fade it; elsewhere it shows whole while any of the card does
    let frost = if progress <= 0.001 {
        Vec::new()
    } else if amane::blur_fades() {
        glass_region::faded(frost, progress)
    } else {
        frost
    };

    let shown = InputArea {
        x: card_x.floor() as i32,
        y: 0,
        width: card_w.ceil().max(1.0) as i32,
        height: at.height as i32,
    };
    let input = dock
        .reveal
        .input_region(at.width, at.height, STRIP, progress, shown);

    View {
        root: Stack::new(layers).width(Parent).height(Parent),
        frost,
        input,
    }
}

/// The swell's motion, or one that's never used without magnification.
fn swell_anim(motion: &DockMotion) -> motion::Anim {
    motion
        .magnify
        .map_or(motion.reveal, |magnify| magnify.hover)
}

/// The row's places at rest, stepping their slides and growth, and the
/// card's width for them; forgets the places that have gone.
fn lay_out(dock: &mut Dock, entries: Vec<Entry>, at: Measures, motion: &DockMotion) -> Laid {
    dock.items
        .retain(|key, _| entries.iter().any(|entry| entry.key == *key));
    // the first row it lays out is simply there
    let fresh = dock.width.is_none();

    let mut slot = 0.0;
    let mut rests = Vec::with_capacity(entries.len());
    let mut shown = Vec::with_capacity(entries.len());
    let mut length: f32 = 0.0;
    for entry in &entries {
        let item = dock.items.entry(entry.key.clone()).or_insert_with(|| Item {
            slot: anim(&motion.item, slot),
            presence: anim(&motion.item, if fresh { 1.0 } else { 0.0 }),
        });
        item.slot.to(slot);
        item.presence.to(1.0);
        let presence = item.presence.value().clamp(0.0, 1.5);
        let rest = Rest {
            slot: item.slot.value(),
            // a rule only fades, it never swells
            presence: if entry.pointable() { presence } else { 0.0 },
        };
        length = length.max(rest.slot + entry.width * presence);
        rests.push(rest);
        shown.push(presence.min(1.0));
        slot += entry.width;
    }

    let target = at.card_width(length);
    let width = dock
        .width
        .get_or_insert_with(|| anim(&motion.resize, target));
    width.to(target);
    Laid {
        entries,
        rests,
        shown,
        width: width.value(),
    }
}

/// How many slots along the row of a card `width` wide the surface's `x`
/// is, the gap before each icon counting as its own.
fn along(at: Measures, width: f32, x: f32) -> f32 {
    let start = at.card_left(width) + at.pad;
    (x - start + at.gap / 2.0) / at.pitch
}

/// One icon as drawn, with its running dot: `x` is its left edge on the
/// surface, `top` the card's top, `shown` how much of it shows.
fn icon_layers(
    app: &Icon,
    icon: &magnify::Placed,
    x: f32,
    top: f32,
    at: Measures,
    shown: f32,
) -> Vec<Box<dyn Widget>> {
    if icon.side < 1.0 {
        return Vec::new();
    }
    // swollen icons grow up from the card's bottom padding
    let y = top + at.pad + at.side - icon.side - icon.lift;
    let mut layers = vec![place(item::icon(app, icon.side), x, y, shown)];

    let (running, width) = item::running(app);
    if width > 0.0 {
        let dot_y = top + at.card - at.pad / 2.0 - item::RUNNING / 2.0;
        layers.push(place(
            item::frame(width, item::RUNNING).child(Row::new(vec![running])),
            x + icon.side / 2.0 - width / 2.0,
            dot_y,
            shown,
        ));
    }
    layers
}

/// The still layer over the whole surface that takes the pointer for every
/// icon: it never moves or changes, and comes first, so the hover it's
/// told of and the targets after it stay put while the icons swell (amane
/// finds a hovered target by its place in the frame).
fn still_layer(
    output: &str,
    laid: &Laid,
    spaces: &[Space],
    at: Measures,
    holding: bool,
) -> Rectangle {
    let (hover_on, move_on, click_on) =
        (output.to_string(), output.to_string(), output.to_string());
    let (drag_on, release_on) = (output.to_string(), output.to_string());
    let (files_on, drop_on) = (output.to_string(), output.to_string());
    let pressed = laid.clone();
    let (held_over, dropped_on) = (laid.clone(), laid.clone());
    let drop_spaces = spaces.to_vec();
    // what a click needs of the row
    let laid = laid.clone();
    let spaces = spaces.to_vec();

    let layer = blank()
        .on_hover(move |over| {
            with_dock(&hover_on, |dock| {
                dock.reveal.hover(over);
                if !over {
                    dock.pointer = None;
                }
            });
        })
        .on_move(move |point: Point| {
            with_dock(&move_on, |dock| {
                dock.pointer = Some((point.x, point.y));
                // a pointer that arrived without an enter still counts
                dock.reveal.hover(true);
            });
        })
        .on_drag(move |point: Point| {
            with_dock(&drag_on, |dock| match &mut dock.drag {
                Some(drag) => drag.to((point.x, point.y)),
                None => dock.drag = take(&pressed, (point.x, point.y), at).filter(|_| dock.out),
            });
            mark_all();
        })
        .on_release(move |point: Point| {
            with_dock(&release_on, |dock| let_go(dock, point.y, at));
            mark_all();
        })
        .on_files_over(move |point, files| {
            let motion = motion::motion();
            with_dock(&files_on, |dock| {
                files_over(dock, &held_over, point, files, at, motion)
            })
        })
        .on_drop(move |files| {
            with_dock(&drop_on, |dock| {
                let_files_go(dock, &dropped_on, &drop_spaces, &files, at)
            });
        })
        .on_click(move |button| {
            let pointer = with_dock(&click_on, |dock| {
                // the button let go after a drag isn't a click
                if std::mem::take(&mut dock.dragged) {
                    return None;
                }
                dock.pointer.filter(|_| dock.out)
            });
            let Some((x, y)) = pointer else {
                return;
            };
            if y < at.top - at.headroom || y > at.top + at.card {
                return;
            }
            let Some(index) = laid.under(along(at, laid.width, x)) else {
                return;
            };
            if let Kind::Icon(icon) = &laid.entries[index].kind {
                click(&click_on, icon, &spaces, button, (x, y));
            }
        });
    if holding {
        layer.cursor(Cursor::Grabbing)
    } else {
        layer
    }
}

/// The icon at `at` on the surface, and its key.
fn icon_at(laid: &Laid, at: (f32, f32), measures: Measures) -> Option<(String, Icon)> {
    let (x, y) = at;
    if y < measures.top - measures.headroom || y > measures.top + measures.card {
        return None;
    }
    let index = laid.under(along(measures, laid.width, x))?;
    match &laid.entries[index].kind {
        Kind::Icon(icon) => Some((laid.entries[index].key.clone(), icon.clone())),
        _ => None,
    }
}

/// Whether `icon`'s app opens every one of `files`.
fn opens(icon: &Icon, files: &[PathBuf]) -> bool {
    !files.is_empty()
        && app_icon::entry_for(icon.app_id())
            .is_some_and(|app| files.iter().all(|file| app.opens(file)))
}

/// Files from another program held at `point` (none: gone) over the
/// dock: they swell the icons as the pointer does, and can be let go on
/// an icon whose app opens them.
fn files_over(
    dock: &mut Dock,
    laid: &Laid,
    point: Option<Point>,
    files: &[PathBuf],
    at: Measures,
    motion: &DockMotion,
) -> bool {
    let Some(point) = point else {
        dock.files = None;
        dock.pointer = None;
        return false;
    };
    let point = (point.x, point.y);
    dock.files_at = Some(point);
    dock.pointer = Some(point);
    let out = dock.out;
    let held = dock.files.get_or_insert_with(FilesOver::default);
    // still coming up from the edge
    let under = icon_at(laid, point, at).filter(|_| out);
    let Some((key, icon)) = under else {
        *held = FilesOver::default();
        return false;
    };
    if held.icon.as_ref().map(|(over, _)| over) != Some(&key) {
        held.icon = Some((key, Instant::now()));
        held.sprung = false;
        held.opens = opens(&icon, files);
        if let Some(delay) = motion.spring_load {
            redraw_after(delay + Duration::from_millis(10));
        }
    }
    held.opens
}

/// Files held over one icon long enough bring its window into view, so
/// they can be let go in it; a pinned app that isn't open opens.
fn spring_load(dock: &mut Dock, laid: &Laid, spaces: &[Space], motion: &DockMotion) {
    let (Some(delay), Some(held)) = (motion.spring_load, dock.files.as_mut()) else {
        return;
    };
    let Some((key, since)) = &held.icon else {
        return;
    };
    if held.sprung || since.elapsed() < delay {
        return;
    }
    held.sprung = true;
    let icon = laid.entries.iter().find_map(|entry| match &entry.kind {
        Kind::Icon(icon) if entry.key == *key => Some(icon.clone()),
        _ => None,
    });
    match icon {
        Some(Icon::Window(window)) => Windows::focus(window.id),
        Some(Icon::Pin(pin)) => {
            let windows: Vec<Win> = Windows::read().list().iter().map(Win::from).collect();
            let recent = RECENT.with_borrow(Clone::clone);
            match model::latest(&pin.app_id, &windows, &recent) {
                Some(id) => Windows::focus(id),
                None => {
                    if let Some(workspace) = pin_workspace(&pin, spaces) {
                        pins::open(&pin.app_id, workspace);
                    }
                }
            }
        }
        None => {}
    }
}

/// `files` let go over the dock: opened with the app of the icon they
/// were on, on its workspace, if it opens them.
fn let_files_go(dock: &mut Dock, laid: &Laid, spaces: &[Space], files: &[PathBuf], at: Measures) {
    let Some(point) = dock.files_at.take() else {
        return;
    };
    let Some((_, icon)) = icon_at(laid, point, at) else {
        return;
    };
    if !opens(&icon, files) {
        return;
    }
    let workspace = match &icon {
        Icon::Window(window) => window.workspace,
        Icon::Pin(pin) => pin_workspace(pin, spaces),
    };
    if let Some(workspace) = workspace {
        pins::open_with(icon.app_id(), workspace, files);
    }
}

/// The icon the button went down on at `at`, held from where the pointer
/// took it, if the press was on one.
fn take(laid: &Laid, at: (f32, f32), measures: Measures) -> Option<Drag> {
    let (x, y) = at;
    if y < measures.top - measures.headroom || y > measures.top + measures.card {
        return None;
    }
    let index = laid.under(along(measures, laid.width, x))?;
    let Kind::Icon(icon) = &laid.entries[index].kind else {
        return None;
    };
    let left =
        measures.card_left(laid.width) + measures.pad + laid.rests[index].slot * measures.pitch;
    let top = measures.top + measures.pad;
    let side = measures.side;
    Some(Drag {
        key: laid.entries[index].key.clone(),
        icon: icon.clone(),
        grab: ((x - left).clamp(0.0, side), (y - top).clamp(0.0, side)),
        pressed: at,
        pointer: at,
        moved: false,
    })
}

/// A click on `icon` at `at` on monitor `output`'s dock.
fn click(output: &str, icon: &Icon, spaces: &[Space], button: Button, at: (f32, f32)) {
    let at = (at.0 as i32, at.1 as i32);
    match (icon, button) {
        (Icon::Window(window), Button::Left) => Windows::focus(window.id),
        (Icon::Window(window), Button::Right) => {
            MENU_ON.set(Some(output.to_string()));
            open_window_menu(window, spaces, DOCK_NS, at);
        }
        (Icon::Pin(pin), Button::Left) => {
            let windows: Vec<Win> = Windows::read().list().iter().map(Win::from).collect();
            let recent = RECENT.with_borrow(Clone::clone);
            match model::latest(&pin.app_id, &windows, &recent) {
                Some(id) => Windows::focus(id),
                None => {
                    if let Some(workspace) = pin_workspace(pin, spaces) {
                        pins::open(&pin.app_id, workspace);
                    }
                }
            }
        }
        (Icon::Pin(pin), Button::Right) => {
            if let Some(workspace) = pin_workspace(pin, spaces) {
                MENU_ON.set(Some(output.to_string()));
                open_pin_menu(pin, workspace, DOCK_NS, at);
            }
        }
        _ => {}
    }
}

/// The workspace `pin` opens its app on: its own, or, while that isn't
/// open, the empty one niri keeps after the last on its monitor.
fn pin_workspace(pin: &pins::Pin, spaces: &[Space]) -> Option<i64> {
    model::resolve(pin, spaces)
        .or_else(|| {
            spaces
                .iter()
                .filter(|space| space.output == pin.output)
                .max_by_key(|space| space.index)
        })
        .map(|space| space.id)
}

/// `widget` with its top-left corner at (`x`, `y`) on the surface, at
/// `opacity`.
fn place(widget: Rectangle, x: f32, y: f32, opacity: f32) -> Box<dyn Widget> {
    boxed(
        Rectangle::new()
            .width(Parent)
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Start, Start)
            .child(widget.translate(x, y).opacity(opacity.clamp(0.0, 1.0))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion::DOCK;

    fn monitor() -> Monitor {
        Monitor {
            name: String::from("eDP-1"),
            width: 1600,
            height: 1000,
        }
    }

    #[test]
    fn hidden_the_card_sits_wholly_under_the_bottom_edge() {
        let at = Measures::new(&monitor(), &DOCK, 3, 1);
        assert_eq!(at.sunk(1.0), 0.0);
        assert!(at.top + at.sunk(0.0) >= at.height);
    }

    #[test]
    fn the_card_is_centred_and_grows_by_a_pitch_an_icon() {
        let at = Measures::new(&monitor(), &DOCK, 3, 1);
        let one = at.card_width(1.0);
        let two = at.card_width(2.0);
        assert!((two - one - at.pitch).abs() < 0.001);
        assert!((at.card_left(one) * 2.0 + one - 1600.0).abs() < 0.001);
    }

    #[test]
    fn the_pointer_over_an_icon_is_in_its_slot() {
        let at = Measures::new(&monitor(), &DOCK, 3, 0);
        let width = at.card_width(3.0);
        let first = at.card_left(width) + at.pad;
        // an icon's centre is half way along its slot, its gap counted in
        assert!((along(at, width, first + at.side / 2.0) - 0.5).abs() < 0.01);
        let third = first + at.pitch * 2.0 + 1.0;
        assert_eq!(along(at, width, third).floor(), 2.0);
    }

    #[test]
    fn too_many_icons_shrink_to_fit_the_monitor_and_the_surface_keeps_its_size() {
        let few = Measures::new(&monitor(), &DOCK, 5, 1);
        let many = Measures::new(&monitor(), &DOCK, 60, 4);
        assert_eq!(few.side, ICON);
        assert!(many.side < ICON);
        let slots = 60.0 + 4.0 * many.rule;
        assert!(many.card_width(slots) <= 1600.0);
        assert_eq!(few.height, many.height);
        assert_eq!(
            few.top + few.card,
            many.top + many.card,
            "the card's foot stays put"
        );
    }

    #[test]
    fn a_rule_sits_between_sections_and_the_pointer_is_never_over_it() {
        let section = |workspace: i64, icons: Vec<Icon>| Section {
            workspace: Some(workspace),
            index: workspace as u32,
            name: None,
            active: false,
            icons,
        };
        let pin = |app: &str| Icon::Pin(pins::Pin::new(app, "eDP-1", None, 1));
        let sections = [section(1, vec![pin("a")]), section(2, vec![])];
        let entries = row(&sections, 0.5);
        let kinds: Vec<&str> = entries
            .iter()
            .map(|entry| match entry.kind {
                Kind::Icon(_) => "icon",
                Kind::Rule => "rule",
                Kind::Empty => "empty",
            })
            .collect();
        assert_eq!(kinds, ["icon", "rule", "empty"]);

        let laid = Laid {
            rests: [0.0, 1.0, 1.5]
                .iter()
                .map(|&slot| Rest {
                    slot,
                    presence: 1.0,
                })
                .collect(),
            shown: vec![1.0; 3],
            width: 0.0,
            entries,
        };
        assert_eq!(laid.under(0.5), Some(0));
        assert_eq!(laid.under(1.2), None);
        assert_eq!(laid.under(1.6), Some(2));
    }

    #[test]
    fn without_magnification_or_a_name_there_is_no_room_above_for_them() {
        let plain = DockMotion {
            magnify: None,
            tooltip: None,
            ..DOCK
        };
        let at = Measures::new(&monitor(), &plain, 3, 0);
        assert!((at.headroom - sys::space::gap::s()).abs() < 0.001);
    }
}
