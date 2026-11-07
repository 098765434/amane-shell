//! Avatar — a person or thing as a circle: their photo, or their initials
//! on a colour.
//!
//! The circle's colour is kept for the name, from the accents that take
//! white text, so a person reads the same everywhere (Messages' inbox and
//! threads).

use std::path::PathBuf;

use amane::{Center, Color, Column, Image, Radius, Rectangle, Text, Widget};

use crate::color::Accent;
use crate::icons::ICON_PERSON;
use crate::layout::{boxed, icon};
use crate::tokens::{Styled, TextStyle};

/// A person or thing as a circle: their photo, or their initials on a color.
pub struct Avatar {
    name: String,
    side: f32,
    fill: Option<Color>,
    text: Color,
    style: Option<TextStyle>,
    photo: Option<PathBuf>,
}

impl Avatar {
    /// `name`'s avatar, `side` across.
    pub fn new(name: impl Into<String>, side: f32) -> Self {
        Self {
            name: name.into(),
            side,
            fill: None,
            text: Color::WHITE,
            style: None,
            photo: None,
        }
    }

    /// The circle's colour and the initials', in place of the name's own.
    pub fn colors(mut self, fill: Color, text: Color) -> Self {
        self.fill = Some(fill);
        self.text = text;
        self
    }

    /// The initials' type, in place of a weight-600 size from the side.
    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// A photo in place of the initials, decoded at the circle's size.
    pub fn photo(mut self, photo: Option<PathBuf>) -> Self {
        self.photo = photo;
        self
    }

    /// The circle, `side` across.
    pub fn build(self) -> Rectangle {
        let circle = Rectangle::new()
            .width(self.side)
            .height(self.side)
            .radius(Radius::Full);
        if let Some(photo) = self.photo {
            let px = (self.side * 2.0) as u32;
            return circle.fill(Image::cover(photo).thumbnail(px, px)).clip();
        }

        let mark: Box<dyn Widget> = match initials(&self.name) {
            None => boxed(icon(ICON_PERSON, self.side * 0.55, self.text)),
            Some(initials) => match self.style {
                Some(style) => boxed(Text::new(initials).style(style).color(self.text)),
                None => boxed(
                    Text::new(initials)
                        .size((self.side * 0.39).round())
                        .weight(600)
                        .color(self.text),
                ),
            },
        };
        circle
            .fill(self.fill.unwrap_or_else(|| colour_for(&self.name)))
            .align_child(Center, Center)
            .child(Column::new(vec![mark]).align(Center))
    }
}

/// Up to two initials, from the first letters of the words; none for a
/// name with no letters (a bare number).
pub fn initials(name: &str) -> Option<String> {
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .filter(|c| c.is_alphabetic())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    (!initials.is_empty()).then_some(initials)
}

/// The same colour for the same name, from the accents that take white
/// text; gray for a name with no letters.
pub fn colour_for(name: &str) -> Color {
    const COLOURS: [Accent; 8] = [
        Accent::Blue,
        Accent::Green,
        Accent::Orange,
        Accent::Purple,
        Accent::Pink,
        Accent::Teal,
        Accent::Indigo,
        Accent::Brown,
    ];
    let hash = name.bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    if name.chars().any(char::is_alphabetic) {
        COLOURS[hash as usize % COLOURS.len()].color()
    } else {
        Accent::Gray.color()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_take_two_letters() {
        assert_eq!(initials("Ada Lovelace King").as_deref(), Some("AL"));
        assert_eq!(initials("bob").as_deref(), Some("B"));
        assert_eq!(initials("+47 123"), None);
    }

    #[test]
    fn a_name_keeps_its_colour() {
        assert_eq!(colour_for("Ada"), colour_for("Ada"));
        assert_eq!(colour_for("123"), Accent::Gray.color());
    }
}
