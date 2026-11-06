//! amane-drop: a status bar app in the manner of Aptonic's Dropzone. Its
//! item in the bar (an amane status item) opens a grid of folders, apps and
//! actions to let files go on, and a Drop Bar to hold them; dragging files
//! over the item opens it at once, so the drag carries on into it. Work
//! shows on its tile and as a Live Activity in the bar.
//!
//! `amane-drop` runs it; launched again, `amane-drop toggle | open | close`
//! goes to the copy running (a keybind's way in), as does the same over its
//! IPC socket.

mod files;
mod grid;
mod share;
mod shared;
mod state;
mod tasks;

use std::os::unix::net::UnixStream;
use std::sync::OnceLock;

use amane::status_item::{Accepts, ItemButton, ItemEvent, ItemIcon, ItemSpec, MenuEntry};
use amane::{App, StatusItem};
use amane_kit::settings::drop_grids;

const APP_ID: &str = "amane-drop";
const ITEM_ID: &str = "org.amane.drop";

static ITEM: OnceLock<StatusItem> = OnceLock::new();

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // a copy already running takes this launch: `App::run` hands it over
    let running = UnixStream::connect(amane::ipc_socket_for(APP_ID)).is_ok();
    if !running {
        let item = StatusItem::register(spec());
        item.on_event(event);
        let _ = ITEM.set(item);
        command(&args);
    }

    App::new()
        .app_id(APP_ID)
        .window(grid::window)
        .ipc("toggle", |_| reply(&["toggle"]))
        .ipc("open", |_| reply(&["open"]))
        .ipc("close", |_| reply(&["close"]))
        .ipc("edit", |_| reply(&["edit"]))
        .ipc("run", |args| {
            let mut all = vec![String::from("run")];
            all.extend(args.iter().cloned());
            command(&all);
            String::from("ok")
        })
        .ipc("hold", |args| {
            let mut all = vec![String::from("hold")];
            all.extend(args.iter().cloned());
            command(&all);
            String::from("ok")
        })
        .on_activate(|args| {
            // the first is the launch's activation token
            command(args.get(1..).unwrap_or_default());
        })
        .run();
}

fn reply(args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    command(&args);
    String::from("ok")
}

/// `toggle`, `open [grid]`, `close`, `edit`, `run <tile> <file>…` (the tile's
/// action on the files, numbered from 1 in the grid showing, as Dropzone's
/// `dz run`) or `hold <file>…` (onto the Drop Bar), from a launch or IPC.
fn command(args: &[String]) {
    let output = amane::pointer_monitor();
    match args.first().map(String::as_str) {
        Some("toggle") => state::toggle(None, output, None),
        Some("open") => {
            if let Some(index) = args.get(1).and_then(|index| index.parse::<usize>().ok()) {
                grid::pick(index.saturating_sub(1));
            }
            state::open(None, output, None, false);
        }
        Some("close") => state::close(),
        Some("edit") => {
            state::open(None, output, None, false);
            state::change(|ui| ui.editing = true);
        }
        Some("run") => {
            let tile = args.get(1).and_then(|tile| tile.parse::<usize>().ok());
            let files = paths(args.get(2..).unwrap_or_default());
            if let Some(tile) = tile.filter(|tile| *tile > 0)
                && !files.is_empty()
            {
                grid::run_tile(tile - 1, files);
            }
        }
        Some("hold") => state::hold(&current_grid_name(), paths(&args[1..])),
        _ => {}
    }
}

/// The bar's item: the tray, its menu naming the grids.
fn spec() -> ItemSpec {
    let mut spec = ItemSpec::new(ITEM_ID, ItemIcon::Glyph(String::from("tray")));
    spec.tooltip = String::from("Drop");
    spec.accepts = Accepts {
        files: true,
        text: false,
    };
    spec.opens_on_drag = true;

    let current = state::ui().grid;
    let grids = drop_grids();
    let mut menu: Vec<MenuEntry> = grids
        .grids
        .iter()
        .enumerate()
        .map(|(index, grid)| MenuEntry {
            checked: Some(index == current),
            ..MenuEntry::new(&format!("grid:{index}"), &grid.name)
        })
        .collect();
    drop(grids);
    let recent = shared::list();
    if !recent.is_empty() {
        let mut links: Vec<MenuEntry> = recent
            .iter()
            .enumerate()
            .map(|(index, shared)| MenuEntry::new(&format!("shared:{index}"), &shared.title))
            .collect();
        links.extend([
            MenuEntry::separator(),
            MenuEntry::new("clear-shared", "Clear Recently Shared"),
        ]);
        menu.extend([
            MenuEntry::separator(),
            MenuEntry {
                children: links,
                ..MenuEntry::new("shared", "Recently Shared")
            },
        ]);
    }
    menu.extend([
        MenuEntry::separator(),
        MenuEntry::new("edit", "Edit Grid"),
        MenuEntry::new("settings", "Drop Settings…"),
        MenuEntry::new("clear", "Clear Drop Bar"),
        MenuEntry::separator(),
        MenuEntry::new("quit", "Quit Drop"),
    ]);
    spec.menu = menu;
    spec
}

/// The item's menu again, as the grids changed.
pub fn refresh_item() {
    if let Some(item) = ITEM.get() {
        let attention = item.spec().is_some_and(|spec| spec.attention);
        let mut spec = spec();
        spec.attention = attention;
        item.update(spec);
    }
}

/// A cross on the item after a task failed, until the grid is next opened.
pub fn attention(on: bool) {
    let Some(item) = ITEM.get() else {
        return;
    };
    if let Some(mut spec) = item.spec()
        && spec.attention != on
    {
        spec.attention = on;
        item.update(spec);
    }
}

/// What happened to the item in the bar; on the bus's thread.
fn event(event: ItemEvent) {
    match event {
        ItemEvent::Activated {
            button: ItemButton::Left | ItemButton::Middle,
            x,
            output,
            edge,
        } => {
            attention(false);
            state::toggle(Some(x), Some(output), Some(edge));
        }
        ItemEvent::Activated { .. } => {}
        ItemEvent::DragHover {
            over: true,
            x,
            output,
            edge,
        } => state::open(Some(x), Some(output), Some(edge), true),
        ItemEvent::DragHover { over: false, .. } => state::drag_left(),
        ItemEvent::DroppedFiles(files) => {
            let name = current_grid_name();
            state::hold(&name, files);
        }
        ItemEvent::MenuSelected(id) => match id.as_str() {
            "edit" => {
                state::open(None, amane::pointer_monitor(), None, false);
                state::change(|ui| ui.editing = true);
            }
            "clear" => state::let_go(&current_grid_name(), None),
            "clear-shared" => shared::clear(),
            "settings" => amane::spawn("amane-settings --page drop"),
            "quit" => {
                if let Some(item) = ITEM.get() {
                    item.clone().remove();
                }
                // let the worker send the removal before going
                std::thread::sleep(std::time::Duration::from_millis(200));
                std::process::exit(0);
            }
            other if other.starts_with("shared:") => {
                // a link picked again goes back on the clipboard
                let index: Option<usize> = other["shared:".len()..].parse().ok();
                if let Some(shared) = index.and_then(|index| shared::list().into_iter().nth(index))
                {
                    shared::copy(&shared.link);
                }
            }
            other => {
                if let Some(index) = other
                    .strip_prefix("grid:")
                    .and_then(|index| index.parse().ok())
                {
                    grid::pick(index);
                }
            }
        },
        ItemEvent::DroppedText(_) | ItemEvent::Shown(_) => {}
    }
}

/// Arguments as files, made absolute against where the launch ran.
fn paths(args: &[String]) -> Vec<std::path::PathBuf> {
    let here = std::env::current_dir().unwrap_or_default();
    args.iter().map(|arg| here.join(arg)).collect()
}

fn current_grid_name() -> String {
    let index = state::ui().grid;
    drop_grids()
        .grids
        .get(index)
        .map(|grid| grid.name.clone())
        .unwrap_or_default()
}
