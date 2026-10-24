//! Quick Settings — GNOME's quick settings menu in the macOS Control Center
//! look: one frosted card above the bar with the session's actions, the
//! volume and brightness sliders and a grid of tiles, each drilling into a
//! page of its own.
//!
//! The grid follows GNOME's order and rules: the system row (battery,
//! screenshot, lock, power), what's playing, the output, microphone and
//! brightness sliders, the toggles (wired, Wi-Fi, VPN, Bluetooth, power
//! mode, night light, dark style, Do Not Disturb, keyboard, airplane mode),
//! then background apps; each shows only when the machine has it. A tile's
//! page slides in over the grid, as on macOS, and the card springs to its
//! height. Escape goes back to the grid, then closes.
//!
//! It shares the bar panels' one-open-at-a-time state
//! ([`Panel::QuickSettings`]), opened from the bar's Control Center button,
//! by right-clicking a status icon (straight to its page), or over IPC:
//! `quick-settings [toggle | open <page> | close]`. See
//! `docs/quick-settings.md`.

mod detail;
mod grid;
mod pages;
mod system_row;
mod tile;
mod tiles;

use std::cell::{Cell, RefCell};

use amane::{
    Animation, Color, Column, End, Horizontal, Key, Keyboard, LayerWindow, Parent, Rectangle, Size,
    Stack, Start, Vertical, Widget, mark_all,
};

use crate::components::layout::{blank, boxed};
use crate::components::motion;
use crate::components::taskbar::{
    self, Panel, bluetooth, close, is_open, overlay, popover_margin, wifi,
};
use crate::tokens::{quick, sys};

pub use pages::Page;

/// Layer-surface namespace; the niri `layer-rule` matches it for blur, and
/// the Bluetooth device menu hangs under it.
pub const QS_NS: &str = "amane.quick-settings";

thread_local! {
    // where it's going: the grid or a page
    static PAGE: Cell<Page> = const { Cell::new(Page::Grid) };

    // the page drawn over the grid, which stays while sliding back out
    static DETAIL: Cell<Page> = const { Cell::new(Page::Grid) };

    // 0 closed … 1 open: the card unfolds from the bar and fades in
    static OPEN: RefCell<Animation> = RefCell::new(motion::sheet(0.0));

    // 0 the grid … 1 a page
    static SLIDE: RefCell<Animation> = RefCell::new(motion::sheet(0.0));

    // the card's height; none until the first frame knows the grid's
    static HEIGHT: RefCell<Option<Animation>> = const { RefCell::new(None) };

    // the surface's height last frame, kept while it closes
    static LAST_HEIGHT: Cell<f32> = const { Cell::new(1.0) };
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

/// Goes to `page`: slides it in over the grid, or back out.
fn go(page: Page) {
    let previous = PAGE.replace(page);
    if previous == page {
        return;
    }
    leave(previous);
    if page != Page::Grid {
        DETAIL.set(page);
    }

    let target = if page == Page::Grid { 0.0 } else { 1.0 };
    // opening straight onto a page shows it, without a slide
    let hidden = OPEN.with_borrow(|open| open.value() < 0.001);
    SLIDE.with_borrow_mut(|slide| {
        if hidden {
            *slide = motion::sheet(target);
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

/// Escape: back to the grid from a page, else close.
fn escape() {
    if PAGE.get() == Page::Grid {
        close();
    } else {
        back();
    }
}

pub(crate) fn opened() {}

/// The panel closed: its page's searches stop now; the page itself stays
/// drawn while the card folds away.
pub(crate) fn closed() {
    leave(PAGE.get());
}

/// Gone from the screen: the next opening starts on the grid.
fn hidden() {
    if PAGE.replace(Page::Grid) != Page::Grid {
        // what the page drew while folding away may have started a search again
        leave(DETAIL.get());
    }
    DETAIL.set(Page::Grid);
    SLIDE.with_borrow_mut(|slide| *slide = motion::sheet(0.0));
    HEIGHT.with_borrow_mut(|height| *height = None);
}

/// `amane-shell` IPC: `quick-settings [toggle | open <page> | close]`.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["toggle"] => taskbar::toggle(Panel::QuickSettings),
        ["open"] => open(Page::Grid),
        ["open", name] => match Page::named(name) {
            Some(page) => open(page),
            None => return format!("unknown page {name}; pages: {}", Page::NAMES.join(", ")),
        },
        ["close"] => {
            if is_open(Panel::QuickSettings) {
                close();
            }
        }
        _ => return String::from("usage: quick-settings [toggle | open <page> | close]"),
    }
    mark_all();
    String::from("ok")
}

// ───── view ──────────────────────────────────────────────────────────────────

/// The surface: hidden while closed, else the card, as tall as its page.
pub fn surface() -> LayerWindow {
    let open = is_open(Panel::QuickSettings);
    let unfolded = OPEN.with_borrow_mut(|anim| {
        anim.to(if open { 1.0 } else { 0.0 });
        anim.value()
    });
    let shown = open || unfolded > 0.001;

    if !shown {
        hidden();
        return window(1.0).visible(false).child(blank());
    }

    let page = PAGE.get();
    let slide = SLIDE.with_borrow_mut(|slide| slide.value()).clamp(0.0, 1.0);
    let inset = sys::space::inset::l();

    // only what's on screen is built, so a page hidden behind the other
    // takes no clicks and starts no searches
    let mut layers: Vec<(Column, bool)> = Vec::new();
    let mut target = 0.0;
    if page == Page::Grid || slide < 0.999 {
        let (grid, height) = tiles::grid();
        if page == Page::Grid {
            target = height;
        }
        layers.push((grid, false));
    }
    let detail = if page == Page::Grid {
        DETAIL.get()
    } else {
        page
    };
    if detail != Page::Grid && (page != Page::Grid || slide > 0.001) {
        let view = pages::page(detail);
        if page != Page::Grid {
            target = view.height();
        }
        layers.push((view.build(), true));
    }

    let height = HEIGHT.with_borrow_mut(|anim| {
        let anim = anim.get_or_insert_with(|| motion::sheet(target));
        anim.to(target);
        anim.value()
    });
    // Opening, the card unfolds upwards from the bar. Closing, it only
    // fades at the size it had: a surface resized right before it hides
    // still has the compositor's answers to the resize on their way, and
    // amane draws on those after the hide, which niri rejects.
    let outer = if open {
        ((height.max(0.0) + inset * 2.0) * unfolded.clamp(0.0, 1.05)).max(1.0)
    } else {
        LAST_HEIGHT.get()
    };
    LAST_HEIGHT.set(outer);
    let inner = (outer - inset * 2.0).max(0.0);

    let width = sys::size::popover::width();
    let recede = quick::page::recede();
    let layers: Vec<Box<dyn Widget>> = layers
        .into_iter()
        .map(|(content, is_page)| {
            // the grid recedes left as a page slides in from the right
            let (opacity, x, scale) = if is_page {
                (slide, (1.0 - slide) * width, 1.0)
            } else {
                (
                    1.0 - slide,
                    -slide * width * 0.25,
                    1.0 - (1.0 - recede) * slide,
                )
            };
            // pinned to the bottom, so the card unfolds upwards from the bar
            boxed(
                Rectangle::new()
                    .width(Parent)
                    .height(Size::Fixed(inner))
                    .fill(Color::TRANSPARENT)
                    .align_child(Start, End)
                    .clip()
                    .opacity(opacity.clamp(0.0, 1.0))
                    .translate(x, 0.0)
                    .scale(scale)
                    .child(content),
            )
        })
        .collect();

    let content = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .opacity(unfolded.clamp(0.0, 1.0))
        .clip()
        .child(Stack::new(layers).width(Parent).height(Parent));

    let card = taskbar::panel_card(vec![boxed(content)], 0.0);
    let card = if open {
        card
    } else {
        card.opacity(unfolded.clamp(0.0, 1.0))
    };
    window(outer.round()).visible(true).child(card)
}

/// The bottom-right overlay surface, `height` tall.
fn window(height: f32) -> LayerWindow {
    let base = LayerWindow::new()
        .width(sys::size::popover::width())
        .height(height)
        .margin(popover_margin())
        .keyboard(Keyboard::OnDemand)
        .on_key(|key| {
            if matches!(key, Key::Escape) {
                escape();
            }
        });
    overlay(base, QS_NS, Vertical::Bottom, Horizontal::Right)
}
