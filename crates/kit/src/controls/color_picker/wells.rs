//! Color wells — the accent colors as a row of dots, as an Appearance
//! setting offers them: the chosen one with a white dot in it,
//! and a "multicolor" first dot for following the system. Hovering a dot
//! names it, for the row's value.

use std::rc::Rc;

use amane::{Center, Color, Rectangle, Row, Size, Widget};

use crate::buttons::hovered;
use crate::controls::parts::{interactive, on_press};
use crate::layout::boxed;
use crate::style::{self, px};

/// One dot: its name and its color, or none for the system's.
pub struct ColorWells {
    id: String,
    colors: Vec<(String, Option<Color>)>,
    chosen: usize,
    on_select: Option<Rc<dyn Fn(usize)>>,
}

fn side() -> f32 {
    px(18.0)
}

fn gap() -> f32 {
    px(8.0)
}

/// The colors a "multicolor" dot is made of, a quarter each. TODO(token): ref.color.system.*
const SYSTEM: [&str; 4] = ["#0091ff", "#ff4245", "#ffd600", "#30d158"];

impl ColorWells {
    /// A row of `colors`, each a name and a color or `None` for the system's, with the one at
    /// `chosen` marked. `id` keys the dots' hover, so it must be unique and stable.
    pub fn new(id: impl Into<String>, colors: Vec<(String, Option<Color>)>, chosen: usize) -> Self {
        Self {
            id: id.into(),
            colors,
            chosen,
            on_select: None,
        }
    }

    /// Runs with a dot's index when it's clicked.
    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Some(Rc::new(select));
        self
    }

    /// The name of the dot under the pointer, or the chosen one's.
    pub fn shown_name(&self) -> String {
        let hovered = (0..self.colors.len()).find(|index| hovered(&format!("{}.{index}", self.id)));
        let index = hovered.unwrap_or(self.chosen);
        self.colors
            .get(index)
            .map(|(name, _)| name.clone())
            .unwrap_or_default()
    }

    /// The row's width in logical pixels.
    pub fn width(&self) -> f32 {
        let count = self.colors.len() as f32;
        count * side() + (count - 1.0).max(0.0) * gap()
    }

    /// The row of dots.
    pub fn build(self) -> Row {
        let dots: Vec<Box<dyn Widget>> = self
            .colors
            .iter()
            .enumerate()
            .map(|(index, (_, color))| {
                let face: Box<dyn Widget> = match color {
                    Some(color) => boxed(
                        Rectangle::new()
                            .width(Size::Fixed(side()))
                            .height(Size::Fixed(side()))
                            .radius(side() / 2.0)
                            .fill(*color),
                    ),
                    // four quarters of color in a round clip
                    None => boxed(
                        Rectangle::new()
                            .width(Size::Fixed(side()))
                            .height(Size::Fixed(side()))
                            .radius(side() / 2.0)
                            .clip()
                            .child(amane::Column::new(
                                [[0, 1], [3, 2]]
                                    .into_iter()
                                    .map(|pair| {
                                        boxed(Row::new(
                                            pair.into_iter()
                                                .map(|at| {
                                                    boxed(
                                                        Rectangle::new()
                                                            .width(Size::Fixed(side() / 2.0))
                                                            .height(Size::Fixed(side() / 2.0))
                                                            .fill(Color::from(SYSTEM[at])),
                                                    )
                                                })
                                                .collect(),
                                        ))
                                    })
                                    .collect(),
                            )),
                    ),
                };
                let chosen = index == self.chosen;
                let mark: Box<dyn Widget> = if chosen {
                    boxed(
                        Rectangle::new()
                            .width(Size::Fixed(px(6.0)))
                            .height(Size::Fixed(px(6.0)))
                            .radius(px(3.0))
                            .fill(Color::WHITE),
                    )
                } else {
                    boxed(Row::new(Vec::new()))
                };
                let dot = Rectangle::new()
                    .width(Size::Fixed(side()))
                    .height(Size::Fixed(side()))
                    .radius(side() / 2.0)
                    .border(1.0, style::divider())
                    .align_child(Center, Center)
                    .child(amane::Stack::new(vec![
                        face,
                        boxed(
                            Rectangle::new()
                                .width(Size::Fixed(side()))
                                .height(Size::Fixed(side()))
                                .fill(Color::TRANSPARENT)
                                .align_child(Center, Center)
                                .child(amane::Column::new(vec![mark])),
                        ),
                    ]));
                let select = self.on_select.clone();
                boxed(interactive(
                    dot,
                    format!("{}.{index}", self.id),
                    false,
                    on_press(move || {
                        if let Some(select) = &select {
                            select(index);
                        }
                    }),
                ))
            })
            .collect();
        Row::new(dots).gap(gap()).align(Center)
    }
}
