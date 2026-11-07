//! What the grid shows now, shared between the bus thread (the bar's
//! clicks and drags), the tasks' threads and the window; and the Drop Bar,
//! kept on disk so held files outlast a restart.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use amane_status::BarEdge;

use crate::tasks::Job;

amane_kit::marker! {
    /// Anything the grid shows changed: it draws again.
    pub Changed
}

/// What files are held over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Tile(usize),
    /// The Drop Bar.
    Bar,
    /// Between the tiles, where folders and apps become tiles.
    Gap,
    /// The card around them.
    Card,
}

/// A question the grid asks before going on.
#[derive(Clone, Debug)]
pub enum Prompt {
    /// Some names are taken where files go: keep both, replace, or stop.
    Conflict { job: Job, names: Vec<String> },
    /// A new name for what Rename & Move moves.
    Rename {
        files: Vec<PathBuf>,
        folder: PathBuf,
        tile: usize,
    },
    /// Which connected phone gets what's sent (none: there's none to send to).
    Phone {
        what: crate::tasks::Payload,
        devices: Vec<(String, String)>,
        tile: usize,
    },
}

/// How a tile's last task went, shown a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Done,
    Failed,
}

#[derive(Debug)]
pub struct Ui {
    pub open: bool,
    /// Opened by files dragged over the bar's item: it takes no keys and
    /// only its card takes input, and it closes when they leave.
    pub by_drag: bool,
    /// Where the item is: its middle, its monitor, the bar's edge.
    pub x: Option<f32>,
    pub output: Option<String>,
    pub edge: BarEdge,
    /// Which grid shows.
    pub grid: usize,
    pub editing: bool,
    pub hover: Option<Target>,
    pub prompt: Option<Prompt>,
    /// The tiles working now, and how far (none while it can't say).
    pub running: HashMap<usize, Option<f32>>,
    /// The tiles that just finished, until when they show it.
    pub outcomes: HashMap<usize, (Outcome, Instant)>,
}

static UI: LazyLock<Mutex<Ui>> = LazyLock::new(|| {
    Mutex::new(Ui {
        open: false,
        by_drag: false,
        x: None,
        output: None,
        edge: BarEdge::Top,
        grid: 0,
        editing: false,
        hover: None,
        prompt: None,
        running: HashMap::new(),
        outcomes: HashMap::new(),
    })
});

pub fn ui() -> MutexGuard<'static, Ui> {
    UI.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Changes the state and draws again.
pub fn change(edit: impl FnOnce(&mut Ui)) {
    edit(&mut ui());
    Changed::changed();
}

/// How long a tile shows how its task went.
pub const OUTCOME_FOR: Duration = Duration::from_secs(2);

/// Opens the grid under the bar's item.
pub fn open(x: Option<f32>, output: Option<String>, edge: Option<BarEdge>, by_drag: bool) {
    change(|ui| {
        if x.is_some() {
            ui.x = x;
        }
        if output.is_some() {
            ui.output = output;
        }
        if let Some(edge) = edge {
            ui.edge = edge;
        }
        // a click on one opened by a drag keeps it open for keys
        ui.by_drag = by_drag && !(ui.open && !ui.by_drag);
        ui.open = true;
    });
}

pub fn close() {
    change(|ui| {
        ui.open = false;
        ui.by_drag = false;
        ui.editing = false;
        ui.hover = None;
        ui.prompt = None;
    });
}

pub fn toggle(x: Option<f32>, output: Option<String>, edge: Option<BarEdge>) {
    if ui().open {
        close();
    } else {
        open(x, output, edge, false);
    }
}

/// Files came over `target` (or left it): when nothing holds them after a
/// moment, a grid opened by a drag closes.
pub fn hover(target: Target, over: bool) {
    change(|ui| {
        if over {
            ui.hover = Some(target);
        } else if ui.hover == Some(target) {
            ui.hover = None;
        }
    });
    if !over {
        drag_left();
    }
}

/// Files left something: a grid opened by a drag closes in a moment, unless
/// they come over it again.
pub fn drag_left() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(700));
        let left = {
            let ui = ui();
            ui.open && ui.by_drag && ui.hover.is_none() && ui.prompt.is_none()
        };
        if left {
            close();
        }
    });
}

/// Asks before going on; a grid opened by a drag takes keys and clicks
/// for the answer.
pub fn ask(prompt: Prompt) {
    change(|ui| {
        ui.prompt = Some(prompt);
        ui.open = true;
        ui.by_drag = false;
    });
}

// ───── the Drop Bar ──────────────────────────────────────────────────────────

/// Each grid's held files, by the grid's name: stacks of what was let go
/// together.
pub type Shelves = BTreeMap<String, Vec<Vec<PathBuf>>>;

static SHELVES: Mutex<Option<Shelves>> = Mutex::new(None);

fn shelf_file() -> Option<PathBuf> {
    let state = amane_kit::paths::state();
    Some(state.join("amane-drop").join("dropbar.json"))
}

fn with_shelves<R>(f: impl FnOnce(&mut Shelves) -> R) -> R {
    let mut shelves = SHELVES.lock().unwrap_or_else(PoisonError::into_inner);
    let shelves = shelves.get_or_insert_with(|| {
        shelf_file()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    });
    f(shelves)
}

fn save(shelves: &Shelves) {
    let (Some(file), Ok(text)) = (shelf_file(), serde_json::to_string_pretty(shelves)) else {
        return;
    };
    let write = || {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let part = file.with_extension("part");
        std::fs::write(&part, text)?;
        std::fs::rename(part, &file)
    };
    if let Err(e) = write() {
        eprintln!("amane-drop: couldn't save {}: {e}", file.display());
    }
}

/// The stacks held on grid `grid`'s Drop Bar.
pub fn stacks(grid: &str) -> Vec<Vec<PathBuf>> {
    with_shelves(|shelves| shelves.get(grid).cloned().unwrap_or_default())
}

/// Holds `files` on the Drop Bar as a stack of their own.
pub fn hold(grid: &str, files: Vec<PathBuf>) {
    if files.is_empty() {
        return;
    }
    with_shelves(|shelves| {
        shelves.entry(grid.to_string()).or_default().push(files);
        save(shelves);
    });
    Changed::changed();
}

/// Lets go of stack `index`, or every stack.
pub fn let_go(grid: &str, index: Option<usize>) {
    with_shelves(|shelves| {
        let Some(stacks) = shelves.get_mut(grid) else {
            return;
        };
        match index {
            Some(index) if index < stacks.len() => {
                stacks.remove(index);
            }
            Some(_) => return,
            None => stacks.clear(),
        }
        save(shelves);
    });
    Changed::changed();
}
