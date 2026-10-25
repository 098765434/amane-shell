//! Dragging a window's icon to another place in the dock: along its own
//! workspace's section to move its column there, or into another
//! workspace's section to move it to that workspace. The others make way
//! while it's held, as on macOS. A pinned app's icon dragged up off the
//! dock comes out of it.
//!
//! Every preview is worked out from the row without the held icon (the
//! base), never from the last preview, so the same place always gives the
//! same arrangement and moving back restores it. Once let go, the window is
//! shown where it went until niri says it's there.

use std::time::{Duration, Instant};

use super::model::{Icon, Section, Win};

/// How far the pointer goes with the button down before it's a drag and
/// not a click.
pub const THRESHOLD: f32 = 6.0;

/// How long a dropped window is shown where it went before niri has said.
const SETTLING: Duration = Duration::from_millis(800);

/// An icon held by the pointer.
#[derive(Clone, Debug)]
pub struct Drag {
    /// The held icon's key in the row, and the icon.
    pub key: String,
    pub icon: Icon,
    /// The pointer less the icon's top-left corner, as it was taken.
    pub grab: (f32, f32),
    /// Where the button went down, and where the pointer is now.
    pub pressed: (f32, f32),
    pub pointer: (f32, f32),
    /// Whether it has gone past [`THRESHOLD`].
    pub moved: bool,
}

impl Drag {
    /// Follows the pointer to `at`.
    pub fn to(&mut self, at: (f32, f32)) {
        self.pointer = at;
        let (dx, dy) = (at.0 - self.pressed.0, at.1 - self.pressed.1);
        if dx.hypot(dy) >= THRESHOLD {
            self.moved = true;
        }
    }

    /// The window held, if it's a window's icon.
    pub fn window(&self) -> Option<&Win> {
        match &self.icon {
            Icon::Window(window) => Some(window),
            Icon::Pin(_) => None,
        }
    }
}

/// Where a held window goes: a workspace, and its place among the windows
/// of that workspace's section.
#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub workspace: i64,
    pub place: usize,
}

/// A window let go of, shown where it went until niri has it there.
#[derive(Clone, Debug)]
pub struct Settle {
    pub window: Win,
    pub drop: Drop,
    until: Instant,
}

impl Settle {
    pub fn new(window: Win, drop: Drop) -> Self {
        Self {
            window,
            drop,
            until: Instant::now() + SETTLING,
        }
    }

    /// Whether niri has the window where it was dropped (as `sections`,
    /// straight from niri, show), or it has had long enough.
    pub fn done(&self, sections: &[Section]) -> bool {
        Instant::now() >= self.until
            || place_of(sections, self.window.id).as_ref() == Some(&self.drop)
    }
}

/// `sections` without `window`'s icon.
pub fn without(sections: &[Section], window: u64) -> Vec<Section> {
    sections
        .iter()
        .map(|section| Section {
            icons: section
                .icons
                .iter()
                .filter(|icon| !matches!(icon, Icon::Window(w) if w.id == window))
                .cloned()
                .collect(),
            ..section.clone()
        })
        .collect()
}

/// `sections` with `window`'s icon at `drop`, among the windows of its
/// workspace (never after its pinned apps).
pub fn with(sections: &[Section], window: &Win, drop: &Drop) -> Vec<Section> {
    let mut sections = without(sections, window.id);
    if let Some(section) = sections
        .iter_mut()
        .find(|section| section.workspace == Some(drop.workspace))
    {
        let place = drop.place.min(windows(section));
        section.icons.insert(place, Icon::Window(window.clone()));
    }
    sections
}

/// How many windows' icons `section` starts with.
fn windows(section: &Section) -> usize {
    section
        .icons
        .iter()
        .take_while(|icon| matches!(icon, Icon::Window(_)))
        .count()
}

/// Where `window` is in `sections`.
fn place_of(sections: &[Section], window: u64) -> Option<Drop> {
    sections.iter().find_map(|section| {
        let place = section
            .icons
            .iter()
            .position(|icon| matches!(icon, Icon::Window(w) if w.id == window))?;
        Some(Drop {
            workspace: section.workspace?,
            place,
        })
    })
}

/// Where a window let go at `along` slots along the row of `sections`
/// would go: the section of the nearest icon, between the windows either
/// side of the pointer. `rule` is how many slots a rule takes. None for a
/// workspace that isn't open.
pub fn drop_at(sections: &[Section], rule: f32, along: f32) -> Option<Drop> {
    // each section's places: where its icons' centres are along the row
    let mut slot = 0.0;
    let mut nearest: Option<(f32, usize)> = None;
    let mut centres: Vec<Vec<f32>> = Vec::with_capacity(sections.len());
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            slot += rule;
        }
        let count = section.icons.len().max(1);
        let mine: Vec<f32> = (0..count).map(|i| slot + i as f32 + 0.5).collect();
        for centre in &mine {
            let off = (centre - along).abs();
            if nearest.is_none_or(|(best, _)| off < best) {
                nearest = Some((off, index));
            }
        }
        // an empty section's one place has no icon in it
        centres.push(if section.icons.is_empty() {
            Vec::new()
        } else {
            mine
        });
        slot += count as f32;
    }
    let (_, index) = nearest?;
    let section = &sections[index];
    let before = centres[index]
        .iter()
        .filter(|centre| **centre < along)
        .count();
    Some(Drop {
        workspace: section.workspace?,
        place: before.min(windows(section)),
    })
}

/// niri's column index (from 1) for a window dropped at `place` among the
/// windows of `section`, which doesn't hold it: one past the columns of
/// the tiled windows before it.
pub fn column_index(section: &Section, place: usize) -> usize {
    let mut columns: Vec<usize> = section
        .icons
        .iter()
        .take(place)
        .filter_map(|icon| match icon {
            Icon::Window(window) => window.column.map(|(column, _)| column),
            Icon::Pin(_) => None,
        })
        .collect();
    columns.dedup();
    columns.len() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::dock::pins::Pin;

    fn win(id: u64, workspace: i64, column: usize) -> Win {
        Win {
            id,
            app_id: format!("app{id}"),
            title: String::new(),
            workspace: Some(workspace),
            focused: false,
            floating: false,
            column: Some((column, 1)),
        }
    }

    fn section(workspace: i64, icons: Vec<Icon>) -> Section {
        Section {
            workspace: Some(workspace),
            index: workspace as u32,
            name: None,
            active: false,
            icons,
        }
    }

    fn ids(section: &Section) -> Vec<String> {
        section.icons.iter().map(|icon| icon.key("s")).collect()
    }

    #[test]
    fn the_window_goes_between_the_icons_either_side_of_the_pointer() {
        let sections = [
            section(
                1,
                vec![Icon::Window(win(1, 1, 1)), Icon::Window(win(2, 1, 2))],
            ),
            section(2, vec![Icon::Window(win(3, 2, 1))]),
        ];
        // slots: 0..1, 1..2, rule 2..2.5, 2.5..3.5
        assert_eq!(
            drop_at(&sections, 0.5, 0.2),
            Some(Drop {
                workspace: 1,
                place: 0
            })
        );
        assert_eq!(
            drop_at(&sections, 0.5, 1.0),
            Some(Drop {
                workspace: 1,
                place: 1
            })
        );
        assert_eq!(
            drop_at(&sections, 0.5, 1.9),
            Some(Drop {
                workspace: 1,
                place: 2
            })
        );
        assert_eq!(
            drop_at(&sections, 0.5, 3.4),
            Some(Drop {
                workspace: 2,
                place: 1
            })
        );
    }

    #[test]
    fn an_empty_workspace_takes_it_and_a_pin_stays_last() {
        let pin = Icon::Pin(Pin::new("gimp", "eDP-1", None, 1));
        let sections = [
            section(1, vec![Icon::Window(win(1, 1, 1)), pin]),
            section(2, vec![]),
        ];
        assert_eq!(
            drop_at(&sections, 0.5, 1.9),
            Some(Drop {
                workspace: 1,
                place: 1
            })
        );
        assert_eq!(
            drop_at(&sections, 0.5, 2.8),
            Some(Drop {
                workspace: 2,
                place: 0
            })
        );

        let mut unopened = section(9, vec![]);
        unopened.workspace = None;
        assert_eq!(drop_at(&[unopened], 0.5, 0.5), None);
    }

    #[test]
    fn the_preview_moves_the_held_window_and_keeps_the_rest() {
        let held = win(1, 1, 1);
        let sections = [
            section(
                1,
                vec![Icon::Window(held.clone()), Icon::Window(win(2, 1, 2))],
            ),
            section(2, vec![Icon::Window(win(3, 2, 1))]),
        ];
        let moved = with(
            &sections,
            &held,
            &Drop {
                workspace: 2,
                place: 0,
            },
        );
        assert_eq!(ids(&moved[0]), ["win:2"]);
        assert_eq!(ids(&moved[1]), ["win:1", "win:3"]);
        let back = with(
            &moved,
            &held,
            &Drop {
                workspace: 1,
                place: 0,
            },
        );
        assert_eq!(ids(&back[0]), ids(&sections[0]));
    }

    #[test]
    fn the_column_is_one_past_the_columns_before_it() {
        // two windows stacked in column 2
        let mut stacked = win(3, 1, 2);
        stacked.column = Some((2, 2));
        let base = section(
            1,
            vec![
                Icon::Window(win(1, 1, 1)),
                Icon::Window(win(2, 1, 2)),
                Icon::Window(stacked),
            ],
        );
        assert_eq!(column_index(&base, 0), 1);
        assert_eq!(column_index(&base, 1), 2);
        assert_eq!(column_index(&base, 3), 3);
    }

    #[test]
    fn a_drag_starts_past_the_threshold() {
        let mut drag = Drag {
            key: String::from("win:1"),
            icon: Icon::Window(win(1, 1, 1)),
            grab: (0.0, 0.0),
            pressed: (10.0, 10.0),
            pointer: (10.0, 10.0),
            moved: false,
        };
        drag.to((12.0, 11.0));
        assert!(!drag.moved);
        drag.to((16.0, 10.0));
        assert!(drag.moved);
        drag.to((10.0, 10.0));
        assert!(drag.moved, "once a drag, a drag to the end");
    }

    #[test]
    fn it_settles_once_niri_has_the_window_there() {
        let held = win(1, 1, 1);
        let drop = Drop {
            workspace: 2,
            place: 0,
        };
        let settle = Settle::new(held.clone(), drop.clone());
        let before = [
            section(1, vec![Icon::Window(held.clone())]),
            section(2, vec![]),
        ];
        assert!(!settle.done(&before));
        let after = [section(1, vec![]), section(2, vec![Icon::Window(held)])];
        assert!(settle.done(&after));
    }
}
