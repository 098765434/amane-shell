//! Quick Settings — GNOME's quick settings menu in the macOS Control Center
//! look: one frosted card above the bar holding Control Center's modules,
//! each drilling into a page of its own.
//!
//! The card is iOS Control Center's canvas, four cells across and eight
//! down (`grid`), laid out as iOS lays it out by default: Wi‑Fi and
//! Bluetooth tiles beside Now Playing, the Focus tile and round buttons, a
//! Display card and a Sound card with their sliders, then the battery and
//! rows of round buttons; Edit Controls moves and resizes
//! them (`edit`, `layout`). It keeps GNOME's rules: a control shows only
//! when the machine has what it drives. A tile's page grows out of the tile
//! to fill the card, as on iOS. Escape goes back to the grid, then closes.
//!
//! As on macOS Tahoe, the card itself has no background: each module is a
//! frosted glass piece of its own, and the surface tells the compositor
//! where to frost (`glass_region`, over ext-background-effect) — the grid's
//! modules, or the whole card while a page is on it.
//!
//! The card has one height, the grid's (the rows in use, or all of them
//! in edit mode), for the grid and every page. A surface resized every
//! frame costs a compositor round-trip and a second draw each time; keeping
//! one size while anything moves is what makes the motion smooth. The
//! surface is resized only at rest, when the grid gains or loses a row, or
//! edit mode changes it.
//!
//! It's the one place for sound, Wi‑Fi, Bluetooth and power: the bar's
//! status icons (each optional, see the pages' "Show in Status Bar") open
//! straight to their page. It shares the notification center's
//! one-open-at-a-time state ([`Panel::QuickSettings`]), opened from the
//! bar's Control Center button, a status icon, or over IPC:
//! `quick-settings [toggle | open <page> | close]`. See
//! `docs/shell/quick-settings.md`.
//!
//! Edit mode opens the Customize window (`customize`) beside the card, as
//! macOS's Customize Control Center does. amane can't carry a drag from one
//! surface to another, so while editing this one surface covers the output
//! and holds both: the card where it always is, at the bottom-right, and
//! the window in the middle, with one still layer under both that every
//! drag is measured on. Clicks outside the two go through to what's under
//! them (`input_region`).

mod controls;
mod customize;
mod detail;
mod edit;
mod grid;
mod layout;
mod pages;
mod slider_card;
mod system_row;
mod tile;
mod tiles;

use std::cell::{Cell, RefCell};
use std::time::Instant;

use amane::{
    Animation, Blend, BlurShape, Color, End, Horizontal, InputArea, Key, Keyboard, LayerWindow,
    Parent, Rectangle, Size, Stack, Start, Vertical, Widget, WindowSize, window_size,
};

use crate::components::glass_region;
use crate::components::layout::{blank, boxed, pad, track_cursor};
use crate::components::material::Material;
use crate::components::motion;
use crate::components::panel::{self, Panel, close, is_open, overlay, popover_margin};
use crate::components::status_bar;
use crate::tokens::quick;

amane_kit::marker! {
    /// Quick Settings' own state changed (its page, editing, Customize):
    /// only its window draws again.
    pub QuickSettings
}

pub use controls::{ControlId, available as control_available, quick_toggle};
pub use pages::Page;
pub(crate) use tiles::keep_awake::status as keep_awake_status;

/// Layer-surface namespace; the niri `layer-rule` matches it for blur, and
/// the Bluetooth device menu hangs under it.
pub const QS_NS: &str = "amane.quick-settings";

thread_local! {
    // where it's going: the grid or a page
    static PAGE: Cell<Page> = const { Cell::new(Page::Grid) };

    // the page drawn over the grid, which stays while sliding back out
    static DETAIL: Cell<Page> = const { Cell::new(Page::Grid) };

    // 0 closed … 1 open: the card fades, swells and rises in over the bar
    static OPEN: RefCell<Animation> = RefCell::new(motion::pop(0.0));

    // whether OPEN is heading for 1
    static OPENING: Cell<bool> = const { Cell::new(false) };

    // when the card began opening from nothing onto the grid: its modules
    // arrive in a wave from then
    static OPENED_AT: Cell<Option<Instant>> = const { Cell::new(None) };

    // 0 the grid … 1 a page: the page grows out of its tile
    static SLIDE: RefCell<Animation> = RefCell::new(motion::zoom(0.0));

    // the surface's height last frame, kept while it closes
    static LAST_HEIGHT: Cell<f32> = const { Cell::new(1.0) };

    // the page sliding out under a new one, instead of the grid
    static SWAP: Cell<Page> = const { Cell::new(Page::Grid) };

    // how far the grid is from where its new height puts it: when the
    // grid gains or loses a row at rest, it slides there rather than jumps
    static SHIFT: RefCell<Animation> = RefCell::new(motion::spring(0.0));
    static LAST_INNER: Cell<f32> = const { Cell::new(0.0) };
}

// ───── runtime ───────────────────────────────────────────────────────────────

/// Opens on `page`, or goes there when open already.
pub fn open(page: Page) {
    if !is_open(Panel::QuickSettings) {
        panel::toggle(Panel::QuickSettings);
    }
    go(page);
}

/// Whether Quick Settings is open on `page`.
pub fn showing(page: Page) -> bool {
    is_open(Panel::QuickSettings) && PAGE.get() == page
}

/// Back to the grid.
pub fn back() {
    go(Page::Grid);
}

/// Goes to `page`: slides it in over the grid, or back out. From one page
/// to another, the old one recedes under the new as the grid would.
fn go(page: Page) {
    let previous = PAGE.replace(page);
    if previous == page {
        return;
    }
    pages::left(previous);
    // a page has no business over the canvas being edited
    if page != Page::Grid {
        edit::leave();
    }
    // opening straight onto a page shows it, without a slide
    let hidden = OPEN.with_borrow(|open| open.value() < 0.001);
    let swapping = page != Page::Grid && previous != Page::Grid && !hidden;
    SWAP.set(if swapping { previous } else { Page::Grid });
    if page != Page::Grid {
        DETAIL.set(page);
    }

    let target = if page == Page::Grid { 0.0 } else { 1.0 };
    SLIDE.with_borrow_mut(|slide| {
        if hidden {
            *slide = motion::zoom(target);
        } else if swapping {
            *slide = motion::zoom(0.0);
            slide.to(1.0);
        } else {
            slide.to(target);
        }
    });
    QuickSettings::changed();
}

/// Escape: lets go of a drag, then clears the Customize window's search,
/// then out of edit mode, back to the grid from a page, else close.
fn escape() {
    if edit::cancel() {
        return;
    }
    if edit::editing() {
        if customize::has_query() {
            customize::clear_query();
        } else {
            edit::leave();
        }
    } else if PAGE.get() == Page::Grid {
        close();
    } else {
        back();
    }
}

pub(crate) fn opened() {}

/// The panel closed: its page's searches stop now; the page itself, and
/// edit mode's canvas, stay drawn as they were while the card fades away.
pub(crate) fn closed() {
    pages::left(PAGE.get());
}

/// Gone from the screen: the next opening starts on the grid, out of edit
/// mode.
fn hidden() {
    if PAGE.replace(Page::Grid) != Page::Grid {
        // what the page drew while fading away may have started a search again
        pages::left(DETAIL.get());
    }
    DETAIL.set(Page::Grid);
    SWAP.set(Page::Grid);
    SLIDE.with_borrow_mut(|slide| *slide = motion::zoom(0.0));
    OPENED_AT.set(None);
    SHIFT.with_borrow_mut(|shift| *shift = motion::spring(0.0));
    LAST_INNER.set(0.0);
    edit::leave();
    customize::hidden();
    motion::forget(tiles::SCOPE);
}

// TODO(token): `quick.page.min-rows` once Penpot has it (manual §1.1).
/// How many rows tall a page is at least: a short grid would leave a
/// page too little room for its list.
const MIN_PAGE_ROWS: u8 = 5;

/// `amane-shell` IPC: `quick-settings [toggle | open <page> | close]`.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["toggle"] => panel::toggle(Panel::QuickSettings),
        ["open"] => open(Page::Grid),
        ["open", name] => match Page::named(name) {
            Some(page) if pages::available(page) => open(page),
            Some(_) => return format!("page {name} has nothing to show on this machine"),
            None => return format!("unknown page {name}; pages: {}", Page::NAMES.join(", ")),
        },
        ["close"] => {
            if is_open(Panel::QuickSettings) {
                close();
            }
        }
        // the Customize window opens with edit mode
        ["edit"] | ["customize"] => {
            open(Page::Grid);
            edit::enter();
        }
        // what Shell Settings' Quick Settings page lists: each control
        // this machine has, and whether it's on the canvas
        ["controls"] => {
            load_layout();
            return ControlId::ALL
                .into_iter()
                .filter(|id| controls::available(*id))
                .map(|id| {
                    let shown = layout::with(|layout| layout.rect_of(id).is_some());
                    format!(
                        "{}\t{}\n",
                        id.name(),
                        if shown { "shown" } else { "hidden" }
                    )
                })
                .collect();
        }
        [action @ ("show" | "hide"), name] => {
            let Some(id) = ControlId::named(name) else {
                let names: Vec<&str> = ControlId::ALL.into_iter().map(ControlId::name).collect();
                return format!("unknown control {name}; controls: {}", names.join(", "));
            };
            load_layout();
            let placed = layout::with(|layout| {
                let placed = if *action == "show" {
                    layout.show(id)
                } else {
                    layout.hide(id);
                    true
                };
                layout.save();
                placed
            });
            if !placed {
                QuickSettings::changed();
                return format!("no room for {name}: hide another control first");
            }
        }
        ["reset-layout"] => {
            let ctx = controls::Ctx::new(false);
            layout::replace(layout::Layout::default_for(grid::Canvas::tokens(), &|id| {
                controls::present(id, &ctx)
            }));
        }
        _ => {
            return String::from(
                "usage: quick-settings [toggle | open <page> | close | edit | customize | reset-layout | controls | (show | hide) <control>]",
            );
        }
    }
    QuickSettings::changed();
    String::from("ok")
}

/// The layout from its file, or packed with what this machine has, for an
/// IPC call that comes before the card has ever shown.
fn load_layout() {
    let ctx = controls::Ctx::new(false);
    layout::load_once(&|id| controls::present(id, &ctx));
}

// ───── view ──────────────────────────────────────────────────────────────────

/// Steps the card's opening towards `open` and gives how far it is, a
/// little past 1 on the spring. Closing is quicker than opening, and a
/// change of mind turns it around at the speed it had.
fn unfold(open: bool) -> f32 {
    OPEN.with_borrow_mut(|anim| {
        if OPENING.replace(open) != open {
            // opening from nothing onto the grid, the modules arrive in a
            // wave; frost that can only show or hide would pop in module by
            // module, and an edit or a page has better things to show
            let cascade = open
                && anim.value() < 0.05
                && PAGE.get() == Page::Grid
                && !edit::editing()
                && amane::blur_fades();
            if cascade {
                OPENED_AT.set(Some(Instant::now()));
            }
            anim.retime(if open { motion::POP } else { motion::CLOSE });
            anim.to(if open { 1.0 } else { 0.0 });
        }
        anim.value()
    })
}

/// The surface: hidden while closed, else the card, as tall as the grid.
pub fn surface() -> LayerWindow {
    QuickSettings::watch();
    let open = is_open(Panel::QuickSettings);
    let unfolded = unfold(open);
    let shown = open || unfolded > 0.001;

    if !shown {
        hidden();
        return window(surface_width(), 1.0)
            .visible(false)
            .blur_shapes(Vec::new())
            .child(blank());
    }

    // a page whose setting has gone gives way to the grid
    if PAGE.get() != Page::Grid && !pages::available(PAGE.get()) {
        back();
    }
    let page = PAGE.get();
    // the zoom's spring overshoots a little; what it draws at is clamped
    let slide_raw = SLIDE.with_borrow_mut(|slide| slide.value());
    let slide = slide_raw.clamp(0.0, 1.0);
    if slide_raw >= 0.999 {
        SWAP.set(Page::Grid);
    }
    let detail = if page == Page::Grid {
        DETAIL.get()
    } else {
        page
    };
    let zooming = detail != Page::Grid && (page != Page::Grid || slide > 0.001);
    let swap = SWAP.get();
    // the canvas sits a margin in from the surface's edge: room for the
    // modules' shadows
    let inset = quick::grid::margin();

    // the grid sets the card's size, so it's always built; only what's on
    // screen is drawn, so a page hidden behind the grid takes no clicks and
    // starts no searches. The tile a page grows out of fades into it, and
    // gives the page its frost.
    let grid = tiles::grid(tiles::Motion {
        opened: OPENED_AT.get(),
        focus: (zooming && swap == Page::Grid)
            .then(|| (detail.controls(), 1.0 - ramp(slide, 0.0, TILE_FADE))),
    });
    // in edit mode the Add list beside the canvas widens the card to the left
    let width = grid.width + inset * 2.0;
    let grid_w = grid::Metrics::tokens().extent(grid::Canvas::tokens().cols);
    // once a page has been on the card, it's at least a page tall, so the
    // surface isn't resized as pages come and go
    let page_min = if page != Page::Grid || DETAIL.get() != Page::Grid {
        grid::Metrics::tokens().extent(MIN_PAGE_ROWS)
    } else {
        0.0
    };
    let inner = grid.height.max(page_min).max(1.0);
    // the grid sits at the bottom of that
    let grid_top = inner - grid.height.max(1.0);

    // a change of height at rest: the grid starts where it was and slides
    // to where the new height puts it
    let shift = SHIFT.with_borrow_mut(|shift| {
        let last = LAST_INNER.replace(inner);
        if open && last > 0.0 && (inner - last).abs() > 0.5 {
            *shift = motion::spring(shift.value() + (inner - last));
            shift.to(0.0);
        }
        shift.value()
    });

    // Closing, the card keeps the size it had: a surface resized right
    // before it hides still has the compositor's answers to the resize on
    // their way, and amane draws on those after the hide, which niri rejects.
    // Nor does it resize while a control is held: the rows may change under
    // the drag, and the one size holds until the drop.
    let outer = if open && edit::drag().is_none() {
        (inner + inset * 2.0).max(1.0)
    } else {
        LAST_HEIGHT.get()
    };
    LAST_HEIGHT.set(outer);

    // the page fills the card; it grows out of its tile on the grid, or out
    // of the bar's corner when it was opened from the bar
    let full = Sheet {
        x: inset,
        y: inset,
        width: grid_w,
        height: inner,
        radius: quick::card::radius(),
        tint: Material::Popover.tint(),
    };
    let origin = match grid.rest(detail.controls()) {
        Some((slot, radius)) => Sheet {
            x: inset + slot.x,
            y: inset + grid_top + shift + slot.y,
            width: slot.width,
            height: slot.height,
            radius,
            tint: Material::Control.tint(),
        },
        None => {
            let cell = grid::Metrics::tokens().extent(1);
            Sheet {
                x: inset + grid_w - cell,
                y: inset + inner - cell,
                width: cell,
                height: cell,
                radius: cell / 2.0,
                tint: Material::Control.tint(),
            }
        }
    };

    let mut shapes: Vec<BlurShape> = Vec::new();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();

    if page == Page::Grid || slide < 0.999 {
        // the grid stays where it is and sinks back from the tile its page
        // grows out of, fading as the page covers it; from one page to
        // another, the old page sinks back in its place
        let mut frost: Vec<BlurShape> = Vec::new();
        let receding: Box<dyn Widget> = if swap != Page::Grid {
            frost.push(full.frost());
            boxed(sheet_layer(swap, full, inner, 1.0))
        } else {
            frost.extend(glass_region::transformed(
                grid.shapes,
                1.0,
                (0.0, 0.0),
                (inset, inset + grid_top + shift),
            ));
            boxed(
                Rectangle::new()
                    .width(Size::Fixed(grid.width))
                    .height(Size::Fixed(inner))
                    .fill(Color::TRANSPARENT)
                    .align_child(Start, End)
                    .translate(0.0, shift)
                    .child(Stack::new(vec![grid.widget])),
            )
        };
        let shrink = f32::blend(1.0, quick::page::recede(), slide_raw);
        // scaled about the origin's centre: a layer scales about its own,
        // then moves by how far apart the two are, scaled down
        let (cx, cy) = origin.centre();
        let moved = (
            (cx - width / 2.0) * (1.0 - shrink),
            (cy - outer / 2.0) * (1.0 - shrink),
        );
        let shows = 1.0 - ramp(slide, 0.0, GRID_FADE);
        // the frost sinks and fades with it
        shapes.extend(glass_region::faded(
            glass_region::transformed(frost, shrink, (cx, cy), (0.0, 0.0)),
            shows,
        ));
        layers.push(boxed(
            Rectangle::new()
                .width(Parent)
                .height(Size::Fixed(outer))
                .fill(Color::TRANSPARENT)
                .padding(pad(inset, inset))
                .align_child(Start, End)
                .clip()
                .opacity(shows)
                .translate(moved.0, moved.1)
                .scale(shrink)
                .child(Stack::new(vec![receding])),
        ));
    }
    if zooming {
        // a page is one glass sheet that grows from its tile's glass to the
        // card's, the frost with it at full strength: it takes over from
        // the tile's own, so the card never shows through half-frosted
        // where motion is reduced it fades in at its size instead, its frost
        // with it, rather than animating out of the tile's (Apple: no
        // animating into and out of blurs)
        let reduced = crate::theme::reduced_motion();
        let from = if reduced { full } else { origin };
        let sheet = Sheet::blend(from, full, slide);
        let frost = sheet.frost();
        shapes.push(if reduced { frost.fade(slide) } else { frost });
        layers.push(boxed(sheet_layer(detail, sheet, inner, slide)));
    }

    let content = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .clip()
        .child(Stack::new(layers).width(Parent).height(Parent));

    // The card swells in from the bar's corner, rising out of the bar:
    // scaled about its centre, then moved so its bottom-right corner stays
    // put and lowered by what's left of the rise, the frost swelling and
    // fading with it. A compositor that can't fade frost only shows or
    // hides it, so there the card arrives whole (frost and modules in the
    // same frame) rather than fading in over frost already at full
    // strength; going, the frost goes at once and the modules fade out over
    // the sharp desktop.
    let fades = amane::blur_fades();
    let unfolded_shown = unfolded.clamp(0.0, 1.0);
    let scale = f32::blend(motion::scale(quick::open::scale()), 1.0, unfolded);
    // under a bar along the top it drops out of the bar instead, its
    // top-right corner staying put
    let down = if status_bar::edge() == Vertical::Top {
        -1.0
    } else {
        1.0
    };
    let lift = down * (1.0 - unfolded) * motion::travel(OPEN_LIFT);
    let opacity = if open && !fades { 1.0 } else { unfolded_shown };
    let card = content.opacity(opacity).scale(scale).translate(
        (1.0 - scale) * width / 2.0,
        down * (1.0 - scale) * outer / 2.0 + lift,
    );
    let pinned = if down > 0.0 { outer } else { 0.0 };
    let mut frost = if open || fades {
        glass_region::faded(
            glass_region::transformed(shapes, scale, (width, pinned), (0.0, lift)),
            opacity,
        )
    } else {
        Vec::new()
    };

    // Where the card is on the surface. In card mode the surface is the
    // card, so this is (0, 0); while editing the surface covers the output
    // and the card keeps its place at the bar's right-hand corner, the same
    // place its margins give it. Worked out from the surface's size rather than the
    // mode, so the one frame amane draws at the old size as the mode
    // changes (the view runs before the new size is sent) draws the card
    // where it is either way.
    let (screen_w, screen_h) = window_size();
    let margin = popover_margin();
    let card_at = (
        (screen_w - margin.right as f32 - width).max(0.0),
        if status_bar::edge() == Vertical::Top {
            margin.top as f32
        } else {
            (screen_h - margin.bottom as f32 - outer).max(0.0)
        },
    );
    let canvas_at = (card_at.0 + inset, card_at.1 + inset + grid_top + shift);
    customize::set_canvas_origin(canvas_at.0, canvas_at.1);

    let full = edit::editing() || customize::visible();
    if !full {
        return window(width, outer.round())
            .visible(true)
            .blur_shapes(frost)
            .child(card);
    }

    // editing: the surface covers the output; the card is drawn where it
    // always is, the Customize window beside it, the control in hand over
    // both, and the one still layer every drag is measured on under all of
    // it (first, so nothing that changes under a drag comes before it)
    frost = glass_region::transformed(frost, 1.0, (0.0, 0.0), card_at);
    let card = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(outer))
        .fill(Color::TRANSPARENT)
        .translate(card_at.0, card_at.1)
        .child(card);
    let card_area = customize::Area {
        x: card_at.0,
        y: card_at.1,
        width,
        height: outer,
    };
    let mut root: Vec<Box<dyn Widget>> = vec![customize::drag_layer(), boxed(card)];
    let mut input = vec![input_area(card_area)];
    if let Some(built) = customize::view((screen_w, screen_h), card_area) {
        root.push(built.widget);
        frost.extend(built.frost);
        input.push(input_area(built.area));
    }
    if let Some((ghost, shape)) = customize::ghost(canvas_at) {
        root.push(ghost);
        frost.push(shape);
    }
    let surface = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .child(Stack::new(root).width(Parent).height(Parent));
    full_window()
        .visible(true)
        .input_region(input)
        .blur_shapes(frost)
        .child(track_cursor(surface))
}

/// `area` as a part of the surface that takes the pointer, rounded
/// outwards so no rim of it is missed.
fn input_area(area: customize::Area) -> InputArea {
    let x = area.x.floor();
    let y = area.y.floor();
    InputArea {
        x: x as i32,
        y: y as i32,
        width: ((area.x + area.width).ceil() - x) as i32,
        height: ((area.y + area.height).ceil() - y) as i32,
    }
}

// TODO(token): `quick.open.lift`, `quick.page.tile-fade` and
// `quick.page.grid-fade` once Penpot has them (manual §1.1).
/// How far below its place the card starts as it rises out of the bar.
const OPEN_LIFT: f32 = 12.0;
/// How much of the zoom it takes the tile to fade into its page, and the
/// rest of the grid to fade under it.
const TILE_FADE: f32 = 0.25;
const GRID_FADE: f32 = 0.6;

/// 0 before `from`, 1 after `to`, easing in and out between: one part of a
/// motion that runs across a larger one.
pub(super) fn ramp(value: f32, from: f32, to: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where a page's glass sheet is drawn, in the surface's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sheet {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    /// Thin glass as its tile, thickening as it grows into the page.
    tint: Color,
}

impl Sheet {
    fn centre(self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Where the compositor frosts behind it.
    fn frost(self) -> BlurShape {
        Material::Popover.shape_tinted(
            self.x,
            self.y,
            self.width,
            self.height,
            self.radius,
            1.0,
            self.tint,
        )
    }
}

impl Blend for Sheet {
    fn blend(from: Self, to: Self, amount: f32) -> Self {
        Self {
            x: f32::blend(from.x, to.x, amount),
            y: f32::blend(from.y, to.y, amount),
            width: f32::blend(from.width, to.width, amount),
            height: f32::blend(from.height, to.height, amount),
            radius: f32::blend(from.radius, to.radius, amount),
            tint: Color::blend(from.tint, to.tint, amount),
        }
    }
}

/// `page` on its glass sheet at `sheet`, laid out for the card it fills,
/// `height` tall (and clipped to the sheet while it grows), `reveal` of the
/// way in.
fn sheet_layer(page: Page, sheet: Sheet, height: f32, reveal: f32) -> Rectangle {
    let pad_in = quick::tile::pad();
    let grid_w = grid::Metrics::tokens().extent(grid::Canvas::tokens().cols);
    let content = Rectangle::new()
        .width(Size::Fixed(grid_w))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(pad_in, pad_in))
        .align_child(Start, Start)
        .child(pages::page(page).build((height - pad_in * 2.0).max(1.0), reveal));
    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .align_child(Start, Start)
        .child(
            tile::sheet(
                sheet.width,
                sheet.height,
                sheet.radius,
                Material::Popover,
                sheet.tint,
                content,
            )
            .translate(sheet.x, sheet.y),
        )
}

/// How wide the surface is at rest: the canvas and the margin either side.
fn surface_width() -> f32 {
    let px = grid::Metrics::tokens();
    px.extent(grid::Canvas::tokens().cols) + quick::grid::margin() * 2.0
}

/// The overlay surface in the bar's right-hand corner, `height` tall.
fn window(width: f32, height: f32) -> LayerWindow {
    let base = LayerWindow::new()
        .width(width)
        .height(height)
        .margin(popover_margin());
    overlay(keyed(base), QS_NS, status_bar::edge(), Horizontal::Right)
}

/// The surface over the whole output while editing (clear of what the bar
/// reserves, as the card is): the card and the Customize window on it.
fn full_window() -> LayerWindow {
    let base = LayerWindow::new()
        .width(WindowSize::Full)
        .height(WindowSize::Full);
    overlay(keyed(base), QS_NS, status_bar::edge(), Horizontal::Right)
}

/// `base` taking the keyboard: the card is modal while open, so Escape
/// works from the first moment, however it was opened; while editing, the
/// rest of the keys type into the Customize window's search.
fn keyed(base: LayerWindow) -> LayerWindow {
    base.keyboard(Keyboard::Exclusive).on_key(|key| {
        if matches!(key, Key::Escape) {
            escape();
        } else if edit::editing() {
            customize::key(key);
        }
    })
}
