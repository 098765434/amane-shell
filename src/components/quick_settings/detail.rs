//! Detail page — what a tile opens: GNOME's submenu, drawn as a macOS
//! Control Center page that slides over the grid.
//!
//! A back chevron and the setting's name (and what it's doing), an optional
//! switch for it, a hairline, then its body — networks, devices, choices —
//! and, pinned at the bottom, an optional link to the app that has the rest
//! of its settings. Every page is as tall as the grid, so the card never
//! changes size while a page slides in (see the module docs): the body is
//! told how much room it has.

use std::rc::Rc;

use amane::{Center, Color, Column, Parent, Rectangle, Size, Start, Widget};

use super::back;
use super::grid::{Canvas, Metrics};
use crate::components::app_bar::TopAppBar;
use crate::components::buttons::Switch;
use crate::components::divider::Divider;
use crate::components::icons::ICON_CHEVRON_RIGHT;
use crate::components::layout::{boxed, icon};
use crate::components::list::{self, ListItem};
use crate::tokens::{quick, sys};

/// How wide a page's content is: the grid's width less the sheet's padding
/// either side.
pub(super) fn page_width() -> f32 {
    let px = Metrics::tokens();
    (px.extent(Canvas::tokens().cols) - quick::tile::pad() * 2.0).max(1.0)
}

/// The header's switch: whether it's on, and what flipping it asks for.
type HeaderSwitch = (bool, Rc<dyn Fn(bool)>);

/// The body, built for the height it gets.
type Body = Box<dyn FnOnce(f32) -> Vec<Box<dyn Widget>>>;

pub struct DetailPage {
    id: &'static str,
    title: String,
    subtitle: Option<String>,
    switch: Option<HeaderSwitch>,
    body: Option<Body>,
    footer: Option<(String, Rc<dyn Fn()>)>,
}

impl DetailPage {
    /// `id` prefixes its controls' ids, so it must be unique.
    pub fn new(id: &'static str, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            subtitle: None,
            switch: None,
            body: None,
            footer: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A switch at the header's end for the setting itself.
    pub fn switch(mut self, on: bool, toggle: impl Fn(bool) + 'static) -> Self {
        self.switch = Some((on, Rc::new(toggle)));
        self
    }

    /// What the page shows, built for the height it has (it's clipped to
    /// that), `gap.l` apart.
    pub fn body(mut self, body: impl FnOnce(f32) -> Vec<Box<dyn Widget>> + 'static) -> Self {
        self.body = Some(Box::new(body));
        self
    }

    /// A last row that opens the app with the rest of the settings.
    pub fn footer(mut self, label: impl Into<String>, open: impl Fn() + 'static) -> Self {
        self.footer = Some((label.into(), Rc::new(open)));
        self
    }

    /// The page, `height` tall.
    pub fn build(self, height: f32) -> Column {
        let row = sys::size::row::height();
        let gap = sys::space::gap::s();

        // the title has what the back chevron and the switch leave
        let width = page_width();
        let gap_s = sys::space::gap::s();
        let mut text_w = width - sys::size::control::height() - gap_s;
        if self.switch.is_some() {
            text_w -= sys::size::switch::width() + gap_s;
        }
        let mut bar = TopAppBar::new(self.title)
            .back(format!("{}.back", self.id), back)
            .elide_to(text_w.max(1.0));
        if let Some(subtitle) = self.subtitle {
            bar = bar.subtitle(subtitle);
        }
        if let Some((on, toggle)) = self.switch {
            bar = bar.actions(vec![boxed(
                Switch::new(format!("{}.switch", self.id), on)
                    .on_toggle(move |on| toggle(on))
                    .build(),
            )]);
        }
        let header = Rectangle::new()
            .width(Parent)
            .height(Size::Fixed(row))
            .fill(Color::TRANSPARENT)
            .align_child(Start, Center)
            .child(bar.build());

        let footer_h = if self.footer.is_some() { row } else { 0.0 };
        let body_h = (height - row - Divider::height() - gap - footer_h).max(0.0);
        let body = list::with_row_width(width, || {
            self.body.map_or_else(Vec::new, |body| body(body_h))
        });
        let body = Rectangle::new()
            .width(Parent)
            .height(Size::Fixed(body_h))
            .fill(Color::TRANSPARENT)
            .align_child(Start, Start)
            .clip()
            .child(Column::new(body).gap(sys::space::gap::l()).width(Parent));

        let mut rows = vec![boxed(header), boxed(Divider::build()), boxed(body)];
        if let Some((label, open)) = self.footer {
            rows.push(boxed(
                ListItem::new(format!("{}.footer", self.id), label)
                    .trailing(icon(
                        ICON_CHEVRON_RIGHT,
                        sys::size::icon::m(),
                        sys::color::content::secondary(),
                    ))
                    .on_press(move || open())
                    .build(),
            ));
        }

        // the divider carries its own room; the body keeps `gap` from it
        Column::new(rows)
            .gap(gap / 3.0)
            .width(Parent)
            .height(Size::Fixed(height))
    }
}
