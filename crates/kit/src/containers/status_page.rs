//! Status page — what shows in place of a list or a page with nothing in it, or that can't be
//! reached: a large faint glyph, a title, a line or two saying why or what to do, and at most
//! one button (the glyph at 55% of the label colour, the title
//! bold, the text dim), sized down for a settings pane: [`StatusPage::compact`] sits inside a
//! page, between its groups.

use amane::{Center, Column, Rectangle, Size, Text, Widget};

use crate::color::with_alpha;
use crate::layout::{boxed, icon, text_height, wrapped_lines};
use crate::style::{self, px};
use crate::tokens::{Styled, TextStyle, sys};

/// An empty or unreachable state, said once and plainly.
pub struct StatusPage {
    glyph: char,
    title: String,
    text: Option<String>,
    action: Option<Box<dyn Widget>>,
    compact: bool,
    width: f32,
}

impl StatusPage {
    /// `glyph` over `title`, as wide as a settings page's groups by default.
    pub fn new(glyph: char, title: impl Into<String>) -> Self {
        Self {
            glyph,
            title: title.into(),
            text: None,
            action: None,
            compact: false,
            width: crate::form::DEFAULT_WIDTH,
        }
    }

    /// A line or two under the title: why it's empty, or what to do.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.text = (!text.is_empty()).then_some(text);
        self
    }

    /// A button under the text, such as Try Again.
    pub fn action(mut self, action: impl Widget + 'static) -> Self {
        self.action = Some(boxed(action));
        self
    }

    /// Smaller, to sit among a page's groups rather than fill the page.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// As wide as this.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    fn glyph_size(&self) -> f32 {
        if self.compact { px(40.0) } else { px(64.0) }
    }

    fn title_font(&self) -> TextStyle {
        TextStyle {
            size: if self.compact { 13.0 } else { 15.0 },
            weight: 700,
            ..sys::r#type::label()
        }
    }

    fn margin(&self) -> f32 {
        if self.compact { px(12.0) } else { px(36.0) }
    }

    fn text_lines(&self) -> Vec<String> {
        self.text
            .as_ref()
            .map(|text| {
                let room = (self.width - px(48.0)).max(120.0);
                wrapped_lines(text, sys::r#type::caption(), room, 4)
            })
            .unwrap_or_default()
    }

    /// How tall it is.
    pub fn height(&self) -> f32 {
        let gap = px(8.0);
        let lines = self.text_lines().len() as f32;
        let mut height =
            self.margin() * 2.0 + self.glyph_size() + gap + text_height(self.title_font());
        if lines > 0.0 {
            height += px(4.0) + lines * text_height(sys::r#type::caption());
        }
        if self.action.is_some() {
            height += px(12.0) + style::control_height();
        }
        height
    }

    /// The glyph, title, text and button, centred.
    pub fn build(self) -> Rectangle {
        let height = self.height();
        let faint = with_alpha(style::label_primary(), 0.55);
        let lines = self.text_lines();
        let glyph = icon(self.glyph, self.glyph_size(), faint);
        let title = Text::new(self.title.clone())
            .style(self.title_font())
            .color(style::label_primary());

        let mut words: Vec<Box<dyn Widget>> = vec![boxed(title)];
        if !lines.is_empty() {
            let text: Vec<Box<dyn Widget>> = lines
                .into_iter()
                .map(|line| {
                    boxed(
                        Text::new(line)
                            .style(sys::r#type::caption())
                            .color(style::label_secondary()),
                    )
                })
                .collect();
            words.push(boxed(Column::new(text).align(Center)));
        }
        let mut parts: Vec<Box<dyn Widget>> = vec![
            boxed(glyph),
            boxed(Column::new(words).gap(px(4.0)).align(Center)),
        ];
        if let Some(action) = self.action {
            parts.push(boxed(
                Rectangle::new()
                    .width(Size::Parent)
                    .height(style::control_height() + px(4.0))
                    .align_child(Center, amane::End)
                    .child(Column::new(vec![action])),
            ));
        }

        Rectangle::new()
            .width(self.width)
            .height(height)
            .align_child(Center, Center)
            .child(Column::new(parts).gap(px(8.0)).align(Center))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_parts_add_to_its_height() {
        let bare = StatusPage::new('\u{e7f4}', "No notifications yet");
        let said = StatusPage::new('\u{e7f4}', "No notifications yet")
            .text("Apps show up here once they've sent one.");
        let compact = StatusPage::new('\u{e7f4}', "No notifications yet").compact();
        assert!(said.height() > bare.height());
        assert!(compact.height() < bare.height());
        let _ = said
            .action(Rectangle::new().width(80.0).height(24.0))
            .build();
    }
}
