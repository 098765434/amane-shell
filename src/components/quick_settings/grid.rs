//! The tile grid — GNOME's two-column QuickSettingsLayout, built from rows.
//!
//! amane has no grid widget (on purpose: its layout stays rows, columns and
//! stacks), so the grid is packed here: half-width tiles go two to a row,
//! and a full-width one (a slider, the system row) starts a row of its own,
//! leaving a half tile before it alone, as GNOME does. Every cell has a
//! fixed height, so the grid knows its own height before it's drawn — the
//! surface is sized to it.

use amane::{Column, Parent, Row, Widget};

use super::tile::QuickToggle;
use crate::components::layout::boxed;
use crate::tokens::sys;

pub enum Cell<T> {
    /// A whole row, `height` tall.
    Full(T, f32),
    /// Half a row, a tile's height.
    Half(T),
}

/// Packs `cells` into rows `gap` apart, half cells `half_height` tall;
/// gives the rows and their total height.
fn pack<T>(cells: Vec<Cell<T>>, half_height: f32, gap: f32) -> (Vec<(Vec<T>, f32)>, f32) {
    let mut rows: Vec<(Vec<T>, f32)> = Vec::new();
    let mut waiting: Option<T> = None;

    for cell in cells {
        match cell {
            Cell::Full(item, height) => {
                if let Some(half) = waiting.take() {
                    rows.push((vec![half], half_height));
                }
                rows.push((vec![item], height));
            }
            Cell::Half(item) => match waiting.take() {
                Some(first) => rows.push((vec![first, item], half_height)),
                None => waiting = Some(item),
            },
        }
    }
    if let Some(half) = waiting {
        rows.push((vec![half], half_height));
    }

    let heights: f32 = rows.iter().map(|(_, height)| height).sum();
    let gaps = gap * rows.len().saturating_sub(1) as f32;
    (rows, heights + gaps)
}

/// The grid of `cells`, and how tall it is.
pub fn grid(cells: Vec<Cell<Box<dyn Widget>>>) -> (Column, f32) {
    let gap = sys::space::gap::s();
    let (rows, height) = pack(cells, QuickToggle::height(), gap);

    let rows: Vec<Box<dyn Widget>> = rows
        .into_iter()
        .map(|(items, _)| boxed(Row::new(items).gap(gap)))
        .collect();

    (Column::new(rows).gap(gap).width(Parent), height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(cells: Vec<Cell<&'static str>>) -> (Vec<Vec<&'static str>>, f32) {
        let (rows, height) = pack(cells, 10.0, 1.0);
        (rows.into_iter().map(|(items, _)| items).collect(), height)
    }

    #[test]
    fn half_tiles_go_two_to_a_row() {
        let (rows, height) = shape(vec![
            Cell::Half("wifi"),
            Cell::Half("bluetooth"),
            Cell::Half("dark"),
        ]);
        assert_eq!(rows, [vec!["wifi", "bluetooth"], vec!["dark"]]);
        assert_eq!(height, 10.0 + 1.0 + 10.0);
    }

    #[test]
    fn a_full_cell_starts_its_own_row() {
        let (rows, height) = shape(vec![
            Cell::Full("system", 30.0),
            Cell::Half("wifi"),
            Cell::Full("apps", 20.0),
            Cell::Half("dark"),
        ]);
        assert_eq!(
            rows,
            [vec!["system"], vec!["wifi"], vec!["apps"], vec!["dark"]]
        );
        assert_eq!(height, 30.0 + 10.0 + 20.0 + 10.0 + 3.0);
    }

    #[test]
    fn nothing_is_no_height() {
        assert_eq!(shape(Vec::new()), (Vec::<Vec<&str>>::new(), 0.0));
    }
}
