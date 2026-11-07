//! Thumbnail picker — choices shown as small pictures of what each looks
//! like, as an Appearance setting offers Light, Dark and Auto: the
//! chosen one ringed in the accent, its name under each. The pictures are
//! anyone's widgets, drawn at the picker's size; they wrap to the width
//! given.

#![allow(missing_docs)]

use std::rc::Rc;

use amane::{Center, Color, Column, Rectangle, Row, Size, Text, Widget};

use crate::controls::parts::{interactive, on_press};
use crate::layout::{boxed, text_height};
use crate::style::{self, px};
use crate::tokens::Styled;

/// One choice: its picture and its name.
struct Choice {
    picture: Box<dyn Widget>,
    label: String,
}

/// Choices as pictures in a row. `id` keys their hover.
pub struct ThumbnailPicker {
    id: String,
    choices: Vec<Choice>,
    chosen: Option<usize>,
    size: (f32, f32),
    width: f32,
    disabled: bool,
    on_select: Option<Rc<dyn Fn(usize)>>,
}

/// The ring around the chosen picture, and the room it takes.
fn ring() -> f32 {
    px(3.0)
}

fn gap() -> f32 {
    px(14.0)
}

impl ThumbnailPicker {
    /// Pictures `width` × `height`.
    pub fn new(id: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            id: id.into(),
            choices: Vec::new(),
            chosen: None,
            size: (width, height),
            width: f32::INFINITY,
            disabled: false,
            on_select: None,
        }
    }

    /// A choice: `picture`, drawn at the picker's size, and its name.
    pub fn option(mut self, picture: impl Widget + 'static, label: impl Into<String>) -> Self {
        self.choices.push(Choice {
            picture: boxed(picture),
            label: label.into(),
        });
        self
    }

    /// The one chosen now.
    pub fn chosen(mut self, chosen: Option<usize>) -> Self {
        self.chosen = chosen;
        self
    }

    /// Wraps to rows no wider than this.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_select(mut self, select: impl Fn(usize) + 'static) -> Self {
        self.on_select = Some(Rc::new(select));
        self
    }

    /// How wide one choice is, with its ring.
    fn cell_width(&self) -> f32 {
        self.size.0 + 2.0 * ring()
    }

    /// How many choices fit in a row.
    fn per_row(&self) -> usize {
        let fits = ((self.width + gap()) / (self.cell_width() + gap())).floor();
        (fits as usize).max(1)
    }

    fn cell_height(&self) -> f32 {
        self.size.1 + 2.0 * ring() + px(6.0) + text_height(style::subtitle_font())
    }

    /// How tall it is, for a row's detail.
    pub fn height(&self) -> f32 {
        let rows = self.choices.len().div_ceil(self.per_row()).max(1);
        rows as f32 * self.cell_height() + (rows - 1) as f32 * gap()
    }

    pub fn build(self) -> Column {
        let per_row = self.per_row();
        let (width, height) = self.size;
        let cell_width = self.cell_width();
        let cell_height = self.cell_height();
        let mut rows: Vec<Box<dyn Widget>> = Vec::new();
        let mut row: Vec<Box<dyn Widget>> = Vec::new();
        let count = self.choices.len();
        for (index, choice) in self.choices.into_iter().enumerate() {
            let chosen = self.chosen == Some(index);
            let ring_color = if chosen {
                style::tint()
            } else {
                Color::TRANSPARENT
            };
            let picture = Rectangle::new()
                .width(Size::Fixed(width))
                .height(Size::Fixed(height))
                .radius(px(7.0))
                .border(1.0, style::divider())
                .clip()
                .child(Column::new(vec![choice.picture]));
            let framed = Rectangle::new()
                .width(Size::Fixed(width + 2.0 * ring()))
                .height(Size::Fixed(height + 2.0 * ring()))
                .radius(px(7.0) + ring())
                .border(ring(), ring_color)
                .align_child(Center, Center)
                .child(picture);
            let label = Text::new(choice.label)
                .style(style::subtitle_font())
                .color(if chosen {
                    style::label_primary()
                } else {
                    style::label_secondary()
                });
            let cell = Rectangle::new()
                .width(Size::Fixed(cell_width))
                .height(Size::Fixed(cell_height))
                .fill(Color::TRANSPARENT)
                .align_child(Center, amane::Start)
                .child(
                    Column::new(vec![boxed(framed), boxed(label)])
                        .gap(px(6.0))
                        .align(Center),
                );
            let select = self.on_select.clone();
            let cell = interactive(
                cell,
                format!("{}.{index}", self.id),
                self.disabled,
                on_press(move || {
                    if let Some(select) = &select {
                        select(index);
                    }
                }),
            );
            row.push(boxed(cell));
            if row.len() == per_row || index + 1 == count {
                rows.push(boxed(Row::new(std::mem::take(&mut row)).gap(gap())));
            }
        }
        Column::new(rows).gap(gap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker(count: usize, width: f32) -> ThumbnailPicker {
        let mut picker = ThumbnailPicker::new("test", 80.0, 54.0).width(width);
        for index in 0..count {
            picker = picker.option(Column::new(Vec::new()), format!("{index}"));
        }
        picker
    }

    #[test]
    fn choices_wrap_to_the_width() {
        assert!(picker(3, 1000.0).per_row() >= 3);
        let narrow = picker(4, 80.0 + 2.0 * ring());
        assert_eq!(narrow.per_row(), 1);
        assert!(narrow.height() > picker(4, 1000.0).height());
    }
}
