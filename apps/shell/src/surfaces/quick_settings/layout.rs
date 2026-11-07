//! The layout — where each control sits on the canvas and how big it is,
//! as edit mode leaves it. Kept in the state file `quick-settings.json`
//! (the older `quick-settings`, version 2 of the same shape, is read when
//! that isn't there); absent or unreadable, it's the default order packed
//! onto the canvas. A control the file
//! doesn't know, or one whose hardware shows up later, waits in the Add
//! list rather than tearing a hole in the canvas.
//!
//! The layout itself, and the moves and resizes edit mode previews and
//! commits, are the shared grid's (`amane_kit::grid::layout`); what's
//! Quick Settings' own is the default, the file, and that every control
//! has an entry.

use std::cell::RefCell;

use amane::serde::{Deserialize, Serialize};
use serde_json::Value;

use super::controls::ControlId;
use super::grid::{Canvas, Place, Span};
use crate::services::state::{self, State};
use crate::surfaces::quick_settings::grid::FromTokens as _;
use amane_kit::grid::{self, GridKey};

/// The file from before the state files, with the version inside.
const OLD_FILE: &str = "quick-settings";
/// The old file's version this reads; older ones were never moved on.
const OLD_VERSION: u64 = 2;

/// The file's shape. Each field is optional so one bad control is dropped,
/// not the whole file.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde", default)]
pub struct Kept {
    controls: Vec<KeptControl>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(crate = "amane::serde", default)]
struct KeptControl {
    id: String,
    cols: Option<u64>,
    rows: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    col: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<u64>,
}

impl State for Kept {
    const FILE: &'static str = "quick-settings.json";
    const VERSION: u32 = 1;

    fn legacy() -> Option<Self> {
        Self::from_old(&state::legacy_text(OLD_FILE)?)
    }
}

impl Kept {
    /// The old file: the same shape, with `"version": 2` beside it.
    fn from_old(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        if value.get("version").and_then(Value::as_u64) != Some(OLD_VERSION) {
            return None;
        }
        serde_json::from_value(value).ok()
    }
}

/// A control's place and size, or that it waits in the Add list.
pub type Entry = grid::Entry<ControlId>;

pub type Layout = grid::Layout<ControlId>;

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

/// What Quick Settings keeps of its layout: the default, the file, and that every control has
/// an entry.
pub trait Stored: Sized {
    /// The default order packed onto `canvas`, each control at its size, skipping what isn't
    /// `present` and what hides by default.
    fn default_for(canvas: Canvas, present: &dyn Fn(ControlId) -> bool) -> Self;
    /// The layout file, or the default when there's none.
    fn load(canvas: Canvas, present: &dyn Fn(ControlId) -> bool) -> Self;
    /// Writes the layout file.
    fn save(&self);
    /// The layout a file kept, on `canvas`.
    fn from_kept(canvas: Canvas, kept: Kept) -> Self;
    /// Makes the entries a layout: every control placed where it fits, or hidden.
    fn reconcile(&mut self);
    /// What the file keeps of the layout.
    fn to_kept(&self) -> Kept;
}

impl Stored for Layout {
    /// The default order packed onto `canvas`, each control at its
    /// size, skipping what isn't `present` and what hides by default.
    fn default_for(canvas: Canvas, present: &dyn Fn(ControlId) -> bool) -> Self {
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

    fn load(canvas: Canvas, present: &dyn Fn(ControlId) -> bool) -> Self {
        state::load::<Kept>()
            .map(|kept| Self::from_kept(canvas, kept))
            .unwrap_or_else(|| Self::default_for(canvas, present))
    }

    fn save(&self) {
        state::save(&self.to_kept());
    }

    /// What the file places is kept where it fits, unknown names are
    /// dropped, and every other control is hidden.
    fn from_kept(canvas: Canvas, kept: Kept) -> Self {
        let mut entries: Vec<Entry> = Vec::new();
        for control in kept.controls {
            let Some(id) = ControlId::named(&control.id) else {
                continue;
            };
            if entries.iter().any(|entry| entry.id == id) {
                continue;
            }
            let span = match (control.cols, control.rows) {
                (Some(cols), Some(rows)) => id.clamp(Span {
                    cols: cols.min(255) as u8,
                    rows: rows.min(255) as u8,
                }),
                _ => id.default_span(),
            };
            let at = match (control.col, control.row) {
                (Some(col), Some(row)) => Some(Place::new(col.min(255) as u8, row.min(255) as u8)),
                _ => None,
            };
            entries.push(Entry { id, span, at });
        }

        let mut layout = Self { canvas, entries };
        layout.reconcile();
        layout
    }

    /// Makes the entries a layout: anything off the canvas or over another
    /// control moves to the first free cells (or hides), and every control
    /// the file lacks is added, hidden.
    fn reconcile(&mut self) {
        self.repair();
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

    fn to_kept(&self) -> Kept {
        let controls = self
            .entries
            .iter()
            .map(|entry| KeptControl {
                id: entry.id.name().to_string(),
                cols: Some(u64::from(entry.span.cols)),
                rows: Some(u64::from(entry.span.rows)),
                col: entry.at.map(|at| u64::from(at.col)),
                row: entry.at.map(|at| u64::from(at.row)),
            })
            .collect();
        Kept { controls }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surfaces::quick_settings::grid::{Rect, Sizes as _};

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
    fn the_file_round_trips_and_older_old_files_are_ignored() {
        let mut layout = default();
        layout.hide(ControlId::Lock);
        let text = serde_json::to_string(&layout.to_kept()).unwrap();
        let parsed = Layout::from_kept(CANVAS, serde_json::from_str(&text).unwrap());
        assert_eq!(parsed, layout);
        assert!(Kept::from_old(r#"{"version":1,"controls":[]}"#).is_none());
        assert!(Kept::from_old("not json").is_none());
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
        let layout = Layout::from_kept(CANVAS, Kept::from_old(text).expect("an old file"));
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
    fn a_hidden_control_is_placed_by_hand_pushing_or_refusing() {
        let layout = default();
        assert_eq!(at(&layout, ControlId::Vpn), None);
        // onto free cells it just goes there, at the size asked for
        let placed = layout
            .placed_at(ControlId::Vpn, Span::M, Place::new(0, 7))
            .unwrap();
        assert_eq!(
            placed.rect_of(ControlId::Vpn),
            Some(Rect::new(Place::new(0, 7), Span::M))
        );
        // onto the two round buttons: they go to the first free cells, at
        // the bottom (it had no cells of its own to give them)
        let placed = layout
            .placed_at(ControlId::Vpn, Span::M, Place::new(2, 2))
            .unwrap();
        assert_eq!(at(&placed, ControlId::Vpn), Some(Place::new(2, 2)));
        assert_eq!(at(&placed, ControlId::DarkStyle), Some(Place::new(1, 7)));
        assert_eq!(at(&placed, ControlId::NightLight), Some(Place::new(2, 7)));
        // a size it lacks snaps to one it has
        let placed = layout
            .placed_at(ControlId::Vpn, Span::S, Place::new(0, 7))
            .unwrap();
        assert_eq!(placed.span_of(ControlId::Vpn), Some(Span::M));
        // off the canvas it can't go
        assert_eq!(
            layout.placed_at(ControlId::Vpn, Span::M, Place::new(3, 7)),
            None
        );
        // one already on the canvas is simply moved
        let moved = layout
            .placed_at(ControlId::Wifi, Span::M, Place::new(0, 1))
            .unwrap();
        assert_eq!(
            moved,
            layout.moved(ControlId::Wifi, Place::new(0, 1)).unwrap()
        );
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

    #[test]
    fn a_live_old_file_migrates() {
        let old = include_str!("../../../tests/fixtures/state/quick-settings");
        crate::services::state::tests::in_folder("quick-settings", &[(OLD_FILE, old)], |_| {
            let layout = Layout::from_kept(CANVAS, state::load::<Kept>().expect("the old file"));
            assert_eq!(layout.entries.len(), ControlId::ALL.len());
            let back = Layout::from_kept(CANVAS, layout.to_kept());
            assert_eq!(back, layout);
        });
    }
}
