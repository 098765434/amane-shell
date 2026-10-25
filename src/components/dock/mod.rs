//! Dock — the macOS Big Sur dock, for the apps open on each workspace.
//!
//! One frosted glass card over the bar, on every monitor, holding an icon
//! for each app open on a workspace (one icon an app, a dot under it for
//! each of its windows) and, at its left end, a column of dots, one for
//! each of the monitor's workspaces. Two fingers up or down on a touchpad
//! slide the icons to another workspace's, a wheel steps there a notch at a
//! time, and a dot goes straight there; none of them changes the workspace
//! niri shows, only the one the dock shows. Clicking an icon focuses the
//! app's latest window, and again goes round its windows; a right-click
//! opens the window's menu.
//!
//! It hides below the bottom edge ([`EdgeReveal`]) and comes up when the
//! pointer touches the edge, on the workspace that's showing; it goes back
//! down a moment after the pointer leaves. While niri's overview opens it
//! rises in step with the zoom. The icons swell under the pointer as on
//! macOS ([`magnify`]), with the app's name over the one under it.
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

mod item;
mod magnify;
mod model;
pub mod motion;
mod pager;
mod window_menu;

use std::cell::RefCell;
use std::collections::HashMap;

use amane::{
    Animation, BlurShape, Button, Color, Full, InputArea, Keyboard, Layer, LayerWindow, Monitor,
    Overview, Parent, Point, Rectangle, Row, Scroll, Service, Stack, Start, Vertical, Widget,
    Windows, Workspaces, Zone,
};

use crate::components::divider::Divider;
use crate::components::edge_reveal::EdgeReveal;
use crate::components::glass_region;
use crate::components::layout::{blank, boxed};
use crate::components::page_dots::PageDots;
use crate::tokens::sys;

use magnify::{Rest, Swollen};
use model::{App, Page, Space, Win};
use motion::{DockMotion, anim};
use pager::Pager;
use window_menu::open_window_menu;

/// The dock's layer-surface namespace; the niri `layer-rule` matches it
/// (a clear surface that says where to frost), and a window's menu hangs
/// under it.
pub const DOCK_NS: &str = "amane.dock";

// TODO(token): `dock.icon.size`, `dock.radius` and `dock.reveal.strip`
// once Penpot has them (manual §1.1).
/// How big an icon is at rest.
const ICON: f32 = 44.0;
/// The card's corners.
const RADIUS: f32 = 18.0;
/// How tall the strip along the bottom edge that brings the dock up is.
const STRIP: f32 = 2.0;

/// One monitor's dock, kept across rebuilds.
struct Dock {
    reveal: EdgeReveal,
    pager: Pager,
    // 0 … 1: the swell coming on with the pointer and going with it
    swell: Animation,
    // the pointer on the surface
    pointer: Option<(f32, f32)>,
    // where the pointer last was along the row, for the swell to go down
    // where it was
    last_at: Option<f32>,
    // the name label coming in, for the app it's over
    tip: Animation,
    tip_for: Option<String>,
    // each icon's slot and growth, by workspace and app
    items: HashMap<(i64, String), Item>,
    // each page's card width at rest, by workspace
    widths: HashMap<i64, Animation>,
    // whether it was out last frame: coming out, it shows the workspace
    // that's showing
    out: bool,
}

/// An icon's motion: its slot along the row, and how far it has grown in.
struct Item {
    slot: Animation,
    presence: Animation,
}

impl Dock {
    fn new(motion: &DockMotion) -> Self {
        Self {
            reveal: EdgeReveal::new(anim(&motion.reveal, 0.0), motion.hide_delay),
            pager: Pager::new(motion),
            swell: anim(&swell_anim(motion), 0.0),
            pointer: None,
            last_at: None,
            tip: anim(&motion.tooltip.unwrap_or(motion.reveal), 0.0),
            tip_for: None,
            items: HashMap::new(),
            widths: HashMap::new(),
            out: false,
        }
    }
}

thread_local! {
    static DOCKS: RefCell<HashMap<String, Dock>> = RefCell::new(HashMap::new());

    // the windows by when they last had the focus, latest first, for which
    // of an app's windows a click brings up
    static RECENT: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
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

/// The dock's measures, from the tokens and its motion.
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
    // from the card's left edge to the first icon
    lead: f32,
    // how far over the card the swollen icons and the name reach
    headroom: f32,
}

impl Measures {
    fn new(monitor: &Monitor, motion: &DockMotion) -> Self {
        let side = ICON;
        let gap = sys::space::gap::xs();
        let pad = sys::space::inset::s();
        let card = side + pad * 2.0;
        let (scale, lift) = motion.magnify.map_or((1.0, 0.0), |magnify| magnify.most());
        let tip = if motion.tooltip.is_some() {
            item::label_height() + sys::space::gap::s()
        } else {
            0.0
        };
        let headroom = side * (scale - 1.0) + lift + tip + sys::space::gap::s();
        // over the bar, the gap a popover keeps from it
        let below = sys::size::panel::height() + sys::space::gap::m();
        let dots = dots_width();
        Self {
            width: monitor.width as f32,
            height: (headroom + card + below).ceil(),
            top: headroom,
            card,
            side,
            pitch: side + gap,
            gap,
            pad,
            lead: pad + dots + sys::space::gap::s() + Divider::width() + sys::space::gap::s(),
            headroom,
        }
    }

    /// How wide a card with icons running `slots` long is.
    fn card_width(&self, slots: f32) -> f32 {
        let row = (slots * self.pitch - self.gap).max(0.0);
        self.lead + row + self.pad
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

/// How wide the column of workspace dots is.
fn dots_width() -> f32 {
    PageDots::new(1, 0.0).vertical().size().0
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The dock on `monitor`. Registered with `App::window_per_monitor`.
pub fn surface(monitor: &Monitor) -> LayerWindow {
    let motion = motion::motion();
    let at = Measures::new(monitor, motion);

    let windows: Vec<Win> = Windows::read().list().iter().map(Win::from).collect();
    let focused = windows.iter().find(|window| window.focused).map(|w| w.id);
    RECENT.with_borrow_mut(|recent| model::remember(recent, focused, &windows));
    let spaces: Vec<Space> = Workspaces::read().list().iter().map(Space::from).collect();
    let pages = model::pages(&spaces, &windows, &monitor.name);
    let hold = if motion.follow_overview {
        Overview::read().progress() as f32
    } else {
        0.0
    };

    let name = monitor.name.clone();
    let view = with_dock(&monitor.name, |dock| {
        build(dock, &name, &pages, hold, at, motion)
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

/// What one frame of the dock draws, and where it takes the pointer.
struct View {
    root: Stack,
    frost: Vec<BlurShape>,
    input: Vec<InputArea>,
}

/// One page laid out: its workspace, its apps, their icons at rest, and
/// the card's width for them at rest.
struct Laid<'a> {
    page: &'a Page,
    rests: Vec<Rest>,
    width: f32,
}

fn build(
    dock: &mut Dock,
    output: &str,
    pages: &[Page],
    hold: f32,
    at: Measures,
    motion: &'static DockMotion,
) -> View {
    let count = pages.len();
    let progress = dock.reveal.progress(hold);
    let out = progress > 0.001 || dock.reveal.wanted();

    // coming out, it shows the workspace that's showing
    let active = pages.iter().position(|page| page.active);
    if out && !dock.out {
        dock.pager.jump(active.unwrap_or(0), motion);
        dock.swell = anim(&swell_anim(motion), 0.0);
    }
    dock.out = out;
    if dock.pager.page() >= count && count > 0 {
        dock.pager.jump(count - 1, motion);
    }
    dock.pager.settle_stale(count, motion);
    let offset = if count == 0 {
        0.0
    } else {
        dock.pager.offset(count, motion)
    };

    let laid = lay_out(dock, pages, at, motion);

    // the page the pointer is over: the one nearest
    let near = (offset.round().max(0.0) as usize).min(count.saturating_sub(1));
    let sunk = at.sunk(progress);
    let top = at.top + sunk;

    // where the pointer is along the near page's row at rest, while it's
    // over the card or the icons swelling above it
    let pointing = dock.pointer.and_then(|(x, y)| {
        let page = laid.get(near)?;
        let over = y >= at.top - at.headroom && y <= at.top + at.card && progress > 0.5;
        over.then(|| along(at, page.width, x))
    });
    dock.swell.to(if pointing.is_some() { 1.0 } else { 0.0 });
    if pointing.is_some() {
        dock.last_at = pointing;
    }
    let strength = dock.swell.value();
    let pointer_at = pointing.or(dock.last_at);

    // each page's icons as drawn, the near one swollen
    let swollen: Vec<Swollen> = laid
        .iter()
        .enumerate()
        .map(|(index, page)| {
            magnify::swell(
                &page.rests,
                at.side,
                at.pitch,
                (index == near).then_some(pointer_at).flatten(),
                strength,
                motion.magnify.as_ref(),
            )
        })
        .collect();

    // the card, between the pages either side of where it is
    let card = |index: usize| -> (f32, f32) {
        let (Some(page), Some(row)) = (laid.get(index), swollen.get(index)) else {
            return (at.card_left(at.card_width(0.0)), at.card_width(0.0));
        };
        (
            at.card_left(page.width) - row.before,
            page.width + row.before + row.after,
        )
    };
    let below = offset.floor().max(0.0) as usize;
    let part = (offset - below as f32).clamp(0.0, 1.0);
    let (x0, w0) = card(below.min(count.saturating_sub(1)));
    let (x1, w1) = card((below + 1).min(count.saturating_sub(1)));
    let card_x = x0 + (x1 - x0) * part;
    let card_w = w0 + (w1 - w0) * part;

    let mut layers: Vec<Box<dyn Widget>> =
        vec![boxed(still_layer(output, &laid, near, at, count, motion))];
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

        // the workspaces down its left end
        let dots = PageDots::new(count, offset)
            .vertical()
            .marked(active)
            .fit(at.side);
        let dots_h = dots.size().1;
        let pick_on = output.to_string();
        let dots = dots.on_pick(move |page| {
            with_dock(&pick_on, |dock| dock.pager.go(page, count, motion));
        });
        layers.push(place(
            item::frame(dots_width(), dots_h).child(Row::new(vec![dots.build()])),
            card_x + at.pad,
            top + (at.card - dots_h) / 2.0,
            progress,
        ));
        let rule_x = card_x + at.pad + dots_width() + sys::space::gap::s();
        layers.push(place(
            Divider::vertical(at.side),
            rule_x,
            top + at.pad,
            progress,
        ));

        // the pages near where it is, sliding past the card
        let mut icons = Vec::new();
        for (index, page) in laid.iter().enumerate() {
            let apart = index as f32 - offset;
            if apart.abs() >= 1.0 {
                continue;
            }
            let shown = (1.0 - apart.abs()) * progress;
            let start = at.card_left(page.width) + at.lead;
            for (app, icon) in page.page.apps.iter().zip(&swollen[index].icons) {
                icons.extend(icon_layers(app, icon, start, top, apart, at, shown));
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

        // the name of the app under the pointer
        let under = pointing
            .and_then(|along| {
                laid.get(near)
                    .and_then(|page| magnify::under(&page.rests, along))
            })
            .and_then(|index| Some((laid.get(near)?.page.apps.get(index)?, index)));
        match (motion.tooltip, under) {
            (Some(_), Some((app, index))) if !dock.pager.swiping() => {
                if dock.tip_for.as_deref() != Some(app.key.as_str()) {
                    dock.tip_for = Some(app.key.clone());
                    dock.tip = anim(&motion.tooltip.unwrap_or(motion.reveal), 0.0);
                }
                dock.tip.to(1.0);
                let icon = swollen[near].icons[index];
                let start = at.card_left(laid[near].width) + at.lead;
                let (label, width, height) = item::label(&item::name(app));
                let x = start + icon.x + icon.side / 2.0 - width / 2.0;
                let y =
                    top + at.pad + at.side - icon.side - icon.lift - sys::space::gap::s() - height;
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

/// Each page's icons at rest, stepping their slides and growth, and the
/// card's width for them; forgets the icons of apps that have gone.
fn lay_out<'a>(
    dock: &mut Dock,
    pages: &'a [Page],
    at: Measures,
    motion: &DockMotion,
) -> Vec<Laid<'a>> {
    dock.items.retain(|(workspace, key), _| {
        pages
            .iter()
            .any(|page| page.workspace == *workspace && page.apps.iter().any(|app| app.key == *key))
    });
    dock.widths
        .retain(|workspace, _| pages.iter().any(|page| page.workspace == *workspace));

    pages
        .iter()
        .map(|page| {
            // a page seen for the first time is simply there
            let fresh = !dock.widths.contains_key(&page.workspace);
            let rests: Vec<Rest> = page
                .apps
                .iter()
                .enumerate()
                .map(|(slot, app)| {
                    let slot = slot as f32;
                    let item = dock
                        .items
                        .entry((page.workspace, app.key.clone()))
                        .or_insert_with(|| Item {
                            slot: anim(&motion.item, slot),
                            presence: anim(&motion.item, if fresh { 1.0 } else { 0.0 }),
                        });
                    item.slot.to(slot);
                    item.presence.to(1.0);
                    Rest {
                        slot: item.slot.value(),
                        presence: item.presence.value().clamp(0.0, 1.5),
                    }
                })
                .collect();
            let slots = rests
                .iter()
                .map(|rest| rest.slot + rest.presence)
                .fold(0.0, f32::max);
            let target = at.card_width(slots);
            let width = dock
                .widths
                .entry(page.workspace)
                .or_insert_with(|| anim(&motion.resize, target));
            width.to(target);
            Laid {
                page,
                rests,
                width: width.value(),
            }
        })
        .collect()
}

/// How many slots along the row of a card `width` wide the surface's `x`
/// is, the gap before each icon counting as its own.
fn along(at: Measures, width: f32, x: f32) -> f32 {
    let start = at.card_left(width) + at.lead;
    (x - start + at.gap / 2.0) / at.pitch
}

/// One icon as drawn, with its running dots: `start` is where its page's
/// row starts, `top` the card's top, `apart` how many pages from where the
/// dock is, `shown` how much of it shows.
fn icon_layers(
    app: &App,
    icon: &magnify::Placed,
    start: f32,
    top: f32,
    apart: f32,
    at: Measures,
    shown: f32,
) -> Vec<Box<dyn Widget>> {
    if icon.side < 1.0 {
        return Vec::new();
    }
    let slide = apart * at.card;
    let x = start + icon.x;
    // swollen icons grow up from the card's bottom padding
    let y = top + at.pad + at.side - icon.side - icon.lift + slide;
    let mut layers = vec![place(item::icon(app, icon.side), x, y, shown)];

    let (running, width) = item::running(app);
    let dot_y = top + at.card - at.pad / 2.0 - item::RUNNING / 2.0 + slide;
    layers.push(place(
        item::frame(width.max(1.0), item::RUNNING).child(Row::new(vec![running])),
        x + icon.side / 2.0 - width / 2.0,
        dot_y,
        shown,
    ));
    layers
}

/// The still layer over the whole surface that takes the pointer for every
/// icon: it never moves or changes, and comes first, so the hover it's
/// told of and the targets after it stay put while the icons swell (amane
/// finds a hovered target by its place in the frame).
fn still_layer(
    output: &str,
    laid: &[Laid],
    near: usize,
    at: Measures,
    count: usize,
    motion: &'static DockMotion,
) -> Rectangle {
    let (hover_on, move_on, scroll_on, click_on) = (
        output.to_string(),
        output.to_string(),
        output.to_string(),
        output.to_string(),
    );
    // what a click needs of the page under the pointer
    let page = laid
        .get(near)
        .map(|page| (page.page.apps.clone(), page.rests.clone(), page.width));

    blank()
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
        .on_scroll(move |scroll: Scroll| {
            with_dock(&scroll_on, |dock| {
                if dock.out {
                    dock.pager.scroll(scroll, at.card, count, motion);
                }
            });
        })
        .on_click(move |button| {
            let Some((apps, rests, width)) = &page else {
                return;
            };
            let pointer = with_dock(&click_on, |dock| dock.pointer.filter(|_| dock.out));
            let Some((x, y)) = pointer else {
                return;
            };
            if y < at.top - at.headroom || y > at.top + at.card {
                return;
            }
            let Some(app) =
                magnify::under(rests, along(at, *width, x)).and_then(|index| apps.get(index))
            else {
                return;
            };
            click(app, button, (x, y));
        })
}

/// A click on `app`'s icon at `at` on the surface.
fn click(app: &App, button: Button, at: (f32, f32)) {
    let recent = RECENT.with_borrow(Clone::clone);
    let Some(id) = model::target(app, &recent) else {
        return;
    };
    match button {
        Button::Left => Windows::focus(id),
        Button::Right => {
            // the menu is for the window a click would bring up, or the
            // focused one when it's this app's
            let id = app
                .windows
                .iter()
                .find(|window| window.focused)
                .map_or(id, |window| window.id);
            let floating = app
                .windows
                .iter()
                .find(|window| window.id == id)
                .is_some_and(|window| window.floating);
            open_window_menu(id, floating, DOCK_NS, (at.0 as i32, at.1 as i32));
        }
        _ => {}
    }
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
        let at = Measures::new(&monitor(), &DOCK);
        assert_eq!(at.sunk(1.0), 0.0);
        assert!(at.top + at.sunk(0.0) >= at.height);
    }

    #[test]
    fn the_card_is_centred_and_grows_by_a_pitch_an_icon() {
        let at = Measures::new(&monitor(), &DOCK);
        let one = at.card_width(1.0);
        let two = at.card_width(2.0);
        assert!((two - one - at.pitch).abs() < 0.001);
        assert!((at.card_left(one) * 2.0 + one - 1600.0).abs() < 0.001);
    }

    #[test]
    fn the_pointer_over_an_icon_is_in_its_slot() {
        let at = Measures::new(&monitor(), &DOCK);
        let width = at.card_width(3.0);
        let first = at.card_left(width) + at.lead;
        // an icon's centre is half way along its slot, its gap counted in
        assert!((along(at, width, first + at.side / 2.0) - 0.5).abs() < 0.01);
        let third = first + at.pitch * 2.0 + 1.0;
        assert_eq!(along(at, width, third).floor(), 2.0);
    }

    #[test]
    fn without_magnification_or_a_name_there_is_no_room_above_for_them() {
        let plain = DockMotion {
            magnify: None,
            tooltip: None,
            ..DOCK
        };
        let at = Measures::new(&monitor(), &plain);
        assert!((at.headroom - sys::space::gap::s()).abs() < 0.001);
    }
}
