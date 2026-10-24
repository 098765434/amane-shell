//! The layout — where each control sits on the canvas and how big it is,
//! as edit mode leaves it. Kept in `$XDG_STATE_HOME/amane-shell/
//! quick-settings` as JSON; absent, unreadable or from an older version,
//! it's Control Center's order packed onto the canvas. A control the file
//! doesn't know, or one whose hardware shows up later, waits in the Add
//! list rather than tearing a hole in the canvas.
//!
//! Moves and resizes are worked out on a copy (`moved`, `resized`): the
//! controls in the way swap with the moving one when it covers just one
//! its own size, else they're pushed to the first free cells; when one
//! can't fit, nothing moves. Edit mode previews those copies as it drags
//! and commits one on the drop.

use std::cell::RefCell;

use serde_json::{Value, json};

use super::controls::ControlId;
use super::grid::{Canvas, Occupancy, Place, Rect, Span};
use crate::services::{save_state, state_file};

const FILE: &str = "quick-settings";
const VERSION: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: ControlId,
    pub span: Span,
    /// Its cell, or `None` when it's hidden (in the Add list).
    pub at: Option<Place>,
}

impl Entry {
    pub fn rect(&self) -> Option<Rect> {
        self.at.map(|at| Rect::new(at, self.span))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub canvas: Canvas,
    pub entries: Vec<Entry>,
}

thread_local! {
    static LAYOUT: RefCell<Option<Layout>> = const { RefCell::new(None) };
}

/// Loads the layout the first time, packing the default for the controls
/// `present` when there's no file.
pub fn load_once(present: &dyn Fn(ControlId) -> bool) {
    LAYOUT.with_borrow_mut(|layout| {
        if layout.is_none() {
            *layout = Some(Layout::load(Canvas::tokens(), present));
        }
    });
}

/// Runs `f` on the layout (loaded with every control present, if nothing
/// loaded it yet).
pub fn with<R>(f: impl FnOnce(&mut Layout) -> R) -> R {
    load_once(&|_| true);
    LAYOUT.with_borrow_mut(|layout| f(layout.as_mut().expect("loaded")))
}

/// Puts `layout` in place of the current one, and keeps it.
pub fn replace(layout: Layout) {
    layout.save();
    LAYOUT.with_borrow_mut(|slot| *slot = Some(layout));
}

impl Layout {
    /// Control Center's order packed onto `canvas`, each control at its
    /// size, skipping what isn't `present` and what hides by default.
    pub fn default_for(canvas: Canvas, present: &dyn Fn(ControlId) -> bool) -> Self {
        let mut layout = Self {
            canvas,
            entries: ControlId::ALL
                .into_iter()
                .map(|id| Entry {
                    id,
                    span: id.default_span(),
                    at: None,
                })
                .collect(),
        };
        for id in ControlId::ALL {
            if present(id) && !id.hidden_by_default() {
                layout.show(id);
            }
        }
        layout
    }

    pub fn load(canvas: Canvas, present: &dyn Fn(ControlId) -> bool) -> Self {
        std::fs::read_to_string(state_file(FILE))
            .ok()
            .and_then(|text| Self::parse(canvas, &text))
            .unwrap_or_else(|| Self::default_for(canvas, present))
    }

    pub fn save(&self) {
        save_state(FILE, &self.to_json());
    }

    /// `None` for anything but a current layout file. What it places is
    /// kept where it fits, unknown names are dropped, and every other
    /// control is hidden.
    pub fn parse(canvas: Canvas, text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        if value.get("version").and_then(Value::as_u64) != Some(VERSION) {
            return None;
        }
        let controls = value.get("controls")?.as_array()?;

        let mut entries: Vec<Entry> = Vec::new();
        for control in controls {
            let Some(id) = control
                .get("id")
                .and_then(Value::as_str)
                .and_then(ControlId::named)
            else {
                continue;
            };
            if entries.iter().any(|entry| entry.id == id) {
                continue;
            }
            let number = |key: &str| control.get(key).and_then(Value::as_u64);
            let span = match (number("cols"), number("rows")) {
                (Some(cols), Some(rows)) => id.clamp(Span {
                    cols: cols.min(255) as u8,
                    rows: rows.min(255) as u8,
                }),
                _ => id.default_span(),
            };
            let at = match (number("col"), number("row")) {
                (Some(col), Some(row)) => Some(Place::new(col.min(255) as u8, row.min(255) as u8)),
                _ => None,
            };
            entries.push(Entry { id, span, at });
        }

        let mut layout = Self { canvas, entries };
        layout.reconcile();
        Some(layout)
    }

    /// Makes the entries a layout: anything off the canvas or over another
    /// control moves to the first free cells (or hides), and every control
    /// the file lacks is added, hidden.
    fn reconcile(&mut self) {
        let mut occupancy: Occupancy<ControlId> = Occupancy::new(self.canvas);
        for i in 0..self.entries.len() {
            let entry = self.entries[i];
            let Some(rect) = entry.rect() else {
                continue;
            };
            let at = if occupancy.free(&rect, None) {
                Some(rect.at)
            } else {
                occupancy.first_free(entry.span)
            };
            self.entries[i].at = at;
            if let Some(at) = at {
                occupancy.put(entry.id, Rect::new(at, entry.span));
            }
        }
        for id in ControlId::ALL {
            if !self.entries.iter().any(|entry| entry.id == id) {
                self.entries.push(Entry {
                    id,
                    span: id.default_span(),
                    at: None,
                });
            }
        }
    }

    pub fn to_json(&self) -> String {
        let controls: Vec<Value> = self
            .entries
            .iter()
            .map(|entry| {
                let mut control = json!({
                    "id": entry.id.name(),
                    "cols": entry.span.cols,
                    "rows": entry.span.rows,
                });
                if let Some(at) = entry.at {
                    control["col"] = json!(at.col);
                    control["row"] = json!(at.row);
                }
                control
            })
            .collect();
        serde_json::to_string_pretty(&json!({ "version": VERSION, "controls": controls }))
            .unwrap_or_default()
    }

    /// The controls on the canvas, in layout order.
    pub fn placed(&self) -> impl Iterator<Item = (ControlId, Rect)> + '_ {
        self.entries
            .iter()
            .filter_map(|entry| entry.rect().map(|rect| (entry.id, rect)))
    }

    /// The controls in the Add list.
    pub fn hidden(&self) -> impl Iterator<Item = ControlId> + '_ {
        self.entries
            .iter()
            .filter(|entry| entry.at.is_none())
            .map(|entry| entry.id)
    }

    pub fn rect_of(&self, id: ControlId) -> Option<Rect> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(Entry::rect)
    }

    pub fn occupancy(&self) -> Occupancy<ControlId> {
        Occupancy::of(self.canvas, self.placed())
    }

    /// How many rows the canvas needs to show everything placed.
    #[cfg(test)]
    pub fn rows_used(&self) -> u8 {
        self.occupancy().rows_used()
    }

    fn entry(&mut self, id: ControlId) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    pub fn hide(&mut self, id: ControlId) {
        if let Some(entry) = self.entry(id) {
            entry.at = None;
        }
    }

    /// Puts `id` in the first free cells it fits, at its size or failing
    /// that its smallest; whether there was room.
    pub fn show(&mut self, id: ControlId) -> bool {
        let occupancy = self.occupancy();
        let Some(entry) = self.entry(id) else {
            return false;
        };
        if entry.at.is_some() {
            return true;
        }
        let smallest = id.supported().first().copied().unwrap_or(entry.span);
        for span in [entry.span, smallest] {
            if let Some(at) = occupancy.first_free(span) {
                entry.span = span;
                entry.at = Some(at);
                return true;
            }
        }
        false
    }

    /// This layout with `id` moved so its top-left is `at`, the controls
    /// in its way swapped or pushed aside; `None` when that can't be done.
    pub fn moved(&self, id: ControlId, at: Place) -> Option<Layout> {
        let rect = self.rect_of(id)?;
        self.settled(id, Rect::new(at, rect.span))
    }

    /// This layout with `id` at `span` (or the nearest size it comes in),
    /// shifted up or left to stay on the canvas; `None` when it can't be.
    pub fn resized(&self, id: ControlId, span: Span) -> Option<Layout> {
        let rect = self.rect_of(id)?;
        let span = id.clamp(span);
        let at = self
            .canvas
            .clamp(i32::from(rect.at.col), i32::from(rect.at.row), span);
        self.settled(id, Rect::new(at, span))
    }

    /// This layout with `id` at `rect`, making room for it.
    fn settled(&self, id: ControlId, rect: Rect) -> Option<Layout> {
        if !self.canvas.fits(&rect) {
            return None;
        }
        let before = self.rect_of(id)?;
        let mut next = self.clone();
        next.entry(id)?.span = rect.span;
        next.entry(id)?.at = Some(rect.at);
        if before == rect {
            return Some(next);
        }

        let others = Occupancy::of(self.canvas, self.placed().filter(|(other, _)| *other != id));
        let covered = others.covered(&rect, None);

        // one control its own size under it changes places with it
        if let [other] = covered[..]
            && let Some(theirs) = self.rect_of(other)
            && theirs.span == before.span
            && rect.span == before.span
        {
            let swapped = Rect::new(before.at, theirs.span);
            let mut rest = Occupancy::of(
                self.canvas,
                self.placed().filter(|(o, _)| *o != id && *o != other),
            );
            rest.put(id, rect);
            if rest.free(&swapped, None) {
                next.entry(other)?.at = Some(swapped.at);
                return Some(next);
            }
        }

        // else what's under it goes to the first free cells, in order
        let mut taken = Occupancy::of(
            self.canvas,
            self.placed()
                .filter(|(o, _)| !covered.contains(o) && *o != id),
        );
        taken.put(id, rect);
        for other in covered {
            let span = self.rect_of(other)?.span;
            let at = taken.first_free(span)?;
            taken.put(other, Rect::new(at, span));
            next.entry(other)?.at = Some(at);
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANVAS: Canvas = Canvas { cols: 4, rows: 8 };

    fn all(_: ControlId) -> bool {
        true
    }

    fn default() -> Layout {
        Layout::default_for(CANVAS, &all)
    }

    fn at(layout: &Layout, id: ControlId) -> Option<Place> {
        layout.rect_of(id).map(|rect| rect.at)
    }

    #[test]
    fn the_default_is_control_center_packed_without_holes() {
        let layout = default();
        assert_eq!(layout.entries.len(), ControlId::ALL.len());
        assert_eq!(at(&layout, ControlId::Wifi), Some(Place::new(0, 0)));
        assert_eq!(at(&layout, ControlId::Bluetooth), Some(Place::new(0, 1)));
        assert_eq!(at(&layout, ControlId::NowPlaying), Some(Place::new(2, 0)));
        assert_eq!(at(&layout, ControlId::Focus), Some(Place::new(0, 2)));
        assert_eq!(at(&layout, ControlId::DarkStyle), Some(Place::new(2, 2)));
        assert_eq!(at(&layout, ControlId::NightLight), Some(Place::new(3, 2)));
        assert_eq!(at(&layout, ControlId::Display), Some(Place::new(0, 3)));
        assert_eq!(at(&layout, ControlId::Sound), Some(Place::new(0, 4)));
        assert_eq!(at(&layout, ControlId::Battery), Some(Place::new(0, 5)));
        assert_eq!(at(&layout, ControlId::Airplane), Some(Place::new(2, 5)));
        // VPN, the cable and Tailscale wait in the Add list
        assert_eq!(at(&layout, ControlId::Vpn), None);
        assert_eq!(at(&layout, ControlId::Tailscale), None);
        assert!(layout.rows_used() <= CANVAS.rows);
        // a machine without a battery or a screen dimmer closes the gaps
        let laptopless = Layout::default_for(CANVAS, &|id| {
            !matches!(id, ControlId::Battery | ControlId::Display)
        });
        assert_eq!(at(&laptopless, ControlId::Sound), Some(Place::new(0, 3)));
        assert_eq!(at(&laptopless, ControlId::Airplane), Some(Place::new(0, 4)));
    }

    #[test]
    fn the_file_round_trips_and_old_files_are_ignored() {
        let mut layout = default();
        layout.hide(ControlId::Lock);
        let parsed = Layout::parse(CANVAS, &layout.to_json()).expect("a layout");
        assert_eq!(parsed, layout);
        assert_eq!(
            Layout::parse(CANVAS, r#"{"version":1,"controls":[]}"#),
            None
        );
        assert_eq!(Layout::parse(CANVAS, "not json"), None);
    }

    #[test]
    fn a_file_is_repaired_and_the_rest_hidden() {
        let text = r#"{"version":2,"controls":[
            {"id":"sound","cols":4,"rows":1,"col":0,"row":0},
            {"id":"focus","cols":2,"rows":1,"col":1,"row":0},
            {"id":"teleport","cols":1,"rows":1,"col":0,"row":1},
            {"id":"lock","cols":9,"rows":1,"col":3,"row":7},
            {"id":"display","cols":3,"rows":1}
        ]}"#;
        let layout = Layout::parse(CANVAS, text).expect("a layout");
        assert_eq!(layout.entries.len(), ControlId::ALL.len());
        assert_eq!(at(&layout, ControlId::Sound), Some(Place::new(0, 0)));
        // Focus overlapped Sound, so it moved to the first free cells
        assert_eq!(at(&layout, ControlId::Focus), Some(Place::new(0, 1)));
        // a size Lock lacks snaps to one it has, and off the canvas it moves
        let lock = layout.rect_of(ControlId::Lock).unwrap();
        assert_eq!(lock.span, Span::M);
        assert_eq!(lock.at, Place::new(2, 1));
        assert_eq!(at(&layout, ControlId::Display), None);
        assert_eq!(at(&layout, ControlId::Wifi), None);
        assert_eq!(layout.rect_of(ControlId::Display).map(|r| r.span), None);
    }

    #[test]
    fn showing_takes_the_first_free_cells_or_shrinks() {
        let mut layout = default();
        layout.hide(ControlId::DarkStyle);
        assert_eq!(at(&layout, ControlId::DarkStyle), None);
        assert!(layout.show(ControlId::DarkStyle));
        assert_eq!(at(&layout, ControlId::DarkStyle), Some(Place::new(2, 2)));
        assert!(layout.show(ControlId::Vpn));
        let vpn = layout.rect_of(ControlId::Vpn).unwrap();
        assert_eq!(vpn.span, Span::M);
        // fill the canvas, then a wide control only fits small, then not at all
        let mut full = Layout {
            canvas: Canvas { cols: 4, rows: 1 },
            entries: vec![
                Entry {
                    id: ControlId::Sound,
                    span: Span::W3,
                    at: Some(Place::new(0, 0)),
                },
                Entry {
                    id: ControlId::Focus,
                    span: Span::M,
                    at: None,
                },
                Entry {
                    id: ControlId::Lock,
                    span: Span::S,
                    at: None,
                },
            ],
        };
        assert!(!full.show(ControlId::Focus));
        assert!(full.show(ControlId::Lock));
        assert!(!full.show(ControlId::Focus));
    }

    #[test]
    fn moving_onto_free_cells_just_moves() {
        let layout = default();
        let moved = layout.moved(ControlId::Focus, Place::new(0, 7)).unwrap();
        assert_eq!(at(&moved, ControlId::Focus), Some(Place::new(0, 7)));
        assert_eq!(at(&moved, ControlId::DarkStyle), Some(Place::new(2, 2)));
        assert_eq!(layout.moved(ControlId::Focus, Place::new(3, 7)), None);
    }

    #[test]
    fn moving_onto_one_of_its_size_swaps() {
        let layout = default();
        let moved = layout.moved(ControlId::Wifi, Place::new(0, 1)).unwrap();
        assert_eq!(at(&moved, ControlId::Wifi), Some(Place::new(0, 1)));
        assert_eq!(at(&moved, ControlId::Bluetooth), Some(Place::new(0, 0)));
        let moved = layout
            .moved(ControlId::DarkStyle, Place::new(3, 2))
            .unwrap();
        assert_eq!(at(&moved, ControlId::NightLight), Some(Place::new(2, 2)));
    }

    #[test]
    fn moving_onto_others_pushes_them_to_free_cells() {
        let layout = default();
        // Focus (2×1) onto the two round buttons: they go to the first free cells
        let moved = layout.moved(ControlId::Focus, Place::new(2, 2)).unwrap();
        assert_eq!(at(&moved, ControlId::Focus), Some(Place::new(2, 2)));
        assert_eq!(at(&moved, ControlId::DarkStyle), Some(Place::new(0, 2)));
        assert_eq!(at(&moved, ControlId::NightLight), Some(Place::new(1, 2)));
        // Battery (2×1) onto the two buttons beside it: they take its old cells
        let moved = layout.moved(ControlId::Battery, Place::new(2, 5)).unwrap();
        assert_eq!(at(&moved, ControlId::Airplane), Some(Place::new(0, 5)));
        assert_eq!(at(&moved, ControlId::PowerMode), Some(Place::new(1, 5)));
        // Now Playing (2×2) onto Focus, a button and Display: Display has
        // no whole row left to go to, so nothing moves
        assert_eq!(layout.moved(ControlId::NowPlaying, Place::new(1, 2)), None);
    }

    #[test]
    fn resizing_makes_room_or_refuses() {
        let layout = default();
        let wider = layout.resized(ControlId::Focus, Span::W3).unwrap();
        assert_eq!(wider.rect_of(ControlId::Focus).unwrap().span, Span::W3);
        // Dark Style was under it and goes to the first free cell; Night Light stays
        assert_eq!(at(&wider, ControlId::DarkStyle), Some(Place::new(1, 7)));
        assert_eq!(at(&wider, ControlId::NightLight), Some(Place::new(3, 2)));
        // a size it lacks snaps to one it has; off the edge it shifts left
        let full = layout.resized(ControlId::NightLight, Span::M).unwrap();
        assert_eq!(
            full.rect_of(ControlId::NightLight).unwrap(),
            Rect::new(Place::new(2, 2), Span::M)
        );
        // a canvas with no room refuses
        let tight = Layout {
            canvas: Canvas { cols: 4, rows: 1 },
            entries: vec![
                Entry {
                    id: ControlId::Sound,
                    span: Span::W3,
                    at: Some(Place::new(0, 0)),
                },
                Entry {
                    id: ControlId::Lock,
                    span: Span::S,
                    at: Some(Place::new(3, 0)),
                },
            ],
        };
        assert_eq!(tight.resized(ControlId::Sound, Span::FULL), None);
        assert_eq!(tight.resized(ControlId::Lock, Span::M), None);
    }
}
