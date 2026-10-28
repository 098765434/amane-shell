//! The macOS magnification, as plain layout: how big each icon of a row is
//! with the pointer at some place along it, and where that puts them.
//!
//! The pointer is measured against the row at rest, never the swollen row
//! it's looking at, so the swell can't feed back on itself (the shell
//! measures drags the same way). Icons spread to make room: those before
//! the pointer move back by what swelled before it, the rest on by what
//! swelled after, so the spot under the pointer stays put and the dock
//! grows out at both ends.
//!
//! Positions are in slots along the row (an icon and its gap is one), so
//! an icon sliding to a new slot as a neighbour closes is a fraction.

use super::motion::Magnify;

/// One icon of the row at rest: its slot (fractional while it slides) and
/// how far in it has grown (0 … 1, as it opens).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rest {
    pub slot: f32,
    pub presence: f32,
}

/// One icon as it's drawn: its left edge from the row's start, its side,
/// and how far it rises.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub side: f32,
    pub lift: f32,
}

/// The row as drawn: each icon, and how far past the row at rest it
/// reaches before and after.
#[derive(Clone, Debug, PartialEq)]
pub struct Swollen {
    pub icons: Vec<Placed>,
    pub before: f32,
    pub after: f32,
}

/// The row's icons, `side` big, `pitch` apart, swollen by `magnify` with
/// the pointer at `pointer` slots along it, as strongly as `strength`
/// (0 … 1, coming and going with the pointer).
pub fn swell(
    rests: &[Rest],
    side: f32,
    pitch: f32,
    pointer: Option<f32>,
    strength: f32,
    magnify: Option<&Magnify>,
) -> Swollen {
    let scales: Vec<(f32, f32)> = rests
        .iter()
        .map(|rest| match (pointer, magnify) {
            (Some(at), Some(magnify)) if strength > 0.0 => {
                let (scale, lift) = size_at(magnify, distance(magnify, at, rest.slot));
                (1.0 + (scale - 1.0) * strength, lift * strength)
            }
            _ => (1.0, 0.0),
        })
        .collect();

    // what each icon adds to the row's length
    let extra: Vec<f32> = rests
        .iter()
        .zip(&scales)
        .map(|(rest, (scale, _))| side * (scale - 1.0) * rest.presence)
        .collect();
    let total: f32 = extra.iter().sum();
    // what swelled before the pointer: the share of each icon behind it
    let before: f32 = match pointer {
        Some(at) => rests
            .iter()
            .zip(&extra)
            .map(|(rest, extra)| extra * (at - rest.slot).clamp(0.0, 1.0))
            .sum(),
        None => 0.0,
    };

    let mut ahead = 0.0;
    let icons = rests
        .iter()
        .zip(scales.iter().zip(&extra))
        .map(|(rest, ((scale, lift), extra))| {
            let placed = Placed {
                x: rest.slot * pitch + ahead - before,
                side: side * scale * rest.presence,
                lift: *lift,
            };
            ahead += extra;
            placed
        })
        .collect();

    Swollen {
        icons,
        before,
        after: total - before,
    }
}

/// How far, in slots, an icon at `slot` is from the pointer at `at`: from
/// the icon's centre when smooth, else in whole icons from the one under
/// the pointer.
fn distance(magnify: &Magnify, at: f32, slot: f32) -> f32 {
    if magnify.smooth {
        at - (slot + 0.5)
    } else {
        at.floor() - slot.round()
    }
}

/// The scale and lift `distance` slots from the pointer: the table's,
/// between its steps when smooth, nothing three or more away.
fn size_at(magnify: &Magnify, distance: f32) -> (f32, f32) {
    let d = distance.abs();
    let scale = |i: usize| magnify.scales.get(i).copied().unwrap_or(1.0);
    let lift = |i: usize| magnify.lifts.get(i).copied().unwrap_or(0.0);
    let step = d.floor() as usize;
    let part = d - step as f32;
    (
        scale(step) + (scale(step + 1) - scale(step)) * part,
        lift(step) + (lift(step + 1) - lift(step)) * part,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surfaces::dock::motion::DOCK;

    fn row(count: usize) -> Vec<Rest> {
        (0..count)
            .map(|slot| Rest {
                slot: slot as f32,
                presence: 1.0,
            })
            .collect()
    }

    fn stepped() -> Magnify {
        Magnify {
            smooth: false,
            ..DOCK.magnify.expect("magnifies as shipped")
        }
    }

    #[test]
    fn stepped_it_swells_as_the_reference_css_does() {
        let magnify = stepped();
        let swollen = swell(&row(5), 10.0, 12.0, Some(2.5), 1.0, Some(&magnify));
        let sides: Vec<f32> = swollen.icons.iter().map(|icon| icon.side).collect();
        assert_eq!(sides, [11.0, 12.0, 15.0, 12.0, 11.0]);
        let lifts: Vec<f32> = swollen.icons.iter().map(|icon| icon.lift).collect();
        assert_eq!(lifts, [0.0, 6.0, 10.0, 6.0, 0.0]);
    }

    #[test]
    fn smooth_it_follows_the_pointer_across_an_icon() {
        let magnify = DOCK.magnify.expect("magnifies as shipped");
        let centre = size_at(&magnify, 0.0);
        let half = size_at(&magnify, 0.5);
        assert_eq!(centre, (1.5, 10.0));
        assert!((half.0 - 1.35).abs() < 0.001 && (half.1 - 8.0).abs() < 0.001);
        assert_eq!(size_at(&magnify, 3.2), (1.0, 0.0));
    }

    #[test]
    fn the_spot_under_the_pointer_stays_put() {
        let magnify = DOCK.magnify.expect("magnifies as shipped");
        let (side, pitch) = (10.0, 12.0);
        for at in [0.3, 1.7, 2.5, 4.9] {
            let swollen = swell(&row(5), side, pitch, Some(at), 1.0, Some(&magnify));
            let i = at.floor() as usize;
            let icon = swollen.icons[i];
            // the share of the way across the icon the pointer is at is
            // the same at rest and swollen
            let share = at - i as f32;
            let drawn = icon.x + share * (pitch - side + icon.side);
            assert!((drawn - at * pitch).abs() < 0.5, "at {at}: {drawn}");
        }
    }

    #[test]
    fn the_row_grows_at_both_ends_by_all_that_swelled() {
        let magnify = DOCK.magnify.expect("magnifies as shipped");
        let swollen = swell(&row(5), 10.0, 12.0, Some(2.5), 1.0, Some(&magnify));
        let grown: f32 = swollen.icons.iter().map(|icon| icon.side - 10.0).sum();
        assert!((swollen.before + swollen.after - grown).abs() < 0.001);
        assert!((swollen.before - swollen.after).abs() < 0.001, "centred");
    }

    #[test]
    fn without_the_pointer_or_magnification_it_rests() {
        let magnify = DOCK.magnify.expect("magnifies as shipped");
        let rest = swell(&row(3), 10.0, 12.0, None, 1.0, Some(&magnify));
        let none = swell(&row(3), 10.0, 12.0, Some(1.5), 1.0, None);
        let gone = swell(&row(3), 10.0, 12.0, Some(1.5), 0.0, Some(&magnify));
        for swollen in [rest, none, gone] {
            let xs: Vec<f32> = swollen.icons.iter().map(|icon| icon.x).collect();
            assert_eq!(xs, [0.0, 12.0, 24.0]);
            assert_eq!(swollen.before + swollen.after, 0.0);
        }
    }
}
