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
//! when the machine has what it drives. A tile's page slides in over the
//! grid, as on macOS. Escape goes back to the grid, then closes.
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
//! It shares the bar panels' one-open-at-a-time state
//! ([`Panel::QuickSettings`]), opened from the bar's Control Center button,
//! by right-clicking a status icon (straight to its page), or over IPC:
//! `quick-settings [toggle | open <page> | close]`. See
//! `docs/quick-settings.md`.

mod controls;
mod detail;
mod edit;
mod glass_region;
mod grid;
mod layout;
mod pages;
mod slider_card;
mod system_row;
mod tile;
mod tiles;

use std::cell::{Cell, RefCell};

use amane::{
    Animation, Blend, Color, End, Horizontal, InputArea, Key, Keyboard, LayerWindow, Parent,
    Rectangle, Size, Stack, Start, Vertical, Widget, mark_all,
};

use crate::components::layout::{blank, boxed, pad};
use crate::components::motion;
use crate::components::taskbar::{
    self, Panel, bluetooth, close, is_open, overlay, popover_margin, wifi,
};
use crate::tokens::quick;

pub use pages::Page;

/// Layer-surface namespace; the niri `layer-rule` matches it for blur, and
/// the Bluetooth device menu hangs under it.
pub const QS_NS: &str = "amane.quick-settings";

thread_local! {
    // where it's going: the grid or a page
    static PAGE: Cell<Page> = const { Cell::new(Page::Grid) };

    // the page drawn over the grid, which stays while sliding back out
    static DETAIL: Cell<Page> = const { Cell::new(Page::Grid) };

    // 0 closed … 1 open: the card fades and swells in over the bar
    static OPEN: RefCell<Animation> = RefCell::new(motion::fade(0.0, motion::OPEN));

    // whether OPEN is heading for 1
    static OPENING: Cell<bool> = const { Cell::new(false) };

    // 0 the grid … 1 a page
    static SLIDE: RefCell<Animation> = RefCell::new(motion::sheet(0.0));

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
        taskbar::toggle(Panel::QuickSettings);
    }
    go(page);
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
    leave(previous);
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
            *slide = motion::sheet(target);
        } else if swapping {
            *slide = motion::sheet(0.0);
            slide.to(1.0);
        } else {
            slide.to(target);
        }
    });
    mark_all();
}

/// What a page started that shouldn't outlive it.
fn leave(page: Page) {
    match page {
        Page::Wifi => wifi::reset(),
        Page::Bluetooth => bluetooth::closed(),
        _ => {}
    }
    pages::left(page);
}

/// Escape: lets go of a drag, then out of edit mode, back to the grid
/// from a page, else close.
fn escape() {
    if edit::cancel() {
        return;
    }
    if edit::editing() {
        edit::leave();
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
    leave(PAGE.get());
}

/// Gone from the screen: the next opening starts on the grid, out of edit
/// mode.
fn hidden() {
    if PAGE.replace(Page::Grid) != Page::Grid {
        // what the page drew while fading away may have started a search again
        leave(DETAIL.get());
    }
    DETAIL.set(Page::Grid);
    SWAP.set(Page::Grid);
    SLIDE.with_borrow_mut(|slide| *slide = motion::sheet(0.0));
    SHIFT.with_borrow_mut(|shift| *shift = motion::spring(0.0));
    LAST_INNER.set(0.0);
    edit::leave();
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
        [] | ["toggle"] => taskbar::toggle(Panel::QuickSettings),
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
        ["edit"] => {
            open(Page::Grid);
            edit::enter();
        }
        ["reset-layout"] => {
            let ctx = controls::Ctx::new(false);
            layout::replace(layout::Layout::default_for(grid::Canvas::tokens(), &|id| {
                controls::present(id, &ctx)
            }));
        }
        _ => {
            return String::from(
                "usage: quick-settings [toggle | open <page> | close | edit | reset-layout]",
            );
        }
    }
    mark_all();
    String::from("ok")
}

// ───── view ──────────────────────────────────────────────────────────────────

/// Steps the card's opening towards `open` and gives how far it is. Each
/// direction has its own duration, so the fade starts over from where it
/// is when the direction changes.
fn unfold(open: bool) -> f32 {
    OPEN.with_borrow_mut(|anim| {
        if OPENING.replace(open) != open {
            let (from, duration) = (
                anim.value(),
                if open { motion::OPEN } else { motion::CLOSE },
            );
            *anim = motion::fade(from, duration);
            anim.to(if open { 1.0 } else { 0.0 });
        }
        anim.value().clamp(0.0, 1.0)
    })
}

/// The surface: hidden while closed, else the card, as tall as the grid.
pub fn surface() -> LayerWindow {
    let open = is_open(Panel::QuickSettings);
    let unfolded = unfold(open);
    let shown = open || unfolded > 0.001;

    if !shown {
        hidden();
        return window(surface_width(), 1.0)
            .visible(false)
            .blur_region(Vec::new())
            .child(blank());
    }

    // a page whose setting has gone gives way to the grid
    if PAGE.get() != Page::Grid && !pages::available(PAGE.get()) {
        back();
    }
    let page = PAGE.get();
    // the sheet's spring overshoots a little; only what it draws at is
    // clamped, not where
    let slide_raw = SLIDE.with_borrow_mut(|slide| slide.value());
    let slide = slide_raw.clamp(0.0, 1.0);
    if slide_raw >= 0.999 {
        SWAP.set(Page::Grid);
    }
    // the canvas sits a margin in from the surface's edge: room for the
    // modules' shadows
    let inset = quick::grid::margin();

    // the grid sets the card's size, so it's always built; only what's on
    // screen is drawn, so a page hidden behind the grid takes no clicks and
    // starts no searches
    let grid = tiles::grid();
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

    let mut regions: Vec<InputArea> = Vec::new();
    let mut layers: Vec<Box<dyn Widget>> = Vec::new();
    let recede = quick::page::recede();
    let swap = SWAP.get();

    if page == Page::Grid || slide < 0.999 {
        // the grid recedes left as a page slides in from the right; from
        // one page to another, the old page recedes in its place
        let receding: Box<dyn Widget> = if swap != Page::Grid {
            regions.extend(sheet_region(inset, inset, grid_w, inner));
            boxed(sheet_layer(swap, grid_w, inner))
        } else {
            regions.extend(grid.regions.iter().map(|area| InputArea {
                x: area.x + inset as i32,
                y: area.y + (inset + grid_top + shift).round() as i32,
                ..*area
            }));
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
        layers.push(boxed(
            Rectangle::new()
                .width(Parent)
                .height(Size::Fixed(outer))
                .fill(Color::TRANSPARENT)
                .padding(pad(inset, inset))
                .align_child(Start, End)
                .clip()
                .opacity(1.0 - slide)
                .translate(-slide_raw * width * GRID_RECEDE, 0.0)
                .scale(1.0 - (1.0 - recede) * slide_raw)
                .child(Stack::new(vec![receding])),
        ));
    }
    let detail = if page == Page::Grid {
        DETAIL.get()
    } else {
        page
    };
    if detail != Page::Grid && (page != Page::Grid || slide > 0.001) {
        // a page is one glass sheet the size of the grid, and the frost
        // comes in with it
        let offset = (1.0 - slide_raw) * width;
        regions.extend(sheet_region(inset + offset, inset, grid_w, inner));
        layers.push(boxed(
            sheet_layer(detail, grid_w, inner)
                .opacity(slide)
                .translate(offset, 0.0),
        ));
    }

    let content = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .clip()
        .child(Stack::new(layers).width(Parent).height(Parent));

    // the card fades and swells in from the bar's corner: scaled about its
    // centre, then moved so its bottom-right corner stays put; the frost
    // swells with it
    let scale = f32::blend(quick::open::scale(), 1.0, unfolded);
    let card = content
        .opacity(unfolded)
        .scale(scale)
        .translate((1.0 - scale) * width / 2.0, (1.0 - scale) * outer / 2.0);
    window(width, outer.round())
        .visible(true)
        .blur_region(glass_region::scaled(regions, scale, width, outer))
        .child(card)
}

// TODO(token): `quick.page.grid-recede` once Penpot has it (manual §1.1).
/// How far across the card the grid moves as a page slides in over it, as
/// a share of the card's width.
const GRID_RECEDE: f32 = 0.25;

/// Where the compositor frosts behind a page's sheet at (`x`, `y`).
fn sheet_region(x: f32, y: f32, width: f32, height: f32) -> Vec<InputArea> {
    glass_region::rounded(
        x,
        y,
        width,
        height,
        quick::card::radius(),
        glass_region::STEPS,
    )
}

/// `page` on its glass sheet, the grid's size, a margin in from the
/// surface's edge like the grid.
fn sheet_layer(page: Page, width: f32, height: f32) -> Rectangle {
    let inset = quick::grid::margin();
    let pad_in = quick::tile::pad();
    let content = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .padding(pad(pad_in, pad_in))
        .align_child(Start, Start)
        .child(pages::page(page).build((height - pad_in * 2.0).max(1.0)));
    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .padding(pad(inset, inset))
        .align_child(Start, Start)
        .child(tile::sheet(width, height, quick::card::radius(), content))
}

/// How wide the surface is at rest: the canvas and the margin either side.
fn surface_width() -> f32 {
    let px = grid::Metrics::tokens();
    px.extent(grid::Canvas::tokens().cols) + quick::grid::margin() * 2.0
}

/// The bottom-right overlay surface, `height` tall.
fn window(width: f32, height: f32) -> LayerWindow {
    let base = LayerWindow::new()
        .width(width)
        .height(height)
        .margin(popover_margin())
        // the card is modal while open, so Escape works from the first
        // moment, however it was opened
        .keyboard(Keyboard::Exclusive)
        .on_key(|key| {
            if matches!(key, Key::Escape) {
                escape();
            }
        });
    overlay(base, QS_NS, Vertical::Bottom, Horizontal::Right)
}
