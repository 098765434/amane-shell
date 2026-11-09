//! The keyboard in Edit Widgets. Tab (Shift-Tab) takes each card in turn,
//! ringed; with one taken, the arrows move it a cell, Shift and an arrow
//! steps its size (right and down bigger, left and up smaller), Delete or
//! Backspace removes it and Enter opens its menu. Enter with none taken
//! adds the first widget the gallery shows; Ctrl+Z is Undo Remove; Escape
//! lets go of a drag, then of the card taken, then clears the search, then
//! is Done. Anything else types into the gallery's search.

use std::cell::Cell;

use amane::{Key, modifiers};

use super::*;

thread_local! {
    // the card the keyboard has, by its id
    static FOCUS: Cell<Option<u32>> = const { Cell::new(None) };
}

/// The card the keyboard has on `output`, if it's still there.
pub(super) fn focused(output: &str) -> Option<WidgetKey> {
    let id = FOCUS.get()?;
    let layout = current(output)?;
    layout.placed().map(|(key, _)| key).find(|key| key.id == id)
}

/// Lets go of the card the keyboard has.
pub(super) fn forget() {
    FOCUS.set(None);
}

/// `key` while `output` is edited.
pub(super) fn key(output: &str, key: Key) {
    let held = modifiers();
    let focus = focused(output);
    match key {
        Key::Escape => escape(output, focus.is_some()),
        Key::Character('z' | 'Z') if held.ctrl => edit::undo(),
        Key::Tab => step_focus(output, if held.shift { -1 } else { 1 }),
        Key::Enter => match focus {
            Some(card) => menu_for(output, card),
            None => {
                if let Some(kind) = gallery::first() {
                    edit::add_from_gallery(output, kind);
                }
            }
        },
        Key::Delete | Key::Backspace if focus.is_some() => {
            if let Some(card) = focus {
                step_focus(output, 1);
                if focused(output) == Some(card) {
                    forget();
                }
                edit::remove(output, card);
            }
        }
        Key::Left | Key::Right | Key::Up | Key::Down if focus.is_some() => {
            if let Some(card) = focus {
                let (cols, rows) = match key {
                    Key::Left => (-1, 0),
                    Key::Right => (1, 0),
                    Key::Up => (0, -1),
                    _ => (0, 1),
                };
                if held.shift {
                    step_size(output, card, cols + rows);
                } else {
                    nudge(output, card, cols, rows);
                }
            }
        }
        _ => {
            // typing goes to the search, and lets go of the card
            forget();
            gallery::key(key);
        }
    }
    changed();
}

/// Escape: lets go of a drag (the button still down takes nothing more
/// until it comes up), else of the card the keyboard has, else clears the
/// search, else is Done.
fn escape(output: &str, focused: bool) {
    let dropped = with_desk(output, |desk| {
        let dropped = desk.session.take().is_some();
        desk.cancelled |= dropped;
        desk.dragged |= dropped;
        dropped
    });
    if dropped {
        return;
    }
    if focused {
        forget();
    } else if !gallery::clear_query() {
        edit::leave_all();
    }
}

/// The cards in reading order: row by row, left to right.
fn in_order(layout: &Layout<WidgetKey>) -> Vec<WidgetKey> {
    let mut placed: Vec<(WidgetKey, Rect)> = layout.placed().collect();
    placed.sort_by_key(|(_, rect)| (rect.at.row, rect.at.col));
    placed.into_iter().map(|(key, _)| key).collect()
}

/// Takes the next card (`by` 1) or the one before (-1), round the ends.
fn step_focus(output: &str, by: i32) {
    let Some(layout) = current(output) else {
        return;
    };
    let order = in_order(&layout);
    if order.is_empty() {
        forget();
        return;
    }
    let now = focused(output).and_then(|key| order.iter().position(|k| *k == key));
    let len = order.len() as i32;
    let next = match now {
        Some(i) => (i as i32 + by).rem_euclid(len),
        None if by < 0 => len - 1,
        None => 0,
    };
    FOCUS.set(Some(order[next as usize].id));
}

/// Moves `card` a cell along, the others making way as for a drag.
fn nudge(output: &str, card: WidgetKey, cols: i32, rows: i32) {
    let Some(layout) = current(output) else {
        return;
    };
    let Some(rect) = layout.rect_of(card) else {
        return;
    };
    let col = i32::from(rect.at.col) + cols;
    let row = i32::from(rect.at.row) + rows;
    if col < 0 || row < 0 {
        return;
    }
    let at = layout.canvas.clamp(col, row, rect.span);
    if at == rect.at {
        return;
    }
    if let Some(next) = layout.moved(card, at) {
        commit(output, &next);
    }
}

/// Makes `card` the next size it comes in (`by` 1) or the one before (-1).
fn step_size(output: &str, card: WidgetKey, by: i32) {
    let Some(layout) = current(output) else {
        return;
    };
    let Some(span) = layout.span_of(card) else {
        return;
    };
    let sizes = card.kind.supported();
    let Some(now) = sizes.iter().position(|s| *s == span) else {
        return;
    };
    let next = now as i32 + by.signum();
    if let Some(span) = usize::try_from(next).ok().and_then(|i| sizes.get(i)) {
        resize(output, card, *span);
    }
}

/// `card`'s menu, at its centre.
fn menu_for(output: &str, card: WidgetKey) {
    let at = with_desk(output, |desk| {
        let geometry = desk.geometry?;
        let slot = desk.slots.iter().find(|(k, _)| *k == card)?.1;
        Some((
            (geometry.origin.0 + slot.x + slot.width / 2.0) as i32,
            (geometry.origin.1 + slot.y + slot.height / 2.0) as i32,
        ))
    });
    if let Some(at) = at {
        widget_menu(output, card, at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emaki_kit::grid::Entry;

    #[test]
    fn the_keyboard_takes_the_cards_row_by_row() {
        let mut layout = Layout::new(Canvas { cols: 8, rows: 4 });
        let key = |id| WidgetKey {
            id,
            kind: WidgetKind::Cpu,
        };
        for (id, col, row) in [(1, 4, 2), (2, 2, 0), (3, 0, 2)] {
            layout.insert(Entry {
                id: key(id),
                span: WidgetSize::Small.span(),
                at: Some(Place::new(col, row)),
            });
        }
        assert_eq!(in_order(&layout), [key(2), key(3), key(1)]);
    }
}
