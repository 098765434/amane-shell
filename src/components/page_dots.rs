//! Page dots — which of several pages shows, as a row or column of dots.
//!
//! Modelled on iOS's page control (M3 has none): one dot per page, the
//! current one lit and the rest dimmed, drawn in the shell's macOS look.
//! The current page may be fractional while pages slide, so the light
//! moves between dots as they do. A second mark rings one dot (the dock
//! rings the workspace that is active, while another is browsed), and a
//! click on a dot picks its page.
//!
//! The dots squeeze closer when they'd run past the length they're given,
//! so any number of pages fits.

use std::rc::Rc;

use amane::{Blend, Button, Center, Color, Column, Pointer, Rectangle, Row, Widget};

use crate::components::layout::boxed;
use crate::tokens::sys;

// TODO(token): `sys.size.page-dot` once Penpot has it (manual §1.1).
/// How big a dot is.
const DOT: f32 = 6.0;

/// Which way the dots run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}

pub struct PageDots {
    count: usize,
    current: f32,
    marked: Option<usize>,
    axis: Axis,
    fit: Option<f32>,
    on_pick: Option<Rc<dyn Fn(usize)>>,
}

impl PageDots {
    /// `count` dots, `current` of the way along them lit (0 is the first).
    pub fn new(count: usize, current: f32) -> Self {
        Self {
            count,
            current,
            marked: None,
            axis: Axis::Row,
            fit: None,
            on_pick: None,
        }
    }

    /// Runs the dots down instead of across.
    pub fn vertical(mut self) -> Self {
        self.axis = Axis::Column;
        self
    }

    /// Rings the dot of page `index`.
    pub fn marked(mut self, index: Option<usize>) -> Self {
        self.marked = index;
        self
    }

    /// Keeps the dots within `length` along their axis, closer together
    /// when they'd run past it.
    pub fn fit(mut self, length: f32) -> Self {
        self.fit = Some(length);
        self
    }

    /// Calls `pick` with a dot's page when it's clicked.
    pub fn on_pick(mut self, pick: impl Fn(usize) + 'static) -> Self {
        self.on_pick = Some(Rc::new(pick));
        self
    }

    /// How far apart dots' centres are: a dot and `gap.xs`, less where
    /// they'd overrun the length they fit in.
    fn pitch(&self) -> f32 {
        let pitch = DOT + sys::space::gap::xs();
        match self.fit {
            Some(length) if self.count > 1 => pitch.min((length - DOT) / (self.count - 1) as f32),
            _ => pitch,
        }
        .max(1.0)
    }

    /// How long the dots run, and how thick they are across.
    pub fn size(&self) -> (f32, f32) {
        let length = if self.count == 0 {
            0.0
        } else {
            DOT + self.pitch() * (self.count - 1) as f32
        };
        match self.axis {
            Axis::Row => (length, DOT),
            Axis::Column => (DOT, length),
        }
    }

    pub fn build(self) -> Box<dyn Widget> {
        let pitch = self.pitch();
        let dots: Vec<Box<dyn Widget>> = (0..self.count)
            .map(|index| boxed(self.dot(index, pitch)))
            .collect();
        let gap = pitch - DOT;
        match self.axis {
            Axis::Row => boxed(Row::new(dots).gap(gap).align(Center)),
            Axis::Column => boxed(Column::new(dots).gap(gap).align(Center)),
        }
    }

    fn dot(&self, index: usize, pitch: f32) -> Rectangle {
        let lit = lit(self.current, index);
        let fill = Color::blend(
            sys::color::content::secondary(),
            sys::color::surface::panel::fg(),
            lit,
        );
        let dot = Rectangle::new()
            .width(DOT)
            .height(DOT)
            .radius(DOT / 2.0)
            .fill(fill);
        let dot = if self.marked == Some(index) && lit < 0.5 {
            dot.border(sys::border::hairline(), sys::color::surface::panel::fg())
        } else {
            dot
        };
        let Some(pick) = self.on_pick.clone() else {
            return dot;
        };
        // the dot takes the clicks of the room around it too
        let (width, height) = match self.axis {
            Axis::Row => (pitch.max(DOT), DOT),
            Axis::Column => (DOT, pitch.max(DOT)),
        };
        Rectangle::new()
            .width(width)
            .height(height)
            .fill(Color::TRANSPARENT)
            .align_child(Center, Center)
            .cursor(Pointer)
            .on_click(move |button| {
                if button == Button::Left {
                    pick(index);
                }
            })
            .child(dot)
    }
}

/// How lit the dot of page `index` is with `current` showing: fully at
/// it, fading to nothing a page away.
fn lit(current: f32, index: usize) -> f32 {
    (1.0 - (current - index as f32).abs()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_light_moves_between_dots_as_pages_slide() {
        assert_eq!(lit(1.0, 1), 1.0);
        assert_eq!(lit(1.0, 0), 0.0);
        assert_eq!(lit(0.25, 0), 0.75);
        assert_eq!(lit(0.25, 1), 0.25);
        assert_eq!(lit(0.25, 2), 0.0);
    }

    #[test]
    fn dots_squeeze_to_fit_their_length() {
        let loose = PageDots::new(3, 0.0).vertical();
        let spaced = DOT + sys::space::gap::xs();
        assert_eq!(loose.size().1, DOT + spaced * 2.0);

        let tight = PageDots::new(11, 0.0).vertical().fit(36.0);
        assert!((tight.size().1 - 36.0).abs() < 0.01);
        assert_eq!(PageDots::new(0, 0.0).size(), (0.0, DOT));
    }
}
