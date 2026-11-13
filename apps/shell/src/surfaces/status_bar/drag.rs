//! Moving the bar's items by dragging them, without a held modifier key.
//! The bar never has the keyboard, so it can't see a held
//! key: a press that moves more than [`SLOP`] picks the item up instead,
//! and a click stays a click. The others make room as it passes; letting
//! go keeps the new order (`StatusBar::order`), and letting go well off
//! the bar hides an item that can be hidden.
//!
//! The drag is measured on one still layer over the whole cluster, never
//! on the item that moves (amane measures a drag against the last frame).

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use crate::chrome::per_output::PerOutput;

use amane::{
    Animation, Blend, Center, End, Parent, Point, Radius, Rectangle, Row, Stack, Start, Widget,
};

use crate::tokens::sys;
use emaki_kit::buttons::swallow_click;
use emaki_kit::icons::ICON_CLOSE;
use emaki_kit::layout::{boxed, fixed_width, icon};
use emaki_kit::motion::{self, duration};

use super::indicators::{Item, move_item};
use super::width_of;

/// How far a press moves before it picks the item up.
const SLOP: f32 = 6.0;

/// How far past the bar's edge an item is let go to hide it.
const OFF: f32 = 40.0;

/// How see-through an item about to be hidden is.
const LEAVING: f32 = 0.35;

/// How long the bar offers to bring back an item dragged off it.
const UNDO_FOR: Duration = Duration::from_secs(5);

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

/// What the cluster lays out: the bar's items, and the undo button before
/// them while there's something to bring back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Undo,
    Item(Item),
}

/// One cell's motion, kept across frames under its key.
struct Track {
    key: Key,
    /// How far its right edge is from the cluster's right end. The
    /// cluster ends at the bar's right inset, so this is what stays put
    /// as cells to its left come, go or change width.
    right: Animation<f32>,
    /// How far in it has come: 0 as it appears, 1 once it's there.
    shown: Animation<f32>,
    /// Its opacity: [`LEAVING`] while it's held off the bar to be hidden.
    dim: Animation<f32>,
}

impl Track {
    fn new(key: Key, right: f32, arrived: bool) -> Self {
        let mut shown = motion::fade(if arrived { 1.0 } else { 0.0 }, duration::fade());
        shown.to(1.0);
        Self {
            key,
            right: motion::spring(right),
            shown,
            dim: motion::fade(1.0, duration::hover()),
        }
    }
}

thread_local! {
    static HELD: RefCell<Option<(String, Held)>> = const { RefCell::new(None) };
    static LAID: PerOutput<Laid> = const { PerOutput::new() };

    // each bar's cells, sliding to their places and fading in
    static TRACKS: PerOutput<Vec<Track>> = const { PerOutput::new() };

    // the item just dragged off the bar, and when, while it can be brought back
    static HIDDEN: Cell<Option<(Item, Instant)>> = const { Cell::new(None) };
}

/// Steps `tracks` towards `rests`, each key and where its right edge rests,
/// keeping only those keys. On a bar seen for the first time (`fresh`)
/// everything is simply there; a key that turns up later comes in.
fn step(tracks: &mut Vec<Track>, rests: &[(Key, f32)], fresh: bool) {
    tracks.retain(|track| rests.iter().any(|(key, _)| *key == track.key));
    for (key, right) in rests {
        match tracks.iter_mut().find(|track| track.key == *key) {
            Some(track) => track.right.to(*right),
            None => tracks.push(Track::new(*key, *right, fresh)),
        }
    }
}

/// Where each of cells `widths` wide rests, as its right edge's distance
/// from the end of a row `gap` apart.
fn rests(widths: &[f32], gap: f32) -> Vec<f32> {
    let mut right = 0.0;
    let mut rests = vec![0.0; widths.len()];
    for (rest, width) in rests.iter_mut().zip(widths).rev() {
        *rest = right;
        right += width + gap;
    }
    rests
}

/// The items `built` for the bar on `output`, in a row whose items can be
/// dragged into another order, after `undo` if there is one. `height` is
/// the bar's. Every cell slides to its place as others come, go or change
/// width, and one that appears fades in.
pub fn cluster(
    output: &str,
    built: Vec<(Item, Vec<Box<dyn Widget>>)>,
    undo: Option<Box<dyn Widget>>,
    height: f32,
) -> Stack {
    let gap = super::metrics().gap;
    let held = HELD.with_borrow(|held| {
        held.as_ref()
            .filter(|(on, held)| on == output && held.moving)
            .map(|(_, held)| *held)
    });

    let mut groups: Vec<(Key, Box<dyn Widget>, f32)> = built
        .into_iter()
        .map(|(item, widgets)| {
            let row = Row::new(widgets).gap(gap).align(Center);
            let width = fixed_width(&row);
            (Key::Item(item), boxed(row), width)
        })
        .collect();

    // the held item goes where its middle is now, the rest close up
    if let Some(held) = held
        && let Some(from) = groups
            .iter()
            .position(|(key, ..)| *key == Key::Item(held.item))
    {
        let group = groups.remove(from);
        let middle = held.at.x - held.grab + group.2 / 2.0;
        let widths: Vec<f32> = groups.iter().map(|(.., width)| *width).collect();
        groups.insert(slot(&widths, gap, middle), group);
    }

    let widths: Vec<f32> = groups.iter().map(|(.., width)| *width).collect();
    let items = (widths.iter().map(|width| width + gap).sum::<f32>() - gap).max(0.0);
    let mut spans = Vec::with_capacity(groups.len());
    let mut left = 0.0;
    for ((key, ..), width) in groups.iter().zip(&widths) {
        if let Key::Item(item) = key {
            spans.push((*item, left, *width));
        }
        left += width + gap;
    }

    // the undo button before the items, outside what can be dragged
    let lead = undo
        .as_ref()
        .map_or(0.0, |undo| width_of(undo.as_ref()) + gap);
    if let Some(undo) = undo {
        let width = lead - gap;
        groups.insert(0, (Key::Undo, undo, width));
    }
    let total = lead + items;
    let widths: Vec<f32> = groups.iter().map(|(.., width)| *width).collect();
    let rest = rests(&widths, gap);
    let keys: Vec<(Key, f32)> = groups.iter().map(|(key, ..)| *key).zip(rest).collect();

    let cells = TRACKS.with(|tracks| {
        let fresh = tracks.all(|tracks| !tracks.contains_key(output));
        tracks.with(output, Vec::new, |tracks| {
            step(tracks, &keys, fresh);
            groups
                .into_iter()
                .zip(widths)
                .map(|((key, content, width), _)| {
                    let track = tracks
                        .iter_mut()
                        .find(|track| track.key == key)
                        .expect("stepped above");
                    let leaving = match (key, held) {
                        (Key::Item(item), Some(held)) if held.item == item => {
                            // it follows the pointer, and springs from there when let go
                            let at = lead + held.at.x - held.grab;
                            track.right = motion::spring(total - at - width);
                            off(held.at.y, height) && item.hideable()
                        }
                        _ => false,
                    };
                    track.dim.to(if leaving { LEAVING } else { 1.0 });
                    cell(
                        content,
                        width,
                        total - track.right.value() - width,
                        track.shown.value(),
                        track.dim.value(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    LAID.with(|laid| {
        laid.all(|laid| {
            laid.insert(
                output.to_string(),
                Laid {
                    spans,
                    height,
                    start: amane::window_size().0 - sys::space::inset::m() - items,
                },
            );
        })
    });

    let (dragged, released) = (output.to_string(), output.to_string());
    let layer = Rectangle::new()
        .width(items)
        .height(Parent)
        .translate(lead, 0.0)
        .on_drag(move |at| drag(&dragged, at))
        .on_release(move |at| release(&released, at));

    let mut children = cells;
    children.push(boxed(layer));
    Stack::new(children).width(total).height(Parent)
}

/// A cell `width` wide at `left`, `shown` of the way in, with `dim` its
/// opacity: below 1 it's on its way off the bar, and wears the badge.
fn cell(content: Box<dyn Widget>, width: f32, left: f32, shown: f32, dim: f32) -> Box<dyn Widget> {
    let content = Rectangle::new()
        .width(Parent)
        .height(Parent)
        .align_child(Center, Center)
        .child(Row::new(vec![content]).align(Center));
    let content: Box<dyn Widget> = if dim < 1.0 {
        // the badge fades in as the item dims
        let badge = removing().opacity(((1.0 - dim) / (1.0 - LEAVING)).clamp(0.0, 1.0));
        boxed(
            Stack::new(vec![boxed(content.opacity(dim)), boxed(badge)])
                .width(width)
                .height(Parent),
        )
    } else {
        boxed(content)
    };
    let scale = f32::blend(motion::scale(motion::enter_scale()), 1.0, shown);
    boxed(
        Rectangle::new()
            .width(width)
            .height(Parent)
            .translate(left, 0.0)
            .opacity(shown)
            .scale(scale)
            .child(Row::new(vec![content]).height(Parent)),
    )
}

/// The badge on an item let go to be hidden: a small red disc with an X
/// at its top right.
fn removing() -> Rectangle {
    let size = (super::metrics().icon * 0.6).round();
    Rectangle::new()
        .width(Parent)
        .height(Parent)
        .align_child(End, Start)
        .child(
            Rectangle::new()
                .width(size)
                .height(size)
                .radius(Radius::Full)
                .fill(sys::color::status::critical::bg())
                .align_child(Center, Center)
                .child(icon(ICON_CLOSE, size * 0.8, sys::color::accent::fg())),
        )
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
        HIDDEN.set(Some((held.item, Instant::now())));
        // the undo button goes again by itself
        amane::redraw_after(UNDO_FOR);
        return;
    }

    // the order the bar showed as it was let go, put among every item
    let shown: Vec<Item> = laid.spans.iter().map(|(item, ..)| *item).collect();
    move_item(held.item, place(&Item::ordered(), &shown, held.item));
}

/// The item just dragged off the bar, while it can still be brought back.
pub fn just_hidden() -> Option<Item> {
    just_hidden_at(HIDDEN.get(), Instant::now())
}

fn just_hidden_at(hidden: Option<(Item, Instant)>, now: Instant) -> Option<Item> {
    hidden
        .filter(|(_, when)| now.duration_since(*when) < UNDO_FOR)
        .map(|(item, _)| item)
}

/// Brings back the item just dragged off the bar.
pub fn undo_hide() {
    if let Some((item, _)) = HIDDEN.take() {
        item.show();
    }
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
    fn a_hidden_item_can_be_brought_back_for_a_while() {
        let then = Instant::now();
        let hidden = Some((Item::Volume, then));
        assert_eq!(just_hidden_at(hidden, then), Some(Item::Volume));
        assert_eq!(just_hidden_at(hidden, then + UNDO_FOR), None);
        assert_eq!(just_hidden_at(None, then), None);
    }

    #[test]
    fn cells_rest_from_the_right_end() {
        assert_eq!(rests(&[10.0, 20.0, 5.0], 4.0), vec![33.0, 9.0, 0.0]);
        assert!(rests(&[], 4.0).is_empty());
    }

    #[test]
    fn a_new_bar_rests_and_a_later_item_comes_in() {
        let (volume, clock) = (Key::Item(Item::Volume), Key::Item(Item::Clock));
        let mut tracks = Vec::new();
        step(&mut tracks, &[(volume, 30.0), (clock, 0.0)], true);
        assert!(tracks.iter().all(|track| track.shown.value() == 1.0));

        let battery = Key::Item(Item::Battery);
        step(
            &mut tracks,
            &[(volume, 60.0), (battery, 30.0), (clock, 0.0)],
            false,
        );
        let shown = |key| {
            tracks
                .iter()
                .find(|t: &&Track| t.key == key)
                .unwrap()
                .shown
                .value()
        };
        assert!(shown(battery) < 0.1, "it fades in from nothing");
        assert_eq!(shown(volume), 1.0);
    }

    #[test]
    fn an_item_that_goes_is_forgotten() {
        let (volume, clock) = (Key::Item(Item::Volume), Key::Item(Item::Clock));
        let mut tracks = Vec::new();
        step(&mut tracks, &[(volume, 30.0), (clock, 0.0)], true);
        step(&mut tracks, &[(clock, 0.0)], false);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].key, clock);
    }

    #[test]
    fn far_enough_off_the_bar_hides() {
        assert!(!off(10.0, 30.0));
        assert!(!off(-30.0, 30.0));
        assert!(off(-50.0, 30.0));
        assert!(off(80.0, 30.0));
    }
}
