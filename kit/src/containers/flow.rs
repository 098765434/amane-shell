//! Flow — fixed-size items laid out in rows, wrapping at a width, as a
//! collection of unlike tiles reads: each row takes as many items as fit
//! across, `gap_x` apart, the items at the top of their row, and the rows
//! `gap_y` apart. Quick Settings' Customize window lays its controls out
//! with it. amane has no wrapping layout, so the items are a stack of
//! translated rectangles.

use amane::{Color, Rectangle, Size, Stack, Widget};

use crate::layout::boxed;

/// Where an item was put, in the flow's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct Flow {
    width: f32,
    gap_x: f32,
    gap_y: f32,
    items: Vec<(f32, f32, Box<dyn Widget>)>,
}

impl Flow {
    /// Rows `width` wide, the items `gap_x` apart and the rows `gap_y`.
    pub fn new(width: f32, gap_x: f32, gap_y: f32) -> Self {
        Self {
            width,
            gap_x,
            gap_y,
            items: Vec::new(),
        }
    }

    /// One more item, `width` by `height`.
    pub fn item(mut self, width: f32, height: f32, widget: impl Widget + 'static) -> Self {
        self.items.push((width, height, boxed(widget)));
        self
    }

    /// The flow, how tall it is, and where each item went (in the order
    /// they were added).
    pub fn build(self) -> (Rectangle, f32, Vec<Placed>) {
        let sizes: Vec<(f32, f32)> = self.items.iter().map(|(w, h, _)| (*w, *h)).collect();
        let placed = place(self.width, self.gap_x, self.gap_y, &sizes);
        let height = placed
            .iter()
            .map(|p| p.y + p.height)
            .fold(0.0_f32, f32::max);
        let layers: Vec<Box<dyn Widget>> = self
            .items
            .into_iter()
            .zip(placed.iter())
            .map(|((w, h, widget), at)| {
                boxed(
                    Rectangle::new()
                        .width(Size::Fixed(w))
                        .height(Size::Fixed(h))
                        .fill(Color::TRANSPARENT)
                        .translate(at.x, at.y)
                        .child(Stack::new(vec![widget])),
                )
            })
            .collect();
        let flow = Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(height.max(1.0)))
            .fill(Color::TRANSPARENT)
            .child(Stack::new(layers));
        (flow, height, placed)
    }
}

/// Where items of `sizes` go in rows `width` wide: each row takes items
/// until the next wouldn't fit (an item wider than the row has a row to
/// itself), items at the top of their row, rows as tall as their tallest.
pub fn place(width: f32, gap_x: f32, gap_y: f32, sizes: &[(f32, f32)]) -> Vec<Placed> {
    let mut placed = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut row_h) = (0.0_f32, 0.0_f32, 0.0_f32);
    for &(w, h) in sizes {
        let fits = x == 0.0 || x + w <= width + 0.001;
        if !fits {
            x = 0.0;
            y += row_h + gap_y;
            row_h = 0.0;
        }
        placed.push(Placed {
            x,
            y,
            width: w,
            height: h,
        });
        x += w + gap_x;
        row_h = row_h.max(h);
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xy(placed: &[Placed]) -> Vec<(f32, f32)> {
        placed.iter().map(|p| (p.x, p.y)).collect()
    }

    #[test]
    fn items_fill_a_row_then_wrap() {
        let sizes = [(100.0, 50.0), (100.0, 50.0), (100.0, 50.0)];
        // two fit across 210 with a gap of 10; the third wraps under
        let placed = place(210.0, 10.0, 20.0, &sizes);
        assert_eq!(xy(&placed), vec![(0.0, 0.0), (110.0, 0.0), (0.0, 70.0)]);
    }

    #[test]
    fn a_row_is_as_tall_as_its_tallest_item() {
        let sizes = [(50.0, 50.0), (50.0, 120.0), (50.0, 50.0), (50.0, 50.0)];
        let placed = place(170.0, 10.0, 8.0, &sizes);
        assert_eq!(
            xy(&placed),
            vec![(0.0, 0.0), (60.0, 0.0), (120.0, 0.0), (0.0, 128.0)]
        );
    }

    #[test]
    fn an_item_too_wide_takes_a_row_of_its_own() {
        let sizes = [(50.0, 50.0), (500.0, 50.0), (50.0, 50.0)];
        let placed = place(200.0, 10.0, 10.0, &sizes);
        assert_eq!(xy(&placed), vec![(0.0, 0.0), (0.0, 60.0), (0.0, 120.0)]);
    }

    #[test]
    fn nothing_takes_no_room() {
        assert!(place(100.0, 1.0, 1.0, &[]).is_empty());
    }
}
