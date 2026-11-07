//! Quick Settings — GNOME's quick settings menu as a grid of glass
//! modules: one frosted card above the bar holding the modules, each
//! drilling into a page of its own.
//!
//! The card is a canvas four cells across and eight down (`grid`), laid
//! out by default as Wi‑Fi and Bluetooth tiles beside Now Playing, the
//! Focus tile and round buttons, a
//! Display card and a Sound card with their sliders, then the battery and
//! rows of round buttons; Edit Controls moves and resizes
//! them (`edit`, `layout`). It keeps GNOME's rules: a control shows only
//! when the machine has what it drives. A tile's page grows out of the tile
//! to fill the card. Escape goes back to the grid, then closes.
//!
//! The card itself has no background: each module is a
//! frosted glass piece of its own, and the surface tells the compositor
//! where to frost (`glass_region`, over ext-background-effect) — the grid's
//! modules, or the whole card while a page is on it.
//!
//! The card has one height, the grid's (the rows in use, or all of them
//! in edit mode), for the grid and every page, and changes only at rest,
//! when the grid gains or loses a row or edit mode changes it: the grid
//! slides to its new place rather than jumping. The surface itself keeps
//! the output's size the whole time the card shows.
//!
//! It's the one place for sound, Wi‑Fi, Bluetooth and power: the bar's
//! status icons (each optional, see the pages' "Show in Status Bar") open
//! straight to their page. It shares the notification center's
//! one-open-at-a-time state ([`Panel::QuickSettings`]), opened from the
//! bar's Quick Settings button, a status icon, or over IPC:
//! `quick-settings [toggle [<page>] | open [<page>] | close]`. See
//! `docs/shell/quick-settings.md`.
//!
//! A status icon opens its page on its own (`open_alone`): no grid behind
//! it and no back chevron, Escape closes it, and the icon again closes it.
//!
//! While the card shows, its surface covers the output (clear of the bar)
//! with the card at the bar's corner, so a click anywhere outside the card
//! closes it, as the launcher does.
//!
//! Edit mode opens the Customize window (`customize`) beside the card.
//! amane can't carry a drag from one
//! surface to another, so the one surface holds both: the card where it
//! always is, at the bottom-right, and the window in the middle, with one
//! still layer under both that every drag is measured on. While editing,
//! clicks outside the two go through to what's under them
//! (`input_region`).

mod controls;
mod customize;
mod detail;
mod edit;
mod grid;
mod ipc;
mod layout;
mod pages;
mod slider_card;
mod system_row;
mod tile;
pub(crate) mod tiles;
mod view;

pub use ipc::run;
pub(super) use view::ramp;
pub use view::surface;

use std::cell::{Cell, RefCell};
use std::time::Instant;

use amane::{
    Animation, Blend, BlurShape, Color, End, Horizontal, InputArea, Key, Keyboard, LayerWindow,
    Parent, Rectangle, Stack, Start, Vertical, Widget, WindowSize, window_size,
};

use crate::chrome::bar;
use crate::chrome::panel::{self, Panel, close, is_open, overlay, popover_margin};
use crate::surfaces::quick_settings::grid::FromTokens as _;
use crate::surfaces::quick_settings::layout::Stored as _;
use crate::tokens::quick;
use amane_kit::glass_region;
use amane_kit::layout::{boxed, pad, track_cursor};
use amane_kit::material::Material;
use amane_kit::motion;
use amane_kit::reveal::Reveal;
use amane_kit::shell_ipc::QuickSettings as Command;

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

    // whether the page was opened on its own, from outside the card (a
    // status icon): no way back to the grid, and Escape closes
    static ALONE: Cell<bool> = const { Cell::new(false) };

    // 0 closed … 1 open: the card fades, swells and rises in over the bar
    static OPEN: RefCell<Reveal> = RefCell::new(Reveal::new());

    // whether OPEN is heading for 1

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

/// Opens on `page`, or goes there when open already; a page has its way
/// back to the grid.
pub fn open(page: Page) {
    ALONE.set(false);
    show(page);
}

/// Opens `page` on its own, as from the bar's status icons: no grid behind
/// it and no way back to it, and Escape closes it. The grid opens as
/// [`open`] does.
pub fn open_alone(page: Page) {
    ALONE.set(page != Page::Grid);
    show(page);
}

/// [`open_alone`], or closes it when it's showing already.
pub fn toggle_alone(page: Page) {
    if alone() && PAGE.get() == page {
        close();
    } else {
        open_alone(page);
    }
}

fn show(page: Page) {
    if is_open(Panel::QuickSettings) {
        panel::follow_pointer();
    } else {
        panel::toggle(Panel::QuickSettings);
    }
    go(page);
}

/// Whether Quick Settings is open on `page`.
pub fn showing(page: Page) -> bool {
    is_open(Panel::QuickSettings) && PAGE.get() == page
}

/// Whether Quick Settings is open on a page on its own (see
/// [`open_alone`]).
pub fn alone() -> bool {
    is_open(Panel::QuickSettings) && ALONE.get()
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
    } else if PAGE.get() == Page::Grid || ALONE.get() {
        close();
    } else {
        back();
    }
}

/// Tells the panels what Quick Settings does as it closes. Once, at startup.
pub fn register_panel() {
    panel::register(
        Panel::QuickSettings,
        panel::Hooks {
            opened: || {},
            closed,
        },
    );
}

/// The panel closed: its page's searches stop now; the page itself, and
/// edit mode's canvas, stay drawn as they were while the card fades away.
fn closed() {
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
    ALONE.set(false);
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

/// The layout from its file, or packed with what this machine has, for an
/// IPC call that comes before the card has ever shown.
fn load_layout() {
    let ctx = controls::Ctx::new(false);
    layout::load_once(&|id| controls::present(id, &ctx));
}

// ───── view ──────────────────────────────────────────────────────────────────

// TODO(token): `quick.open.lift`, `quick.page.tile-fade` and
// `quick.page.grid-fade` once Penpot has them (manual §1.1).
/// How far below its place the card starts as it rises out of the bar.
const OPEN_LIFT: f32 = 12.0;
/// How much of the zoom it takes the tile to fade into its page, and the
/// rest of the grid to fade under it.
const TILE_FADE: f32 = 0.25;
const GRID_FADE: f32 = 0.6;

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
