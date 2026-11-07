//! Moving through a list with the keyboard. amane has no widget focus, so
//! as the shell's context menu does, each list keeps which item is focused
//! by its id: Up and Down move (wrapping, as Material Web's lists do), Home
//! and End jump, Enter and Space press, Escape and Tab let go. The focus
//! ring only shows once the keyboard has moved it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amane::{Key, request_frame};

/// Whether an item can take focus, and what pressing it does.
pub(super) type NavItem = (bool, Option<Rc<dyn Fn()>>);

#[derive(Default)]
struct Nav {
    focus: Option<usize>,
    // whether the keyboard put it there, so it shows
    shown: bool,
    // which items can take focus, and what pressing each does
    items: Vec<NavItem>,
}

thread_local! {
    static LISTS: RefCell<HashMap<String, Nav>> = RefCell::new(HashMap::new());
}

/// List `id` was built with these items: whether each can take focus and
/// what a press does. Keeps the focus where it was, within the list.
pub(super) fn register(id: &str, items: Vec<NavItem>) {
    LISTS.with_borrow_mut(|lists| {
        let nav = lists.entry(id.to_string()).or_default();
        if nav.focus.is_some_and(|f| f >= items.len()) {
            nav.focus = None;
        }
        nav.items = items;
    });
}

/// The item the keyboard has focused in list `id`, if it shows.
pub fn focused(id: &str) -> Option<usize> {
    LISTS.with_borrow(|lists| {
        lists
            .get(id)
            .filter(|nav| nav.shown)
            .and_then(|nav| nav.focus)
    })
}

/// Puts list `id`'s focus on `index`: shown when the keyboard asked, kept
/// quietly when a click did (the next arrow key goes on from there).
pub fn set_focus(id: &str, index: Option<usize>, shown: bool) {
    LISTS.with_borrow_mut(|lists| {
        let nav = lists.entry(id.to_string()).or_default();
        nav.focus = index;
        nav.shown = shown && index.is_some();
    });
    request_frame();
}

/// The next item from `from` that can take focus, `delta` steps on (±1),
/// wrapping round; the first (or last) one when nothing is focused.
pub fn step(focusable: &[bool], from: Option<usize>, delta: i32) -> Option<usize> {
    let n = focusable.len();
    if n == 0 || !focusable.contains(&true) {
        return None;
    }
    let mut at = match from {
        Some(i) => i as i64,
        None if delta >= 0 => -1,
        None => n as i64,
    };
    for _ in 0..n {
        at = (at + i64::from(delta)).rem_euclid(n as i64);
        if focusable[at as usize] {
            return Some(at as usize);
        }
    }
    None
}

/// The first (`last` false) or last item that can take focus.
pub fn edge(focusable: &[bool], last: bool) -> Option<usize> {
    if last {
        focusable.iter().rposition(|&f| f)
    } else {
        focusable.iter().position(|&f| f)
    }
}

/// `key` went to list `id`. Answers whether the list used it.
pub fn handle_key(id: &str, key: Key) -> bool {
    let (focusable, focus, shown) = LISTS.with_borrow(|lists| match lists.get(id) {
        Some(nav) => (
            nav.items.iter().map(|(f, _)| *f).collect::<Vec<_>>(),
            nav.focus,
            nav.shown,
        ),
        None => (Vec::new(), None, false),
    });
    let next = match key {
        // the first arrow shows the focus where it is, if it's there
        Key::Down if !shown && focus.is_some() => focus,
        Key::Up if !shown && focus.is_some() => focus,
        Key::Down => step(&focusable, focus, 1),
        Key::Up => step(&focusable, focus, -1),
        Key::Home => edge(&focusable, false),
        Key::End => edge(&focusable, true),
        Key::Enter | Key::Space => {
            let press = LISTS.with_borrow(|lists| {
                let nav = lists.get(id)?;
                nav.items.get(nav.focus?)?.1.clone()
            });
            return match press {
                Some(press) if shown => {
                    press();
                    request_frame();
                    true
                }
                _ => false,
            };
        }
        Key::Escape | Key::Tab => {
            if shown {
                set_focus(id, focus, false);
            }
            return false;
        }
        _ => return false,
    };
    if next.is_none() {
        return false;
    }
    set_focus(id, next, true);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_skip_items_that_cant_take_focus() {
        let items = [true, false, true, true];
        assert_eq!(step(&items, Some(0), 1), Some(2));
        assert_eq!(step(&items, Some(2), -1), Some(0));
        // wraps round
        assert_eq!(step(&items, Some(3), 1), Some(0));
        assert_eq!(step(&items, Some(0), -1), Some(3));
    }

    #[test]
    fn the_first_step_lands_on_an_end() {
        let items = [false, true, true, false];
        assert_eq!(step(&items, None, 1), Some(1));
        assert_eq!(step(&items, None, -1), Some(2));
        assert_eq!(edge(&items, false), Some(1));
        assert_eq!(edge(&items, true), Some(2));
    }

    #[test]
    fn nothing_to_focus() {
        assert_eq!(step(&[false, false], None, 1), None);
        assert_eq!(step(&[], Some(0), 1), None);
    }

    #[test]
    fn keys_move_and_press() {
        use std::cell::Cell;
        let pressed = Rc::new(Cell::new(None));
        let items = (0..3)
            .map(|i| {
                let pressed = pressed.clone();
                let press: Rc<dyn Fn()> = Rc::new(move || pressed.set(Some(i)));
                (i != 1, Some(press))
            })
            .collect();
        register("test.nav", items);
        assert!(handle_key("test.nav", Key::Down));
        assert_eq!(focused("test.nav"), Some(0));
        assert!(handle_key("test.nav", Key::Down));
        assert_eq!(focused("test.nav"), Some(2));
        assert!(handle_key("test.nav", Key::Enter));
        assert_eq!(pressed.get(), Some(2));
        assert!(!handle_key("test.nav", Key::Escape));
        assert_eq!(focused("test.nav"), None);
    }
}
