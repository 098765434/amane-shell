//! The widgets kept: for each monitor (by connector name) where each sits
//! and how big it is, and each one's settings. Kept in
//! `$XDG_STATE_HOME/amane-shell/widgets` as JSON:
//!
//! ```json
//! { "version": 1, "next_id": 3, "outputs": { "eDP-1": [
//!     { "id": 1, "kind": "battery", "cols": 2, "rows": 2, "col": 0, "row": 0 },
//!     { "id": 2, "kind": "disk", "cols": 4, "rows": 2, "col": 2, "row": 0,
//!       "settings": { "mount": "/home" } } ] } }
//! ```
//!
//! A monitor's layout is kept on the canvas it was last laid out on and
//! shown refitted to the one it has now (`Layout::refit`), so a smaller
//! screen or a bigger scale moves or parks widgets without losing where
//! they were: a widget parked by the refit keeps its saved cells.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value, json};

use super::kind::{WidgetKey, WidgetKind, WidgetSize};
use crate::components::grid::geometry::{Canvas, Place, Rect, Span};
use crate::components::grid::{Entry, GridKey, Layout};
use crate::services::{save_state, state_file};

const FILE: &str = "widgets";
const VERSION: u64 = 1;

/// What one widget is set to show; each kind reads what's its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// Batteries: the one device a Small widget shows (a peripheral's
    /// model, a Bluetooth device's address, or "system"); none is the
    /// machine's battery, or the first device there is.
    pub device: Option<String>,
    /// Network: the interface; none follows the default route.
    pub interface: Option<String>,
    /// Disk: the mount point; none is the root, or the first disk where the root is none.
    pub mount: Option<String>,
    /// Temperature: the sensor's label; none is the hottest.
    pub sensor: Option<String>,
}

impl Settings {
    fn fields(&mut self) -> [(&'static str, &mut Option<String>); 4] {
        [
            ("device", &mut self.device),
            ("interface", &mut self.interface),
            ("mount", &mut self.mount),
            ("sensor", &mut self.sensor),
        ]
    }

    fn parse(value: Option<&Value>) -> Self {
        let mut settings = Self::default();
        if let Some(object) = value.and_then(Value::as_object) {
            for (name, field) in settings.fields() {
                *field = object.get(name).and_then(Value::as_str).map(String::from);
            }
        }
        settings
    }

    fn to_json(&self) -> Option<Value> {
        let mut object = Map::new();
        for (name, field) in self.clone().fields() {
            if let Some(value) = field {
                object.insert(name.to_string(), json!(value));
            }
        }
        (!object.is_empty()).then_some(Value::Object(object))
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

    fn load() -> Self {
        std::fs::read_to_string(state_file(FILE))
            .ok()
            .and_then(|text| Self::parse(&text))
            .unwrap_or_else(Self::empty)
    }

    fn save(&self) {
        // the tests never touch the real file
        if !cfg!(test) {
            save_state(FILE, &self.to_json());
        }
    }

    /// `None` for anything but a current file. Unknown kinds are dropped,
    /// sizes a kind lacks snap to the nearest it has, and overlaps move
    /// to the first free cells.
    pub fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        if value.get("version").and_then(Value::as_u64) != Some(VERSION) {
            return None;
        }
        let mut store = Self::empty();
        store.seeded = true;
        let mut highest = 0;
        for (output, widgets) in value.get("outputs")?.as_object()? {
            let mut layout = Layout::new(UNBOUNDED);
            for widget in widgets.as_array().into_iter().flatten() {
                let number = |key: &str| widget.get(key).and_then(Value::as_u64);
                let small = |n: u64| n.min(255) as u8;
                let Some(kind) = widget
                    .get("kind")
                    .and_then(Value::as_str)
                    .and_then(WidgetKind::named)
                else {
                    continue;
                };
                let Some(id) = number("id").and_then(|id| u32::try_from(id).ok()) else {
                    continue;
                };
                if store.settings.contains_key(&id) {
                    continue;
                }
                let key = WidgetKey { id, kind };
                let span = match (number("cols"), number("rows")) {
                    (Some(cols), Some(rows)) => key.clamp(Span::new(small(cols), small(rows))),
                    _ => kind.default_size().span(),
                };
                let at = match (number("col"), number("row")) {
                    (Some(col), Some(row)) => Some(Place::new(small(col), small(row))),
                    _ => None,
                };
                layout.insert(Entry { id: key, span, at });
                store
                    .settings
                    .insert(id, Settings::parse(widget.get("settings")));
                highest = highest.max(id);
            }
            layout.repair();
            store.outputs.insert(output.clone(), layout);
        }
        let next = value.get("next_id").and_then(Value::as_u64).unwrap_or(1);
        store.next_id = u32::try_from(next).unwrap_or(u32::MAX).max(highest + 1);
        Some(store)
    }

    pub fn to_json(&self) -> String {
        let outputs: Map<String, Value> = self
            .outputs
            .iter()
            .map(|(output, layout)| {
                let widgets: Vec<Value> = layout
                    .entries
                    .iter()
                    .map(|entry| {
                        let mut widget = json!({
                            "id": entry.id.id,
                            "kind": entry.id.kind.name(),
                            "cols": entry.span.cols,
                            "rows": entry.span.rows,
                        });
                        if let Some(at) = entry.at {
                            widget["col"] = json!(at.col);
                            widget["row"] = json!(at.row);
                        }
                        if let Some(settings) =
                            self.settings.get(&entry.id.id).and_then(Settings::to_json)
                        {
                            widget["settings"] = settings;
                        }
                        widget
                    })
                    .collect();
                (output.clone(), Value::Array(widgets))
            })
            .collect();
        serde_json::to_string_pretty(&json!({
            "version": VERSION,
            "next_id": self.next_id,
            "outputs": outputs,
        }))
        .unwrap_or_default()
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
        for entry in &shown.entries {
            let untouched = before.entries.iter().any(|e| e == entry);
            let kept = saved
                .filter(|_| untouched)
                .and_then(|saved| saved.entries.iter().find(|e| e.id == entry.id))
                .copied()
                .unwrap_or(*entry);
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
mod tests {
    use super::*;

    const SCREEN: Canvas = Canvas { cols: 20, rows: 11 };

    fn store() -> Store {
        let mut store = Store::empty();
        store.seeded = true;
        store
    }

    #[test]
    fn the_file_round_trips() {
        let mut store = store();
        let layout = Layout::new(SCREEN);
        let cpu = store
            .add("eDP-1", &layout, WidgetKind::Cpu, WidgetSize::Medium, None)
            .unwrap();
        let shown = store.layout_for("eDP-1", SCREEN);
        let disk = store
            .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
            .unwrap();
        store.set_settings(
            disk.id,
            Settings {
                mount: Some(String::from("/home")),
                ..Settings::default()
            },
        );
        let parsed = Store::parse(&store.to_json()).expect("a store");
        assert_eq!(parsed, store);
        let shown = parsed.clone().layout_for("eDP-1", SCREEN);
        assert_eq!(
            shown.rect_of(cpu),
            Some(Rect::new(Place::new(0, 0), Span::new(4, 2)))
        );
        assert_eq!(shown.rect_of(disk).map(|r| r.at), Some(Place::new(4, 0)));
        assert_eq!(parsed.settings(disk.id).mount.as_deref(), Some("/home"));
        assert_eq!(Store::parse(r#"{"version":0,"outputs":{}}"#), None);
        assert_eq!(Store::parse("not json"), None);
    }

    #[test]
    fn a_file_is_repaired() {
        let text = r#"{"version":1,"next_id":2,"outputs":{"DP-1":[
            {"id":5,"kind":"cpu","cols":2,"rows":2,"col":0,"row":0},
            {"id":6,"kind":"memory","cols":2,"rows":2,"col":1,"row":1},
            {"id":7,"kind":"teleport","cols":2,"rows":2,"col":8,"row":0},
            {"id":5,"kind":"disk","cols":2,"rows":2,"col":8,"row":0},
            {"id":8,"kind":"disk","cols":8,"rows":4,"col":10,"row":0}
        ]}}"#;
        let mut store = Store::parse(text).expect("a store");
        let layout = store.layout_for("DP-1", SCREEN);
        assert_eq!(layout.entries.len(), 3);
        // the overlap moved, a size Disk lacks snapped to one it has
        let memory = WidgetKey {
            id: 6,
            kind: WidgetKind::Memory,
        };
        assert_eq!(layout.rect_of(memory).map(|r| r.at), Some(Place::new(2, 0)));
        let disk = WidgetKey {
            id: 8,
            kind: WidgetKind::Disk,
        };
        assert_eq!(layout.span_of(disk), Some(WidgetSize::Medium.span()));
        // ids never repeat
        assert_eq!(store.next_id, 9);
    }

    #[test]
    fn a_smaller_screen_moves_widgets_without_losing_their_place() {
        let mut store = store();
        let big = Layout::new(SCREEN);
        let far = store
            .add(
                "eDP-1",
                &big,
                WidgetKind::Cpu,
                WidgetSize::Small,
                Some(Place::new(18, 9)),
            )
            .unwrap();
        // shown on a small screen it moves to where there's room
        let small = Canvas { cols: 4, rows: 2 };
        let shown = store.layout_for("eDP-1", small);
        assert_eq!(shown.rect_of(far).map(|r| r.at), Some(Place::new(0, 0)));
        // a widget added there is kept, and the far one keeps its cells
        let near = store
            .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
            .unwrap();
        assert_eq!(
            store.layout_for("eDP-1", small).rect_of(near).map(|r| r.at),
            Some(Place::new(2, 0))
        );
        let back = store.layout_for("eDP-1", SCREEN);
        assert_eq!(back.rect_of(far).map(|r| r.at), Some(Place::new(18, 9)));
        assert_eq!(back.rect_of(near).map(|r| r.at), Some(Place::new(2, 0)));
        // with no room at all a widget hides there, and still comes back
        let tiny = store.layout_for("eDP-1", Canvas { cols: 2, rows: 2 });
        assert_eq!(tiny.placed().count(), 1);
        store.commit("eDP-1", &tiny);
        assert_eq!(store.layout_for("eDP-1", SCREEN), back);
        // a widget moved by hand on the small screen is kept where it went
        let moved = store
            .layout_for("eDP-1", small)
            .moved(near, Place::new(0, 0))
            .unwrap();
        store.commit("eDP-1", &moved);
        let back = store.layout_for("eDP-1", SCREEN);
        assert_eq!(back.rect_of(near).map(|r| r.at), Some(Place::new(0, 0)));
    }

    #[test]
    fn removing_a_widget_forgets_its_settings() {
        let mut store = store();
        let shown = Layout::new(SCREEN);
        let key = store
            .add("eDP-1", &shown, WidgetKind::Disk, WidgetSize::Small, None)
            .unwrap();
        assert!(store.settings.contains_key(&key.id));
        let mut shown = store.layout_for("eDP-1", SCREEN);
        shown.remove(key);
        store.commit("eDP-1", &shown);
        assert!(!store.settings.contains_key(&key.id));
        assert!(store.all().next().is_none());
    }
}
