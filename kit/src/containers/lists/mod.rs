//! Lists — Material 3's list component, complete, in three styles.
//!
//! A [`ListItem`] has every element of M3's anatomy: a leading icon,
//! avatar, image, video thumbnail, checkbox, radio or switch; an overline,
//! the headline and one or two lines of supporting text; and trailing
//! icons, supporting text, controls, a chevron, an expand chevron or a
//! drag handle. Its line count (one, two or three) comes from its text.
//! It has M3's item types (text, button, link) and states: enabled,
//! hovered, focused, pressed, dragged, disabled, and selected alongside
//! them, with a state layer that fades between them.
//!
//! A [`List`] stacks items with an optional subheader and dividers (full,
//! inset to the text, or middle-inset), and can bind a single or multiple
//! [`Selection`] to the items' controls, move a keyboard focus through
//! them ([`handle_key`]), reorder them by dragging their handles, and
//! build only the rows in view of a long list ([`List::window`]).
//!
//! Every number and colour comes from the [`ListStyle`]'s [`ListTokens`]:
//! `Shell` (the shell's look, the default), `M3` (Material 3's baseline)
//! or `M3Expressive` (2025's segmented lists, whose items change shape
//! with their state). Today's simpler [`crate::list`] stays as it is.

mod item;
mod nav;
pub mod scheme;
mod state;
mod style;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amane::{
    Center, Color, Cursor, Point, Rectangle, Size, Stack, Start, Text, Widget, request_frame,
};

pub use item::{ItemKind, Leading, ListItem, Trailing, line_type};
pub use nav::{focused, handle_key, set_focus};
pub use state::{Segment, State, corners};
pub use style::{Line, ListStyle, ListTokens, Shapes};

use crate::divider::Divider;
use crate::layout::boxed;
use crate::tokens::{Styled, sys};
use item::Context;

/// Which dividers go between items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dividers {
    #[default]
    None,
    /// Edge to edge.
    Full,
    /// From where the text starts to the end.
    Inset,
    /// In from both edges by the item's padding.
    Middle,
}

/// What's selected, which the items' controls and fills show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    /// At most one item, as radio buttons choose.
    Single(Option<usize>),
    /// Any number, as checkboxes or switches choose.
    Multi(Vec<usize>),
}

impl Selection {
    pub fn contains(&self, index: usize) -> bool {
        match self {
            Selection::None => false,
            Selection::Single(chosen) => *chosen == Some(index),
            Selection::Multi(chosen) => chosen.contains(&index),
        }
    }

    /// Item `index` was set `on`: what's selected then.
    pub fn toggled(&self, index: usize, on: bool) -> Selection {
        match self {
            Selection::None => Selection::None,
            Selection::Single(_) => Selection::Single(Some(index)),
            Selection::Multi(chosen) => {
                let mut chosen: Vec<usize> =
                    chosen.iter().copied().filter(|&i| i != index).collect();
                if on {
                    chosen.push(index);
                    chosen.sort_unstable();
                }
                Selection::Multi(chosen)
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    from: usize,
    start: f32,
    at: f32,
}

thread_local! {
    // each list's drag in flight, by the list's id
    static DRAGS: RefCell<HashMap<String, Drag>> = RefCell::new(HashMap::new());
}

/// Where item `from` lands when its middle is dragged `moved` down a list
/// whose items start at `tops` and are `heights` tall.
pub fn drop_index(tops: &[f32], heights: &[f32], from: usize, moved: f32) -> usize {
    let Some(&top) = tops.get(from) else {
        return from;
    };
    let middle = top + heights[from] / 2.0 + moved;
    let last = tops.len().saturating_sub(1);
    (0..tops.len())
        .find(|&i| middle < tops[i] + heights[i])
        .unwrap_or(last)
}

pub struct List {
    id: String,
    style: ListStyle,
    width: f32,
    items: Vec<ListItem>,
    header: Option<String>,
    empty: Option<String>,
    dividers: Dividers,
    segmented: Option<bool>,
    selection: Selection,
    on_select: Option<Rc<dyn Fn(usize, bool)>>,
    reorder: Option<Rc<dyn Fn(usize, usize)>>,
    keyboard: bool,
    window: Option<(f32, f32)>,
    backdrop: bool,
}

impl List {
    /// A list keyed `id` (its focus and drags are kept by it), the
    /// popover's width until [`List::width`] says otherwise.
    pub fn new(id: impl Into<String>, items: Vec<ListItem>) -> Self {
        Self {
            id: id.into(),
            style: ListStyle::default(),
            width: sys::size::popover::width(),
            items,
            header: None,
            empty: None,
            dividers: Dividers::None,
            segmented: None,
            selection: Selection::None,
            on_select: None,
            reorder: None,
            keyboard: false,
            window: None,
            backdrop: false,
        }
    }

    pub fn style(mut self, style: ListStyle) -> Self {
        self.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// A subheader over the items.
    pub fn header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }

    /// A note in place of the items while there are none.
    pub fn empty(mut self, note: impl Into<String>) -> Self {
        self.empty = Some(note.into());
        self
    }

    pub fn dividers(mut self, dividers: Dividers) -> Self {
        self.dividers = dividers;
        self
    }

    /// Whether the items are segments of one rounded group: the expressive
    /// style's are, by default.
    pub fn segmented(mut self, segmented: bool) -> Self {
        self.segmented = Some(segmented);
        self
    }

    /// Binds what's selected to the items' fills and controls: `on_change`
    /// hears an item set on or off, by its index.
    pub fn select(
        mut self,
        selection: Selection,
        on_change: impl Fn(usize, bool) + 'static,
    ) -> Self {
        self.selection = selection;
        self.on_select = Some(Rc::new(on_change));
        self
    }

    /// Lets the items be dragged into another order by their handles (a
    /// [`Trailing::DragHandle`] is added to each). `on_move` hears an item
    /// moved from one index to another.
    pub fn reorder(mut self, on_move: impl Fn(usize, usize) + 'static) -> Self {
        self.reorder = Some(Rc::new(on_move));
        self
    }

    /// Lets the keyboard move through the items: pass the window's keys to
    /// [`handle_key`] with this list's id.
    pub fn keyboard(mut self, keyboard: bool) -> Self {
        self.keyboard = keyboard;
        self
    }

    /// Builds only the items within `height` of `offset` (scrolled that
    /// far), for a long list in a scroll view. The items must all be the
    /// first one's height.
    pub fn window(mut self, offset: f32, height: f32) -> Self {
        self.window = Some((offset, height));
        self
    }

    /// Fills the list's own area with the style's backdrop: what an M3 list
    /// sits on.
    pub fn backdrop(mut self, backdrop: bool) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub fn build(self) -> Rectangle {
        let t = self.style.tokens();
        let width = self.width;
        let segmented = self
            .segmented
            .unwrap_or(self.style == ListStyle::M3Expressive);
        let gap = if self.dividers == Dividers::None {
            t.item_gap
        } else {
            t.item_gap.max(sys::border::hairline())
        };
        let focus = if self.keyboard {
            nav::focused(&self.id)
        } else {
            None
        };
        let drag = DRAGS.with_borrow(|drags| drags.get(&self.id).copied());

        let mut layers: Vec<Box<dyn Widget>> = Vec::new();
        let mut y = 0.0;

        if let Some(header) = &self.header {
            let (style, color, h) = match self.style {
                ListStyle::Shell => (
                    sys::r#type::caption(),
                    crate::material::label(crate::material::Level::Secondary),
                    t.headline.height + sys::space::gap::s(),
                ),
                // title-small, in a 48 dp row
                _ => {
                    let mut style = t.supporting.style;
                    style.weight = 500;
                    style.letter_spacing = Some(0.1);
                    (style, t.overline_color, 48.0)
                }
            };
            layers.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(h))
                    .fill(Color::TRANSPARENT)
                    .padding(crate::layout::pad(t.leading_space, 0.0))
                    .align_child(Start, Center)
                    .child(Text::new(header.clone()).style(style).color(color)),
            ));
            y += h;
        }

        if self.items.is_empty()
            && let Some(note) = &self.empty
        {
            {
                let h = t.min_height(1);
                layers.push(boxed(
                    Rectangle::new()
                        .width(Size::Fixed(width))
                        .height(Size::Fixed(h))
                        .fill(Color::TRANSPARENT)
                        .padding(crate::layout::pad(t.leading_space, 0.0))
                        .align_child(Start, Center)
                        .translate(0.0, y)
                        .child(
                            Text::new(note.clone())
                                .style(t.supporting.style)
                                .color(t.supporting_color),
                        ),
                ));
                y += h;
            }
        }

        let count = self.items.len();
        let reorder = self.reorder.clone();
        let mut items = self.items;
        for (i, item) in items.iter_mut().enumerate() {
            if let Some(on_select) = &self.on_select {
                let on = self.selection.contains(i);
                item.selected = on;
                item.set_control(on);
                let on_select = on_select.clone();
                item.toggle = Some(Rc::new(move |next| on_select(i, next)));
            }
            if reorder.is_some()
                && !item
                    .trailing
                    .iter()
                    .any(|t| matches!(t, Trailing::DragHandle))
            {
                item.trailing.push(Trailing::DragHandle);
            }
        }

        let ctx = |i: usize| Context {
            segment: segmented.then_some(Segment { index: i, count }),
            focused: focus == Some(i),
            dragged: drag.is_some_and(|d| d.from == i),
        };
        let mut items = items.into_iter().enumerate();

        // only the rows in view, for a long list: the first item, built,
        // says how far apart they all are
        let mut first_built = None;
        let (first, last, pitch) = match self.window {
            Some((offset, height)) if count > 0 => {
                let (_, item) = items.next().expect("count > 0");
                let built = item.build(&t, width, ctx(0));
                let pitch = built.height + gap;
                let margin = pitch * 2.0;
                let first = ((offset - y - margin) / pitch).floor().max(0.0) as usize;
                let last =
                    (((offset + height + margin - y) / pitch).ceil().max(0.0) as usize).min(count);
                first_built = Some(built);
                (first, last, Some(pitch))
            }
            _ => (0, count, None),
        };

        let mut built = Vec::with_capacity(last.saturating_sub(first));
        let mut tops = Vec::with_capacity(built.capacity());
        let mut nav_items = Vec::with_capacity(count);
        let start_y = y;
        let mut i = 0;
        loop {
            let next = if i == 0 && first_built.is_some() {
                None
            } else {
                match items.next() {
                    Some((_, item)) => Some(item),
                    None => break,
                }
            };
            let index = i;
            i += 1;
            if index >= count {
                break;
            }
            if index < first || index >= last {
                // still focusable, for the keyboard, though not built
                nav_items.push((true, None));
                continue;
            }
            let b = match next {
                Some(item) => item.build(&t, width, ctx(index)),
                None => first_built.take().expect("the first item"),
            };
            let i = index;
            let top = match pitch {
                Some(pitch) => start_y + i as f32 * pitch,
                None => y,
            };
            y = top + b.height + gap;
            nav_items.push((b.focusable, b.press.clone()));
            tops.push(top);
            built.push((i, b));
        }
        if count > 0 {
            y -= gap;
        }
        if let Some(pitch) = pitch {
            y = start_y + count as f32 * pitch - gap;
        }
        if self.keyboard {
            nav::register(&self.id, nav_items);
        }

        // the drag in flight: where the dragged item would land, and how
        // far the others move aside for it
        let heights: Vec<f32> = built.iter().map(|(_, b)| b.height).collect();
        let landing = drag.map(|d| {
            let local = d.from.saturating_sub(first);
            (
                local,
                drop_index(&tops, &heights, local, d.at - d.start),
                d.at - d.start,
            )
        });

        // dividers between the items, while nothing is being dragged
        if self.dividers != Dividers::None && landing.is_none() {
            for k in 0..built.len().saturating_sub(1) {
                let (_, b) = &built[k];
                let (leading, trailing) = match self.dividers {
                    Dividers::Full | Dividers::None => (0.0, 0.0),
                    Dividers::Inset => (b.text_x, 0.0),
                    Dividers::Middle => (t.leading_space, t.trailing_space),
                };
                let at = tops[k] + b.height + (gap - sys::border::hairline()) / 2.0;
                layers.push(boxed(
                    Divider::inset(width, leading, trailing, t.divider).translate(0.0, at),
                ));
            }
        }

        let mut dragged_layer = None;
        for (k, (i, b)) in built.into_iter().enumerate() {
            let shift = match landing {
                Some((from, to, _)) if k != from => {
                    let room = heights[from] + gap;
                    if from < to && k > from && k <= to {
                        -room
                    } else if to < from && k >= to && k < from {
                        room
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            let key = format!("{}.{}", self.id, i);
            match landing {
                Some((from, _, moved)) if k == from => {
                    let moved = moved.clamp(-tops[k], (y - tops[k] - b.height).max(0.0));
                    dragged_layer = Some(b.rect.translate(0.0, tops[k] + moved));
                }
                _ => {
                    let shift = state::spring(&key, "shift", shift);
                    layers.push(boxed(b.rect.translate(0.0, tops[k] + shift)));
                }
            }
        }
        if let Some(layer) = dragged_layer {
            layers.push(boxed(layer));
        }

        // the handles' strip: one still layer the drags are measured on
        if let Some(on_move) = reorder {
            let strip = t.trailing_space + t.icon + t.trailing_gap / 2.0;
            let tops_at = tops.clone();
            let id = self.id.clone();
            let release_id = self.id.clone();
            let tops_release = tops.clone();
            let heights_release = heights.clone();
            layers.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(strip))
                    .height(Size::Fixed(y.max(0.0)))
                    .fill(Color::TRANSPARENT)
                    .translate(width - strip, 0.0)
                    .cursor(Cursor::Grab)
                    .on_drag(move |p: Point| {
                        DRAGS.with_borrow_mut(|drags| match drags.get_mut(&id) {
                            Some(drag) => drag.at = p.y,
                            None => {
                                let from = tops_at.iter().rposition(|&top| top <= p.y);
                                if let Some(from) = from {
                                    drags.insert(
                                        id.clone(),
                                        Drag {
                                            from: from + first,
                                            start: p.y,
                                            at: p.y,
                                        },
                                    );
                                }
                            }
                        });
                        request_frame();
                    })
                    .on_release(move |_: Point| {
                        let drag = DRAGS.with_borrow_mut(|drags| drags.remove(&release_id));
                        if let Some(d) = drag {
                            let from = d.from.saturating_sub(first);
                            let to =
                                drop_index(&tops_release, &heights_release, from, d.at - d.start);
                            if to != from {
                                crate::buttons::swallow_click();
                                on_move(d.from, to + first);
                            }
                        }
                        request_frame();
                    }),
            ));
        }

        let height = y.max(0.0);
        Rectangle::new()
            .width(Size::Fixed(width))
            .height(Size::Fixed(height))
            .fill(if self.backdrop {
                t.backdrop
            } else {
                Color::TRANSPARENT
            })
            .child(
                Stack::new(layers)
                    .width(Size::Fixed(width))
                    .height(Size::Fixed(height)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_lands_where_its_middle_is() {
        let tops = [0.0, 56.0, 112.0, 168.0];
        let heights = [56.0; 4];
        assert_eq!(drop_index(&tops, &heights, 0, 0.0), 0);
        assert_eq!(drop_index(&tops, &heights, 0, 30.0), 1);
        assert_eq!(drop_index(&tops, &heights, 0, 500.0), 3);
        assert_eq!(drop_index(&tops, &heights, 3, -200.0), 0);
        assert_eq!(drop_index(&tops, &heights, 2, -20.0), 2);
    }

    #[test]
    fn selections_toggle() {
        let single = Selection::Single(Some(1));
        assert_eq!(single.toggled(2, true), Selection::Single(Some(2)));
        let multi = Selection::Multi(vec![0, 2]);
        assert_eq!(multi.toggled(1, true), Selection::Multi(vec![0, 1, 2]));
        assert_eq!(multi.toggled(0, false), Selection::Multi(vec![2]));
        assert!(multi.contains(2) && !multi.contains(1));
    }

    #[test]
    fn a_windowed_list_keeps_its_whole_height() {
        let items: Vec<ListItem> = (0..1000)
            .map(|i| ListItem::new(format!("w{i}"), "Item").on_press(|| {}))
            .collect();
        let list = List::new("w.list", items)
            .style(ListStyle::M3)
            .width(360.0)
            .keyboard(true)
            .window(5_000.0, 300.0)
            .build();
        assert_eq!(list.height(), Size::Fixed(1000.0 * 56.0));
        // every item is still there for the keyboard
        set_focus("w.list", Some(998), true);
        assert!(handle_key("w.list", amane::Key::Down));
        assert_eq!(focused("w.list"), Some(999));
    }

    #[test]
    fn a_list_is_as_tall_as_its_items_and_gaps() {
        let items = || {
            (0..3)
                .map(|i| ListItem::new(format!("h{i}"), "Item"))
                .collect::<Vec<_>>()
        };
        let base = List::new("h.base", items())
            .style(ListStyle::M3)
            .width(360.0)
            .build();
        assert_eq!(base.height(), Size::Fixed(3.0 * 56.0));
        let expressive = List::new("h.exp", items())
            .style(ListStyle::M3Expressive)
            .width(360.0)
            .build();
        assert_eq!(expressive.height(), Size::Fixed(3.0 * 56.0 + 2.0 * 2.0));
    }
}
