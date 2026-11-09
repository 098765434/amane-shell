//! The widgets kept: for each monitor (by connector name) where each sits
//! and how big it is, and each one's settings. Kept in the state file
//! `widgets.json` (the older `widgets`, version 1 of the same shape, is read
//! when that isn't there):
//!
//! ```json
//! { "version": 1, "data": { "next_id": 3, "outputs": { "eDP-1": [
//!     { "id": 1, "kind": "battery", "cols": 2, "rows": 2, "col": 0, "row": 0 },
//!     { "id": 2, "kind": "disk", "cols": 4, "rows": 2, "col": 2, "row": 0,
//!       "settings": { "mount": "/home" } } ] } } }
//! ```
//!
//! A monitor's layout is kept on the canvas it was last laid out on and
//! shown refitted to the one it has now (`Layout::refit`), so a smaller
//! screen or a bigger scale moves or parks widgets without losing where
//! they were: a widget parked by the refit keeps its saved cells.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use amane::serde::{Deserialize, Serialize};
use serde_json::Value;

use super::kind::{WidgetKey, WidgetKind, WidgetSize};
use crate::services::state::{self, State};
use emaki_kit::grid::geometry::{Canvas, Occupancy, Place, Rect, Span};
use emaki_kit::grid::{Entry, GridKey, Layout};

/// The file from before the state files, with the version inside.
const OLD_FILE: &str = "widgets";

/// What one widget is set to show, by its kind's settings' keys
/// (`kind::SettingSpec`): the batteries' `device`, the network's
/// `interface`, the disk's `mount`, the temperature's `sensor`, the
/// graphs' `range` (`15`), the units (`unit`: `f`, `bits`), and the clock's
/// `color` (`#rrggbb`), `line` (`thin`, `bold`), `seconds` (`on`), `hours`
/// (`12`) and `date` (`off`). A setting not kept is its automatic choice, or off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "amane::serde", transparent)]
pub struct Settings(BTreeMap<String, String>);

impl Settings {
    /// What `key` is set to; none for automatic, or off.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Sets `key` to `value`, or back to automatic (off) for none.
    pub fn set(&mut self, key: &str, value: Option<String>) {
        match value {
            Some(value) => self.0.insert(key.to_string(), value),
            None => self.0.remove(key),
        };
    }

    /// These settings with `key` set to `value`.
    #[must_use]
    pub fn with(mut self, key: &str, value: Option<String>) -> Self {
        self.set(key, value);
        self
    }

    /// Every setting kept, by key.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Only what `kind` has a setting for, each a value it takes, so a file
    /// edited by hand (or a kind that lost a setting) keeps nothing stray.
    fn for_kind(mut self, kind: WidgetKind) -> Self {
        self.0.retain(|key, value| {
            kind.setting(key)
                .is_some_and(|setting| setting.accepts(Some(value)))
        });
        self
    }
}

/// The file's shape. Each widget is read on its own and each of its fields
/// is optional, so one bad widget is dropped, not the whole file.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde", default)]
struct Kept {
    next_id: u64,
    #[serde(deserialize_with = "each_widget")]
    outputs: BTreeMap<String, Vec<KeptWidget>>,
}

/// Every monitor's widgets, leaving out (and logging) any that can't be read.
fn each_widget<'de, D>(deserializer: D) -> Result<BTreeMap<String, Vec<KeptWidget>>, D::Error>
where
    D: amane::serde::Deserializer<'de>,
{
    let outputs = BTreeMap::<String, Vec<Value>>::deserialize(deserializer)?;
    Ok(outputs
        .into_iter()
        .map(|(output, widgets)| {
            let widgets = widgets
                .into_iter()
                .filter_map(|widget| {
                    serde_json::from_value(widget)
                        .map_err(|error| {
                            log::warn!("widgets.json: {output}: a widget left out: {error}")
                        })
                        .ok()
                })
                .collect();
            (output, widgets)
        })
        .collect())
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde", default)]
struct KeptWidget {
    id: Option<u64>,
    kind: String,
    cols: Option<u64>,
    rows: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    col: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<Settings>,
}

impl State for Kept {
    const FILE: &'static str = "widgets.json";
    const VERSION: u32 = 1;

    fn legacy() -> Option<Self> {
        Self::from_old(&state::legacy_text(OLD_FILE)?)
    }
}

impl Kept {
    /// The old file: the same shape, with `"version": 1` beside it.
    fn from_old(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        if value.get("version").and_then(Value::as_u64) != Some(1) {
            return None;
        }
        serde_json::from_value(value).ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    next_id: u32,
    outputs: BTreeMap<String, Layout<WidgetKey>>,
    settings: HashMap<u32, Settings>,
    /// Whether there was a file: without one, the first monitor gets the
    /// widgets every desktop starts with.
    seeded: bool,
}

thread_local! {
    static STORE: RefCell<Option<Store>> = const { RefCell::new(None) };
}

/// Runs `f` on the store, loading it the first time.
pub fn with<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    STORE.with_borrow_mut(|store| f(store.get_or_insert_with(Store::load)))
}

/// The canvas a layout read from the file is on before it's refitted: as
/// big as anything can be.
const UNBOUNDED: Canvas = Canvas {
    cols: u8::MAX,
    rows: u8::MAX,
};

impl Store {
    pub fn empty() -> Self {
        Self {
            next_id: 1,
            outputs: BTreeMap::new(),
            settings: HashMap::new(),
            seeded: false,
        }
    }

    /// What the file keeps. One that can't be read is copied aside first
    /// (`widgets.json.bad`), so the default shown in its place, and saved
    /// once anything changes, doesn't lose it.
    fn load() -> Self {
        state::load::<Kept>()
            .map(Self::from_kept)
            .unwrap_or_else(|| {
                state::keep_unreadable::<Kept>();
                Self::empty()
            })
    }

    fn save(&self) {
        // the tests never touch the real file
        if !cfg!(test) {
            state::save(&self.to_kept());
        }
    }

    /// The store a file kept. Unknown kinds are dropped, sizes a kind lacks
    /// snap to the nearest it has, and overlaps move to the first free
    /// cells.
    fn from_kept(kept: Kept) -> Self {
        let mut store = Self::empty();
        store.seeded = true;
        let mut highest = 0;
        let small = |n: u64| n.min(255) as u8;
        for (output, widgets) in kept.outputs {
            let mut layout = Layout::new(UNBOUNDED);
            for widget in widgets {
                let Some(kind) = WidgetKind::named(&widget.kind) else {
                    continue;
                };
                let Some(id) = widget.id.and_then(|id| u32::try_from(id).ok()) else {
                    continue;
                };
                if store.settings.contains_key(&id) {
                    continue;
                }
                let key = WidgetKey { id, kind };
                let span = match (widget.cols, widget.rows) {
                    (Some(cols), Some(rows)) => key.clamp(Span::new(small(cols), small(rows))),
                    _ => kind.default_size().span(),
                };
                // kept on the canvas a u8 counts, so it never reaches past it
                let at = match (widget.col, widget.row) {
                    (Some(col), Some(row)) => Some(Place::new(
                        small(col).min(u8::MAX - span.cols),
                        small(row).min(u8::MAX - span.rows),
                    )),
                    _ => None,
                };
                layout.insert(Entry { id: key, span, at });
                store
                    .settings
                    .insert(id, widget.settings.unwrap_or_default().for_kind(kind));
                highest = highest.max(id);
            }
            layout.repair();
            store.outputs.insert(output, layout);
        }
        let next = u32::try_from(kept.next_id.max(1)).unwrap_or(u32::MAX);
        store.next_id = next.max(highest + 1);
        store
    }

    fn to_kept(&self) -> Kept {
        let outputs = self
            .outputs
            .iter()
            .map(|(output, layout)| {
                let widgets = layout
                    .entries
                    .iter()
                    .map(|entry| KeptWidget {
                        id: Some(u64::from(entry.id.id)),
                        kind: entry.id.kind.name().to_string(),
                        cols: Some(u64::from(entry.span.cols)),
                        rows: Some(u64::from(entry.span.rows)),
                        col: entry.at.map(|at| u64::from(at.col)),
                        row: entry.at.map(|at| u64::from(at.row)),
                        settings: self
                            .settings
                            .get(&entry.id.id)
                            .filter(|settings| !settings.is_empty())
                            .cloned(),
                    })
                    .collect();
                (output.clone(), widgets)
            })
            .collect();
        Kept {
            next_id: u64::from(self.next_id),
            outputs,
        }
    }

    /// `output`'s widgets on `canvas`. A desktop with no file yet starts
    /// with the battery (where there is one) and the CPU on its first
    /// monitor.
    pub fn layout_for(&mut self, output: &str, canvas: Canvas) -> Layout<WidgetKey> {
        if !self.seeded {
            self.seeded = true;
            let mut layout = Layout::new(canvas);
            for kind in [WidgetKind::Battery, WidgetKind::Cpu] {
                if kind.available() {
                    let key = self.key(kind);
                    layout.insert(Entry {
                        id: key,
                        span: kind.default_size().span(),
                        at: None,
                    });
                    layout.show(key);
                }
            }
            self.outputs.insert(output.to_string(), layout);
            self.save();
        }
        self.outputs
            .get(output)
            .map(|layout| layout.refit(canvas))
            .unwrap_or_else(|| Layout::new(canvas))
    }

    /// A new key for a `kind`.
    fn key(&mut self, kind: WidgetKind) -> WidgetKey {
        let id = self.next_id;
        self.next_id += 1;
        self.settings.insert(id, Settings::default());
        WidgetKey { id, kind }
    }

    /// Keeps `shown`, a layout of `output`'s on the screen it has now, as
    /// an edit left it: widgets it lacks are gone, new ones are added, and
    /// one moved or resized is kept where it is now. One the edit didn't
    /// touch keeps what was saved, so a widget that only moved (or hid)
    /// because the screen is smaller goes back on a bigger one.
    pub fn commit(&mut self, output: &str, shown: &Layout<WidgetKey>) {
        let before = self.layout_for(output, shown.canvas);
        let saved = self.outputs.get(output);
        let mut layout = Layout::new(UNBOUNDED);
        // what the edit placed or moved takes its cells first; one it didn't
        // touch keeps its saved cells unless they're now taken, when it goes
        // to the first free ones (or hides, as the screen shows it)
        let mut taken: Occupancy<WidgetKey> = Occupancy::new(UNBOUNDED);
        let mut untouched = Vec::new();
        for entry in &shown.entries {
            if before.entries.iter().any(|e| e == entry) {
                let kept = saved
                    .and_then(|saved| saved.entries.iter().find(|e| e.id == entry.id))
                    .copied()
                    .unwrap_or(*entry);
                untouched.push(kept);
            } else if let Some(rect) = entry.rect() {
                taken.put(entry.id, rect);
            }
        }
        for entry in &shown.entries {
            let Some(kept) = untouched.iter().find(|e| e.id == entry.id) else {
                layout.insert(*entry);
                continue;
            };
            let kept = match kept.rect() {
                Some(rect) if !taken.free(&rect, None) => Entry {
                    at: taken.first_free(kept.span),
                    ..*kept
                },
                _ => *kept,
            };
            if let Some(rect) = kept.rect() {
                taken.put(kept.id, rect);
            }
            layout.insert(kept);
        }
        let ids: Vec<u32> = self
            .outputs
            .iter()
            .filter(|(name, _)| name.as_str() != output)
            .flat_map(|(_, layout)| layout.entries.iter().map(|e| e.id.id))
            .chain(layout.entries.iter().map(|e| e.id.id))
            .collect();
        self.settings.retain(|id, _| ids.contains(id));
        self.outputs.insert(output.to_string(), layout);
        self.save();
    }

    /// Puts a new `kind` widget on `output`, `size` big, at `at` or the
    /// first free cells of `shown` (its layout now); its key, or `None`
    /// when there's no room.
    pub fn add(
        &mut self,
        output: &str,
        shown: &Layout<WidgetKey>,
        kind: WidgetKind,
        size: WidgetSize,
        at: Option<Place>,
    ) -> Option<WidgetKey> {
        let span = WidgetKey { id: 0, kind }.clamp(size.span());
        let at = match at {
            Some(at) => shown
                .occupancy()
                .free(&Rect::new(at, span), None)
                .then_some(at),
            None => shown.occupancy().first_free(span),
        }?;
        let key = self.key(kind);
        let mut next = shown.clone();
        next.insert(Entry {
            id: key,
            span,
            at: Some(at),
        });
        self.commit(output, &next);
        Some(key)
    }

    pub fn settings(&self, id: u32) -> Settings {
        self.settings.get(&id).cloned().unwrap_or_default()
    }

    /// Sets widget `id`'s settings, if it's on a desktop.
    pub fn set_settings(&mut self, id: u32, settings: Settings) {
        let placed = self
            .outputs
            .values()
            .any(|layout| layout.entries.iter().any(|e| e.id.id == id));
        if placed {
            self.settings.insert(id, settings);
            self.save();
        }
    }

    /// Every widget on every monitor, for `widgets list`.
    pub fn all(&self) -> impl Iterator<Item = (&str, &Entry<WidgetKey>)> {
        self.outputs.iter().flat_map(|(output, layout)| {
            layout
                .entries
                .iter()
                .map(move |entry| (output.as_str(), entry))
        })
    }

    /// Takes every widget off `output`.
    pub fn clear(&mut self, output: &str) {
        let canvas = self.outputs.get(output).map_or(UNBOUNDED, |l| l.canvas);
        self.commit(output, &Layout::new(canvas));
    }
}

#[cfg(test)]
mod tests;
