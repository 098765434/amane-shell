//! Programs' own items, status items: put up over the bar's bus
//! service ([`status_apps`]) and drawn here, the program's picture and
//! title. A click, a middle click, a menu entry and files or text let go on
//! one go back to the program; files held over an item that opens on drag
//! tell it at once, so it can open a window to drop them on. They're kept
//! in the bar's order, dragged and hidden like its own.

use amane_kit::shell_ipc::Listed;
use std::cell::RefCell;
use std::collections::HashSet;

use amane::{Center, Color, Parent, Rectangle, Row, Text, Vertical, Widget};
use amane_status::{BarEdge, ItemButton, ItemEvent, ItemSpec, MenuEntry};

use crate::services::status_apps;
use crate::tokens::{Styled, sys};
use amane_kit::buttons::DrawnButton;
use amane_kit::layout::{boxed, cursor, text_width};
use amane_kit::material::{self, Level};
use amane_kit::menu::layer as context_menu;
use amane_kit::menu::{ContextMenu, MenuItem};

use super::activities::picture;
use super::indicators::Item;
use super::{BAR_NS, drag, edge, item, menu_edge};

/// How wide an item's title may be.
const TITLE: f32 = 120.0;

const MENU_KEY: &str = "amane.bar.app";

thread_local! {
    // the items files are held over now, so a program hears once each way
    static HOVERED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

/// The program's item named `name` (`app:<id>`) on the bar on `output`;
/// nothing once its program has gone.
pub fn widgets(name: &'static str, output: &str) -> Vec<Box<dyn Widget>> {
    let Some(id) = name.strip_prefix("app:") else {
        return Vec::new();
    };
    match status_apps::items()
        .into_iter()
        .find(|item| item.spec.id == id)
    {
        Some(app) => vec![boxed(button(app.spec, name, output))],
        None => Vec::new(),
    }
}

fn button(spec: ItemSpec, name: &'static str, output: &str) -> Rectangle {
    let metrics = super::metrics();
    let style = metrics.label();
    let title = spec
        .title
        .as_deref()
        .filter(|title| !title.is_empty())
        .map(|title| super::activities::clip(title, style, TITLE));
    let badge = spec.badge.clone().filter(|badge| !badge.is_empty());
    let inset = sys::space::inset::s();
    let mut width = metrics.icon + inset * 2.0;
    if let Some(title) = &title {
        width += 6.0 + text_width(title, style);
    }
    if let Some(badge) = &badge {
        width += 4.0 + text_width(badge, style);
    }
    let width = width.max(metrics.item_width);

    let id = spec.id.clone();
    let icon = spec.icon.clone();
    let attention = spec.attention;
    let tooltip = if spec.tooltip.is_empty() {
        title.clone().unwrap_or_else(|| id.clone())
    } else {
        spec.tooltip.clone()
    };

    let draw = move |color: Color| {
        let tint = if attention {
            sys::color::accent::bg()
        } else {
            color
        };
        let mut parts = vec![picture(&icon, metrics.icon, tint, false)];
        if let Some(title) = title {
            parts.push(boxed(Text::new(title).style(style).color(color)));
        }
        if let Some(badge) = badge {
            parts.push(boxed(
                Text::new(badge)
                    .style(style)
                    .color(sys::color::accent::bg()),
            ));
        }
        Rectangle::new()
            .width(width)
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .child(Row::new(parts).gap(4.0).align(Center))
    };

    let (on, pressed, middled) = (output.to_string(), output.to_string(), output.to_string());
    let (left, right, middle) = (id.clone(), id.clone(), id.clone());
    let menu = spec.menu.clone();
    let built = item(DrawnButton::new(format!("amane.bar.app.{id}"), draw))
        .width(width)
        .color(material::label(Level::Secondary))
        .on_press(move || activated(&left, ItemButton::Left, name, &pressed))
        .on_secondary(move || {
            if menu.is_empty() {
                activated(&right, ItemButton::Right, name, &on);
            } else {
                show_menu(&right, &menu);
            }
        })
        .on_middle(move || activated(&middle, ItemButton::Middle, name, &middled))
        .tooltip(BAR_NS, tooltip)
        .build();

    if !(spec.accepts.files || spec.accepts.text) {
        return built;
    }
    let accepts = spec.accepts;
    let opens = spec.opens_on_drag;
    let (over_id, over_on) = (id.clone(), output.to_string());
    let (files_id, text_id) = (id.clone(), id);
    let mut built = built.on_files_over(move |at, _files| {
        hover(&over_id, at.is_some() && opens, name, &over_on);
        accepts.files
    });
    if accepts.files {
        built = built.on_drop(move |files| {
            status_apps::send(&files_id, ItemEvent::DroppedFiles(files));
        });
    }
    if accepts.text {
        built = built.on_text_drop(move |text| {
            status_apps::send(&text_id, ItemEvent::DroppedText(text));
        });
    }
    built
}

/// Where the item's middle is on its monitor, and which edge the bar is on.
fn place(name: &'static str, output: &str) -> (f32, BarEdge) {
    let x = drag::middle_of(output, Item::App(name)).unwrap_or_else(|| cursor().0 as f32);
    let edge = match edge() {
        Vertical::Top => BarEdge::Top,
        _ => BarEdge::Bottom,
    };
    (x, edge)
}

fn activated(id: &str, button: ItemButton, name: &'static str, output: &str) {
    let (x, edge) = place(name, output);
    status_apps::send(
        id,
        ItemEvent::Activated {
            button,
            x,
            output: output.to_string(),
            edge,
        },
    );
}

/// Files came over the item or left it: its program hears once each way.
fn hover(id: &str, over: bool, name: &'static str, output: &str) {
    let changed = HOVERED.with_borrow_mut(|hovered| {
        if over {
            hovered.insert(id.to_string())
        } else {
            hovered.remove(id)
        }
    });
    if !changed {
        return;
    }
    let (x, edge) = place(name, output);
    status_apps::send(
        id,
        ItemEvent::DragHover {
            over,
            x,
            output: output.to_string(),
            edge,
        },
    );
}

/// The program's menu, as the shell's own; a pick goes back to it.
fn show_menu(id: &str, entries: &[MenuEntry]) {
    let menu = entries
        .iter()
        .fold(ContextMenu::new(), |menu, entry| add(menu, id, entry));
    context_menu::show_under(MENU_KEY, menu, BAR_NS, (cursor().0, menu_edge()));
}

fn add(menu: ContextMenu, id: &str, entry: &MenuEntry) -> ContextMenu {
    if entry.separator {
        return menu.separator();
    }
    menu.item(menu_item(id, entry))
}

fn menu_item(id: &str, entry: &MenuEntry) -> MenuItem {
    let mut row = MenuItem::label(entry.label.clone());
    if let Some(on) = entry.checked {
        row = row.checked(on);
    }
    if !entry.enabled {
        row = row.disabled();
    }
    if entry.destructive {
        row = row.destructive();
    }
    if !entry.children.is_empty() {
        let submenu = entry
            .children
            .iter()
            .fold(ContextMenu::new(), |menu, child| add(menu, id, child));
        return row.submenu(submenu);
    }
    let (item_id, entry_id) = (id.to_string(), entry.id.clone());
    row.on_select(move || status_apps::send(&item_id, ItemEvent::MenuSelected(entry_id.clone())))
}

/// `status-bar apps list | activate <id>`.
pub fn run(command: Listed) -> String {
    match command {
        Listed::List => status_apps::items()
            .iter()
            .map(|app| {
                let shown = if amane_kit::settings::status_bar().shows(&app.name()) {
                    "shown"
                } else {
                    "hidden"
                };
                format!(
                    "{}\t{}\t{}\t{shown}",
                    app.spec.id,
                    app.owner,
                    app.spec.title.clone().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Listed::Activate(id) => match status_apps::item(&id) {
            Some(_) => {
                let item = Item::named(&format!("app:{id}"));
                let output = amane::pointer_monitor()
                    .or_else(|| item.and_then(drag::output_with))
                    .unwrap_or_default();
                activated(&id, ItemButton::Left, item.map_or("", Item::name), &output);
                String::from("ok")
            }
            None => format!("no item {id}"),
        },
        Listed::Menu(id) => format!("{id}: a program's item has no menu to open from here"),
    }
}
