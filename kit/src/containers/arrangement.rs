//! Arrangement — screens laid out as they sit side by side, to drag into
//! place: macOS's Arrange view in the Displays settings.
//!
//! Each [`Screen`] is a rounded rectangle at its place in the layout
//! (logical pixels), scaled to fit the area, named in its middle, the
//! selected one in the accent. A press picks a screen; a drag carries it,
//! and on release it snaps against the nearest edge of another so they
//! touch, lining up with that one's edge when close. All quads.
//!
//! The drag is measured on one still layer under the screens, the size of
//! the area, so the screen moving under the pointer never changes what the
//! pointer is measured against.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amane::{Center, Color, Cursor, Parent, Point, Rectangle, Size, Stack, Text, Widget};

use crate::color::with_alpha;
use crate::layout::boxed;
use crate::material::{self, FillLevel, Level};
use crate::tokens::{Styled, sys};

/// A screen where it sits, in logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Screen {
    pub fn new(label: impl Into<String>, x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            label: label.into(),
            x,
            y,
            width: width.max(1),
            height: height.max(1),
        }
    }
}

/// A screen in hand: which, where in it the pointer took it, where it is
/// now (area pixels), and whether it has moved yet.
#[derive(Clone, Copy, Debug)]
struct Held {
    index: usize,
    grab: (f32, f32),
    at: (f32, f32),
    moved: bool,
}

thread_local! {
    static HELD: RefCell<HashMap<String, Held>> = RefCell::new(HashMap::new());
}

/// How far a press must travel before it's a drag, not a pick.
const SLOP: f32 = 3.0;

pub struct Arrangement {
    id: String,
    screens: Vec<Screen>,
    selected: Option<usize>,
    width: f32,
    height: f32,
    on_select: Rc<dyn Fn(usize)>,
    on_move: Rc<dyn Fn(usize, i32, i32)>,
}

impl Arrangement {
    /// `id` keys the drag, so it must be unique and stable.
    pub fn new(id: impl Into<String>, screens: Vec<Screen>, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            screens,
            selected: None,
            width,
            height,
            on_select: Rc::new(|_| {}),
            on_move: Rc::new(|_, _, _| {}),
        }
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    /// Runs with the screen let go and its new top-left, snapped, in
    /// logical pixels. A lone screen can't be dragged.
    pub fn on_move(mut self, moved: impl Fn(usize, i32, i32) + 'static) -> Self {
        self.on_move = Rc::new(moved);
        self
    }

    pub fn build(self) -> Rectangle {
        let fit = Fit::new(&self.screens, self.width, self.height);
        let held = HELD.with_borrow(|held| held.get(&self.id).copied());
        let movable = self.screens.len() > 1;

        let mut layers: Vec<Box<dyn Widget>> = Vec::new();
        layers.push(boxed(self.drag_layer(fit, movable, held)));

        // the one in hand last, over the others
        let mut order: Vec<usize> = (0..self.screens.len()).collect();
        if let Some(held) = held {
            order.retain(|index| *index != held.index);
            order.push(held.index);
        }
        for index in order {
            let screen = &self.screens[index];
            let (x, y) = match held {
                Some(held) if held.index == index && held.moved => held.at,
                _ => fit.to_area(screen.x as f32, screen.y as f32),
            };
            let (w, h) = (
                screen.width as f32 * fit.scale,
                screen.height as f32 * fit.scale,
            );
            layers.push(boxed(
                tile(screen, w, h, self.selected == Some(index)).translate(x, y),
            ));
        }

        Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(self.height))
            .radius(sys::radius::raised())
            .fill(material::fill_role(FillLevel::Tertiary))
            .clip()
            .child(Stack::new(layers))
    }

    fn drag_layer(&self, fit: Fit, movable: bool, held: Option<Held>) -> Rectangle {
        let rects: Vec<(f32, f32, f32, f32)> = self
            .screens
            .iter()
            .map(|screen| {
                let (x, y) = fit.to_area(screen.x as f32, screen.y as f32);
                (
                    x,
                    y,
                    screen.width as f32 * fit.scale,
                    screen.height as f32 * fit.scale,
                )
            })
            .collect();

        let id = self.id.clone();
        let drag = move |point: Point| {
            HELD.with_borrow_mut(|all| match all.get_mut(&id) {
                Some(held) => {
                    let at = (point.x - held.grab.0, point.y - held.grab.1);
                    let start = rects[held.index];
                    if movable && ((at.0 - start.0).abs() > SLOP || (at.1 - start.1).abs() > SLOP) {
                        held.moved = true;
                    }
                    if held.moved {
                        held.at = at;
                    }
                }
                None => {
                    // the topmost under the pointer is taken
                    if let Some(index) = rects.iter().rposition(|(x, y, w, h)| {
                        point.x >= *x && point.x < x + w && point.y >= *y && point.y < y + h
                    }) {
                        let (x, y, _, _) = rects[index];
                        all.insert(
                            id.clone(),
                            Held {
                                index,
                                grab: (point.x - x, point.y - y),
                                at: (x, y),
                                moved: false,
                            },
                        );
                    }
                }
            });
        };

        let id = self.id.clone();
        let screens = self.screens.clone();
        let (select, moved) = (Rc::clone(&self.on_select), Rc::clone(&self.on_move));
        let release = move |_: Point| {
            let Some(held) = HELD.with_borrow_mut(|all| all.remove(&id)) else {
                return;
            };
            if !held.moved {
                select(held.index);
                return;
            }
            let (x, y) = fit.to_layout(held.at.0, held.at.1);
            let (x, y) = snap(&screens, held.index, x.round() as i32, y.round() as i32);
            select(held.index);
            moved(held.index, x, y);
        };

        let layer = Rectangle::new()
            .width(Parent)
            .height(Parent)
            .fill(Color::TRANSPARENT)
            .on_drag(drag)
            .on_release(release);
        match held {
            Some(held) if held.moved => layer.cursor(Cursor::Grabbing),
            _ => layer,
        }
    }
}

/// One screen: its fill, a hairline, its name in the middle.
fn tile(screen: &Screen, width: f32, height: f32, selected: bool) -> Rectangle {
    let (fill, stroke, label) = if selected {
        (
            with_alpha(sys::color::accent::bg(), 0.22),
            sys::color::accent::bg(),
            material::label(Level::Primary),
        )
    } else {
        (
            material::fill_role(FillLevel::Primary),
            material::separator(),
            material::label(Level::Secondary),
        )
    };
    Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(sys::radius::raised() / 2.0)
        .fill(fill)
        .border(
            sys::border::hairline() * if selected { 2.0 } else { 1.0 },
            stroke,
        )
        .align_child(Center, Center)
        .child(
            Text::new(screen.label.clone())
                .style(sys::r#type::caption())
                .color(label),
        )
}

/// How the layout maps onto the area: scaled to fit with a margin, and
/// centred.
#[derive(Clone, Copy, Debug)]
struct Fit {
    scale: f32,
    // the layout's top-left, and where it lands in the area
    origin: (f32, f32),
    offset: (f32, f32),
}

impl Fit {
    fn new(screens: &[Screen], width: f32, height: f32) -> Self {
        let left = screens.iter().map(|s| s.x).min().unwrap_or(0) as f32;
        let top = screens.iter().map(|s| s.y).min().unwrap_or(0) as f32;
        let right = screens.iter().map(|s| s.x + s.width).max().unwrap_or(1) as f32;
        let bottom = screens.iter().map(|s| s.y + s.height).max().unwrap_or(1) as f32;
        let (span_w, span_h) = ((right - left).max(1.0), (bottom - top).max(1.0));

        // room around, so a screen can be dragged past the others
        let margin = 0.2;
        let scale =
            (width * (1.0 - margin * 2.0) / span_w).min(height * (1.0 - margin * 2.0) / span_h);
        Self {
            scale,
            origin: (left, top),
            offset: (
                (width - span_w * scale) / 2.0,
                (height - span_h * scale) / 2.0,
            ),
        }
    }

    fn to_area(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.offset.0 + (x - self.origin.0) * self.scale,
            self.offset.1 + (y - self.origin.1) * self.scale,
        )
    }

    fn to_layout(self, x: f32, y: f32) -> (f32, f32) {
        (
            (x - self.offset.0) / self.scale + self.origin.0,
            (y - self.offset.1) / self.scale + self.origin.1,
        )
    }
}

/// Where the screen at `index` lands when let go at `x, y`: against the
/// nearest edge of another so they touch and overlap along it, its other
/// edges lining up with that screen's when within a tenth of its size.
pub fn snap(screens: &[Screen], index: usize, x: i32, y: i32) -> (i32, i32) {
    let moving = &screens[index];
    let (w, h) = (moving.width, moving.height);
    let mut best: Option<(i64, (i32, i32))> = None;
    for (other_index, other) in screens.iter().enumerate() {
        if other_index == index {
            continue;
        }
        let along_y = y.clamp(other.y - h + 1, other.y + other.height - 1);
        let along_x = x.clamp(other.x - w + 1, other.x + other.width - 1);
        let candidates = [
            (other.x + other.width, along_y),
            (other.x - w, along_y),
            (along_x, other.y + other.height),
            (along_x, other.y - h),
        ];
        for (cx, cy) in candidates {
            let (cx, cy) = align(cx, cy, w, h, other);
            let distance = i64::from(cx - x).pow(2) + i64::from(cy - y).pow(2);
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, (cx, cy)));
            }
        }
    }
    best.map_or((x, y), |(_, at)| at)
}

/// Lines a touching screen's edges up with `other`'s when close.
fn align(x: i32, y: i32, w: i32, h: i32, other: &Screen) -> (i32, i32) {
    let near = |a: i32, b: i32, size: i32| (a - b).abs() <= size / 10;
    let x = if near(x, other.x, w) {
        other.x
    } else if near(x + w, other.x + other.width, w) {
        other.x + other.width - w
    } else {
        x
    };
    let y = if near(y, other.y, h) {
        other.y
    } else if near(y + h, other.y + other.height, h) {
        other.y + other.height - h
    } else {
        y
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Vec<Screen> {
        vec![
            Screen::new("Built-in", 0, 0, 1645, 1028),
            Screen::new("Dell", 1645, 0, 2560, 1440),
        ]
    }

    #[test]
    fn a_screen_let_go_apart_snaps_to_touch() {
        // dropped a little left of and below the panel's left edge
        assert_eq!(snap(&two(), 1, -2700, 40), (-2560, 0));
        // dropped above, overlapping: it goes on top, left edges lined up
        assert_eq!(snap(&two(), 1, 60, -1300), (0, -1440));
    }

    #[test]
    fn it_lines_bottoms_up_when_close() {
        // to the right, bottom near the panel's bottom
        assert_eq!(snap(&two(), 1, 1700, -400), (1645, 1028 - 1440));
    }

    #[test]
    fn the_fit_maps_both_ways() {
        let fit = Fit::new(&two(), 400.0, 200.0);
        let (ax, ay) = fit.to_area(1645.0, 0.0);
        let (lx, ly) = fit.to_layout(ax, ay);
        assert!((lx - 1645.0).abs() < 0.01 && ly.abs() < 0.01);
    }
}
