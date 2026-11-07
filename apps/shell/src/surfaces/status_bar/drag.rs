//! Moving the bar's items by dragging them, without a held modifier key.
//! The bar never has the keyboard, so it can't see a held
//! key: a press that moves more than [`SLOP`] picks the item up instead,
//! and a click stays a click. The others make room as it passes; letting
//! go keeps the new order (`StatusBar::order`), and letting go well off
//! the bar hides an item that can be hidden.
//!
//! The drag is measured on one still layer over the whole cluster, never
//! on the item that moves (amane measures a drag against the last frame).

use crate::chrome::per_output::PerOutput;
use std::cell::RefCell;

use amane::{Center, Color, Parent, Point, Rectangle, Row, Size, Stack, Widget};

use crate::tokens::sys;
use amane_kit::buttons::swallow_click;
use amane_kit::layout::{boxed, fixed_width};

use super::indicators::{Item, move_item};

/// How far a press moves before it picks the item up.
const SLOP: f32 = 6.0;

/// How far past the bar's edge an item is let go to hide it.
const OFF: f32 = 40.0;

/// How see-through an item about to be hidden is.
const LEAVING: f32 = 0.35;

/// The item picked up.
#[derive(Clone, Copy, Debug)]
struct Held {
    item: Item,
    /// Where the pointer went down.
    from: Point,
    /// How far into the item it went down.
    grab: f32,
    /// Where the pointer is.
    at: Point,
    /// Whether it moved far enough to be a drag rather than a click.
    moving: bool,
}

/// Where each item sits on a bar, left to right, as last drawn.
#[derive(Clone, Default)]
struct Laid {
    spans: Vec<(Item, f32, f32)>,
    height: f32,
    /// Where the cluster starts on the bar: it ends at the bar's right
    /// inset.
    start: f32,
}

thread_local! {
    static HELD: RefCell<Option<(String, Held)>> = const { RefCell::new(None) };
    static LAID: PerOutput<Laid> = const { PerOutput::new() };
}

/// The items `built` for the bar on `output`, in a row whose items can be
/// dragged into another order. `height` is the bar's.
pub fn cluster(output: &str, built: Vec<(Item, Vec<Box<dyn Widget>>)>, height: f32) -> Stack {
    let gap = sys::space::gap::_2xs();
    let held = HELD.with_borrow(|held| {
        held.as_ref()
            .filter(|(on, held)| on == output && held.moving)
            .map(|(_, held)| *held)
    });

    let mut groups: Vec<(Item, Row, f32)> = built
        .into_iter()
        .map(|(item, widgets)| {
            let row = Row::new(widgets).gap(gap).align(Center);
            let width = fixed_width(&row);
            (item, row, width)
        })
        .collect();

    // the held item goes where its middle is now, the rest close up
    if let Some(held) = held
        && let Some(from) = groups.iter().position(|(item, ..)| *item == held.item)
    {
        let group = groups.remove(from);
        let middle = held.at.x - held.grab + group.2 / 2.0;
        let widths: Vec<f32> = groups.iter().map(|(.., width)| *width).collect();
        groups.insert(slot(&widths, gap, middle), group);
    }

    let mut spans = Vec::with_capacity(groups.len());
    let mut children: Vec<Box<dyn Widget>> = Vec::with_capacity(groups.len());
    let mut left = 0.0;
    for (item, row, width) in groups {
        spans.push((item, left, width));
        let mut cell = Rectangle::new()
            .width(Size::Fixed(width))
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center);
        if let Some(held) = held.filter(|held| held.item == item) {
            cell = cell.translate(held.at.x - held.grab - left, 0.0);
            if off(held.at.y, height) && item.hideable() {
                cell = cell.opacity(LEAVING);
            }
        }
        children.push(boxed(cell.child(row)));
        left += width + gap;
    }
    let total = (left - gap).max(0.0);
    LAID.with(|laid| {
        laid.all(|laid| {
            laid.insert(
                output.to_string(),
                Laid {
                    spans,
                    height,
                    start: amane::window_size().0 - sys::space::inset::m() - total,
                },
            );
        })
    });

    let row = Row::new(children).gap(gap).align(Center).height(Parent);
    let (dragged, released) = (output.to_string(), output.to_string());
    let layer = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .fill(Color::TRANSPARENT)
        .on_drag(move |at| drag(&dragged, at))
        .on_release(move |at| release(&released, at));

    Stack::new(vec![boxed(row), boxed(layer)])
        .width(Size::Fixed(total))
        .height(Parent)
}

/// A monitor whose bar has `item`, for when no pointer says which (IPC).
pub fn output_with(item: Item) -> Option<String> {
    LAID.with(|laid| {
        laid.all(|laid| {
            laid.iter()
                .find(|(_, laid)| laid.spans.iter().any(|(laid, ..)| *laid == item))
                .map(|(output, _)| output.clone())
        })
    })
}

/// Where the middle of `item` is on the bar on `output`, as last drawn,
/// from the bar's left: where a program opens a window under its item.
pub fn middle_of(output: &str, item: Item) -> Option<f32> {
    LAID.with(|laid| {
        laid.all(|laid| {
            let laid = laid.get(output)?;
            let (_, left, width) = laid.spans.iter().find(|(laid, ..)| *laid == item)?;
            Some(laid.start + left + width / 2.0)
        })
    })
}

/// The index among items `widths` wide whose place a middle at `middle`
/// takes: after every item whose middle is left of it.
fn slot(widths: &[f32], gap: f32, middle: f32) -> usize {
    let mut left = 0.0;
    let mut index = 0;
    for width in widths {
        if left + width / 2.0 < middle {
            index += 1;
        }
        left += width + gap;
    }
    index
}

/// Whether a pointer at `y` is far enough off a bar `height` tall to hide
/// what it holds.
fn off(y: f32, height: f32) -> bool {
    y < -OFF || y > height + OFF
}

fn drag(output: &str, at: Point) {
    HELD.with_borrow_mut(|held| match held {
        Some((on, held)) if on == output => {
            held.at = at;
            if !held.moving
                && ((at.x - held.from.x).abs() > SLOP || (at.y - held.from.y).abs() > SLOP)
            {
                held.moving = true;
            }
        }
        _ => {
            // the press: the item under it, if any
            let under = LAID.with(|laid| {
                laid.all(|laid| {
                    laid.get(output).and_then(|laid| {
                        laid.spans
                            .iter()
                            .find(|(_, left, width)| at.x >= *left && at.x < left + width)
                            .map(|(item, left, _)| (*item, at.x - left))
                    })
                })
            });
            *held = under.map(|(item, grab)| {
                (
                    output.to_string(),
                    Held {
                        item,
                        from: at,
                        grab,
                        at,
                        moving: false,
                    },
                )
            });
        }
    });
}

fn release(output: &str, at: Point) {
    let Some((on, mut held)) = HELD.take() else {
        return;
    };
    if on != output || !held.moving {
        return;
    }
    held.at = at;
    swallow_click();

    let laid = LAID.with(|laid| laid.get(output)).unwrap_or_default();
    if off(at.y, laid.height) && held.item.hideable() {
        held.item.hide();
        return;
    }

    // the order the bar showed as it was let go, put among every item
    let shown: Vec<Item> = laid.spans.iter().map(|(item, ..)| *item).collect();
    move_item(held.item, place(&Item::ordered(), &shown, held.item));
}

/// Where `item` goes among all of `order`, given the order `shown` on the
/// bar: just after the shown item before it, or else just before the one
/// after it.
fn place(order: &[Item], shown: &[Item], item: Item) -> usize {
    let rest: Vec<Item> = order
        .iter()
        .copied()
        .filter(|other| *other != item)
        .collect();
    let Some(at) = shown.iter().position(|other| *other == item) else {
        return rest.len();
    };
    if let Some(before) = at.checked_sub(1).map(|before| shown[before])
        && let Some(index) = rest.iter().position(|other| *other == before)
    {
        return index + 1;
    }
    shown
        .get(at + 1)
        .and_then(|after| rest.iter().position(|other| other == after))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_takes_the_place_its_middle_is_over() {
        let widths = [20.0, 20.0, 20.0];
        assert_eq!(slot(&widths, 4.0, 0.0), 0);
        assert_eq!(slot(&widths, 4.0, 11.0), 1);
        assert_eq!(slot(&widths, 4.0, 40.0), 2);
        assert_eq!(slot(&widths, 4.0, 100.0), 3);
    }

    #[test]
    fn a_dropped_item_follows_its_new_neighbour_among_all() {
        let order = Item::ALL;
        // the clock dropped right after the volume on a bar that shows only some
        let shown = [Item::Volume, Item::Clock, Item::Battery];
        let index = place(&order, &shown, Item::Clock);
        let mut rest: Vec<Item> = order.into_iter().filter(|i| *i != Item::Clock).collect();
        rest.insert(index, Item::Clock);
        let volume = rest.iter().position(|i| *i == Item::Volume).unwrap();
        assert_eq!(rest[volume + 1], Item::Clock);

        // dropped first: before the one now after it
        let shown = [Item::Clock, Item::Volume];
        let index = place(&order, &shown, Item::Clock);
        let mut rest: Vec<Item> = order.into_iter().filter(|i| *i != Item::Clock).collect();
        rest.insert(index, Item::Clock);
        let volume = rest.iter().position(|i| *i == Item::Volume).unwrap();
        assert_eq!(rest[volume - 1], Item::Clock);
    }

    #[test]
    fn far_enough_off_the_bar_hides() {
        assert!(!off(10.0, 30.0));
        assert!(!off(-30.0, 30.0));
        assert!(off(-50.0, 30.0));
        assert!(off(80.0, 30.0));
    }
}
