//! Top app bar — the title row of a sheet.
//!
//! Modelled on Material 3's small top app bar: a title with an optional
//! subtitle, and trailing action items. A page drilled into from another (Quick Settings' pages)
//! leads with M3's back button too, a chevron as on macOS. Unlike M3's it's
//! no taller than its content and has no fill of its own: it sits at the
//! top of a [`Card`](super::card::Card), in the vibrant labels of the
//! material it's on.

use std::rc::Rc;

use amane::{Center, Column, Row, Start, Text, Widget};

use crate::buttons::IconButton;
use crate::icons::ICON_CHEVRON_LEFT;
use crate::layout::{boxed, clipped_label, split_row};
use crate::material::{Level, label};
use crate::tokens::{Styled, sys};

pub struct TopAppBar {
    title: String,
    subtitle: Option<String>,
    back: Option<(String, Rc<dyn Fn()>)>,
    actions: Vec<Box<dyn Widget>>,
    // the title's and subtitle's width, when they're to be elided
    text_width: Option<f32>,
}

impl TopAppBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            back: None,
            actions: Vec::new(),
            text_width: None,
        }
    }

    /// A back chevron first, which runs `back`. `id` keys its hover, so it
    /// must be unique and stable.
    pub fn back(mut self, id: impl Into<String>, back: impl Fn() + 'static) -> Self {
        self.back = Some((id.into(), Rc::new(back)));
        self
    }

    /// A secondary line under the title.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The controls at the trailing end, `gap.s` apart.
    pub fn actions(mut self, actions: Vec<Box<dyn Widget>>) -> Self {
        self.actions = actions;
        self
    }

    /// Elides the title and subtitle to `width`, for a long network or
    /// device name.
    pub fn elide_to(mut self, width: f32) -> Self {
        self.text_width = Some(width);
        self
    }

    pub fn build(self) -> Row {
        let color = label(Level::Primary);

        let line = |text: String, style, color| -> Box<dyn Widget> {
            match self.text_width {
                Some(width) => boxed(clipped_label(text, style, color, width, Start)),
                None => boxed(Text::new(text).style(style).color(color)),
            }
        };
        let title = line(self.title, sys::r#type::title(), color);
        let title: Box<dyn Widget> = match self.subtitle {
            Some(subtitle) => boxed(
                Column::new(vec![
                    title,
                    line(subtitle, sys::r#type::caption(), label(Level::Secondary)),
                ])
                .gap(sys::space::gap::_2xs()),
            ),
            None => title,
        };

        let mut leading = Vec::new();
        if let Some((id, back)) = self.back {
            leading.push(boxed(
                IconButton::new(id, ICON_CHEVRON_LEFT)
                    .color(color)
                    .on_press(move || back())
                    .build(),
            ));
        }
        leading.push(title);

        split_row(
            Row::new(leading).gap(sys::space::gap::s()).align(Center),
            self.actions,
        )
    }
}
