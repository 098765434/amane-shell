//! Apps' own items in the bar: the system tray's icons
//! (amane's [`Tray`], StatusNotifierItem) for Steam, Discord, nm-applet
//! and the like. Left click does the app's action, or shows its menu when
//! it only has a menu; right click shows its menu; middle click is its
//! second action and scrolling goes to it. An app can be kept out of the
//! bar ("Allow in the Status Bar", `StatusBar::tray_denied`), and items
//! that don't fit go behind a » button.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use amane::{
    Center, Color, Image, Radius, Rectangle, Scroll, Service, Tray, TrayMenuItem, TrayStatus,
    TrayToggle, Widget,
};
use emaki_kit::settings::{remember_tray, status_bar};

use crate::tokens::sys;
use emaki_kit::buttons::DrawnButton;
use emaki_kit::layout::{boxed, cursor};
use emaki_kit::material::{self, Level};
use emaki_kit::menu::{Check, ContextMenu, Icon, MenuItem};

use super::{BAR_NS, bar_icon, item, menu_edge, menu_point, menu_showing, show_menu};
use emaki_kit::icons::ICON_MORE;
use emaki_kit::shell_ipc::Listed;

const MENU_KEY: &str = "emaki.bar.tray";
const MORE_KEY: &str = "emaki.bar.tray.more";

/// One app's item as the bar shows it.
#[derive(Clone)]
pub struct Shown {
    key: String,
    title: String,
    icon: Option<PathBuf>,
    attention: bool,
    menu_only: bool,
    has_menu: bool,
}

thread_local! {
    // the item whose menu was asked for, and where along the bar, until it comes
    static ASKED: RefCell<Option<(String, i32)>> = const { RefCell::new(None) };

    // how many of the tray's items the bar has seen, so new ones are remembered once
    static SEEN: Cell<usize> = const { Cell::new(0) };
}

/// The apps allowed in the bar with something to show, in the order they
/// came.
pub fn shown() -> Vec<Shown> {
    open_asked_menu();

    let tray = Tray::read();
    let items = tray.items();

    // apps seen for the first time are remembered for Emaki Settings' list
    if SEEN.replace(items.len()) != items.len() {
        for item in items {
            remember_tray(item.id(), item.title());
        }
    }

    let settings = status_bar();
    items
        .iter()
        .filter(|item| item.status() != TrayStatus::Passive && settings.allows(item.id()))
        .map(|item| Shown {
            key: item.key(),
            title: String::from(item.tooltip()),
            icon: item.icon().map(PathBuf::from),
            attention: item.status() == TrayStatus::NeedsAttention,
            menu_only: item.item_is_menu(),
            has_menu: item.has_menu(),
        })
        .collect()
}

/// The icons of `shown`, the last `hidden` of them behind a » button.
pub fn icons(shown: &[Shown], hidden: usize) -> Vec<Box<dyn Widget>> {
    let kept = shown.len() - hidden.min(shown.len());
    let mut widgets: Vec<Box<dyn Widget>> = shown[..kept]
        .iter()
        .map(|shown| boxed(icon(shown)))
        .collect();
    if kept < shown.len() {
        widgets.push(boxed(more(shown[kept..].to_vec())));
    }
    widgets
}

/// How wide `count` icons are, gaps after each included.
pub fn width_of(count: usize) -> f32 {
    let metrics = super::metrics();
    count as f32 * (metrics.item_width + metrics.gap)
}

fn icon(shown: &Shown) -> Rectangle {
    let picture = shown.icon.clone();
    let attention = shown.attention;
    let title = shown.title.clone();
    let (pressed, secondary, middle, scrolled) = (
        shown.clone(),
        shown.key.clone(),
        shown.key.clone(),
        shown.key.clone(),
    );

    let id = button_id(&shown.key);
    item(DrawnButton::new(id.clone(), move |color| {
        picture_of(picture, attention, color)
    }))
    .width(super::metrics().item_width)
    .active(menu_showing(&id))
    .color(material::label(Level::Secondary))
    .on_press(move || press(&pressed))
    .on_secondary(move || ask_menu(&secondary))
    .on_middle(move || {
        let (x, y) = menu_point();
        Tray::secondary_activate(&middle, x, y);
    })
    .on_scroll(move |scroll: Scroll| {
        let (delta, vertical) = if scroll.y != 0.0 {
            (scroll.y, true)
        } else {
            (scroll.x, false)
        };
        Tray::scroll(&scrolled, delta.round() as i32, vertical);
    })
    .tooltip(BAR_NS, title)
    .build()
}

/// The app's picture, or a dot in `color` for one that sent none; with an
/// accent dot at its corner while it wants attention, inside the glyph's
/// square so it fits the Compact bar.
fn picture_of(picture: Option<PathBuf>, attention: bool, color: Color) -> Rectangle {
    let size = super::metrics().icon;
    let inner = match picture {
        Some(path) => Rectangle::new()
            .width(size)
            .height(size)
            .fill(Image::contain(path)),
        None => Rectangle::new()
            .width(size / 2.0)
            .height(size / 2.0)
            .radius(Radius::Full)
            .fill(color),
    };
    let frame = Rectangle::new()
        .width(size)
        .height(size)
        .align_child(Center, Center);
    if !attention {
        return frame.child(inner);
    }
    let badge = (size * 0.4).round();
    frame.child(amane::Stack::new(vec![
        boxed(
            Rectangle::new()
                .width(size)
                .height(size)
                .align_child(Center, Center)
                .child(inner),
        ),
        boxed(
            Rectangle::new()
                .width(size)
                .height(size)
                .align_child(amane::End, amane::End)
                .child(
                    Rectangle::new()
                        .width(badge)
                        .height(badge)
                        .radius(Radius::Full)
                        .fill(sys::color::accent::bg()),
                ),
        ),
    ]))
}

fn press(shown: &Shown) {
    if shown.menu_only && shown.has_menu {
        ask_menu(&shown.key);
    } else {
        let (x, y) = menu_point();
        Tray::activate(&shown.key, x, y);
    }
}

/// The bar button of the tray item `key`.
fn button_id(key: &str) -> String {
    format!("emaki.bar.tray.{key}")
}

/// Asks for the item's menu; the bar shows it once it's read. An item
/// without one shows its own.
fn ask_menu(key: &str) {
    let has_menu = Tray::read()
        .items()
        .iter()
        .any(|item| item.key() == key && item.has_menu());
    if !has_menu {
        let (x, y) = menu_point();
        Tray::context_menu(key, x, y);
        return;
    }
    ASKED.set(Some((key.to_string(), cursor().0)));
    Tray::open_menu(key);
}

/// The menu asked for, once the tray has read it.
fn open_asked_menu() {
    let Some((key, x)) = ASKED.with_borrow(Clone::clone) else {
        return;
    };
    let Some(items) = Tray::take_menu(&key) else {
        return;
    };
    ASKED.set(None);
    if items.is_empty() {
        return;
    }
    let menu = menu_of(&key, &items);
    show_menu(&button_id(&key), MENU_KEY, menu, (x, menu_edge()));
}

/// The app's menu as the shell's own.
fn menu_of(key: &str, items: &[TrayMenuItem]) -> ContextMenu {
    let mut menu = ContextMenu::new();
    let mut last_separator = true;
    for entry in items {
        if entry.separator {
            if !last_separator {
                menu = menu.separator();
                last_separator = true;
            }
            continue;
        }
        last_separator = false;
        let mut row = MenuItem::label(entry.label.clone());
        row = match entry.toggle {
            TrayToggle::Check(on) => row.checked(on),
            TrayToggle::Radio(on) => row.check(Check::Radio(on)),
            TrayToggle::None => row,
        };
        if let Some(path) = &entry.icon {
            row = row.icon(Icon::image(path.clone()));
        }
        if !entry.enabled {
            row = row.disabled();
        }
        if entry.children.is_empty() {
            let (key, id) = (key.to_string(), entry.id);
            row = row.on_select(move || Tray::select(&key, id));
        } else {
            row = row.submenu(menu_of(key, &entry.children));
        }
        menu = menu.item(row);
    }
    menu
}

/// The » button: the items that didn't fit, each opening as it would in
/// the bar.
fn more(hidden: Vec<Shown>) -> Rectangle {
    let count = hidden.len();
    bar_icon(
        "emaki.bar.tray.more",
        ICON_MORE,
        material::label(Level::Secondary),
    )
    .active(menu_showing(MORE_KEY))
    .on_press(move || {
        let mut menu = ContextMenu::new();
        for shown in &hidden {
            let mut row = MenuItem::label(if shown.title.is_empty() {
                String::from("App")
            } else {
                shown.title.clone()
            });
            if let Some(path) = &shown.icon {
                row = row.icon(Icon::image(path.clone()));
            }
            let pressed = shown.clone();
            row = row.on_select(move || press(&pressed));
            menu = menu.item(row);
        }
        show_menu(MORE_KEY, MORE_KEY, menu, menu_point());
    })
    .tooltip(BAR_NS, format!("{count} more"))
    .build()
}

/// `status-bar tray list | activate <id> | menu <id>`: the apps in the
/// tray, or one app's action or menu, as from a keybind.
pub fn run(command: Listed) -> String {
    let find = |id: &str| {
        Tray::read()
            .items()
            .iter()
            .find(|item| item.id() == id)
            .map(|item| (item.key(), item.has_menu()))
    };
    match command {
        Listed::List => Tray::read()
            .items()
            .iter()
            .map(|item| format!("{}\t{:?}\t{}", item.id(), item.status(), item.title()))
            .collect::<Vec<_>>()
            .join("\n"),
        Listed::Activate(id) => match find(&id) {
            Some((key, _)) => {
                let (x, y) = menu_point();
                Tray::activate(&key, x, y);
                String::from("ok")
            }
            None => format!("no tray app {id}"),
        },
        Listed::Menu(id) => match find(&id) {
            Some((key, true)) => {
                ask_menu(&key);
                amane::mark_all();
                String::from("ok")
            }
            Some((_, false)) => format!("{id} has no menu"),
            None => format!("no tray app {id}"),
        },
    }
}
