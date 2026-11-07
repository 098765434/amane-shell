//! The keyboard in a menu, the same for both hosts: Up and Down move along
//! the rows that can be picked, round from the last to the first; Right
//! opens a submenu and Left goes back out of one; Enter picks; Escape closes.

use amane::Key;

/// What a key asks of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// To this row.
    To(usize),
    /// Into the focused row's submenu.
    Open,
    /// Out of this submenu.
    Back,
    /// The focused row.
    Pick,
    Close,
    /// Nothing; the key isn't the menu's.
    None,
}

/// What `key` does in a menu whose rows `selectable` says can be picked,
/// with `focus` on one of them (by index), or none yet (then `start`, as a
/// dropdown's chosen value, is where Up and Down go from).
pub fn press(key: Key, selectable: &[bool], focus: Option<usize>, start: Option<usize>) -> Move {
    match key {
        Key::Down => step(selectable, focus.or(start), 1).map_or(Move::None, Move::To),
        Key::Up => step(selectable, focus.or(start), -1).map_or(Move::None, Move::To),
        Key::Right => Move::Open,
        Key::Left => Move::Back,
        Key::Enter => Move::Pick,
        Key::Escape => Move::Close,
        _ => Move::None,
    }
}

/// The row `delta` along from `from` among those that can be picked, round
/// past either end; from none, the first (or the last, going up).
pub fn step(selectable: &[bool], from: Option<usize>, delta: isize) -> Option<usize> {
    let rows: Vec<usize> = selectable
        .iter()
        .enumerate()
        .filter_map(|(i, can)| can.then_some(i))
        .collect();
    let (&first, &last) = (rows.first()?, rows.last()?);
    let Some(at) = from.and_then(|from| rows.iter().position(|&i| i == from)) else {
        return Some(if delta >= 0 { first } else { last });
    };
    let n = rows.len() as isize;
    Some(rows[((at as isize + delta) % n + n) as usize % rows.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_and_down_skip_what_cant_be_picked_and_go_round() {
        let rows = [false, true, false, true, true];
        assert_eq!(step(&rows, None, 1), Some(1));
        assert_eq!(step(&rows, None, -1), Some(4));
        assert_eq!(step(&rows, Some(1), 1), Some(3));
        assert_eq!(step(&rows, Some(4), 1), Some(1));
        assert_eq!(step(&rows, Some(1), -1), Some(4));
        assert_eq!(step(&[false, false], None, 1), None);
    }

    #[test]
    fn a_dropdown_moves_on_from_its_value() {
        let rows = [true, true, true];
        assert_eq!(press(Key::Down, &rows, None, Some(1)), Move::To(2));
        assert_eq!(press(Key::Up, &rows, None, Some(0)), Move::To(2));
        assert_eq!(press(Key::Enter, &rows, Some(0), None), Move::Pick);
    }
}
