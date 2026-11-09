//! Detail page — what a tile opens: GNOME's submenu, drawn as a page
//! that slides over the grid.
//!
//! A back chevron (none on a page opened on its own, from its status
//! icon) and the setting's name (and what it's doing), an optional
//! switch for it, a hairline, then its body — networks, devices, choices —
//! and, pinned at the bottom, an optional "Show in Status Bar" switch for the
//! setting's status icon and an optional link to the app that has the rest
//! of its settings. Every page is as tall as the grid, so the card never
//! changes size while a page grows in (see the module docs): the body is
//! told how much room it has.
//!
//! As the page's sheet grows out of its tile, what's on it comes in after:
//! the header once the sheet has room for it, then the body's rows one
//! after another, rising a little. Going back, they go first, the last row
//! before the header, and the sheet shrinks into the tile empty.

use std::rc::Rc;

use amane::{Center, Column, Parent, Rectangle, Stack, Start, Widget};

use super::grid::{Canvas, Metrics};
use super::{ALONE, back, ramp};
use crate::surfaces::quick_settings::grid::FromTokens as _;
use crate::surfaces::status_bar::Indicator;
use crate::tokens::{quick, sys};
use emaki_kit::app_bar::TopAppBar;
use emaki_kit::buttons::Switch;
use emaki_kit::control_size;
use emaki_kit::divider::Divider;
use emaki_kit::layout::boxed;
use emaki_kit::list::{self, ListItem};

/// How wide a page's content is: the grid's width less the sheet's padding
/// either side.
pub(super) fn page_width() -> f32 {
    let px = Metrics::tokens();
    (px.extent(Canvas::tokens().cols) - quick::tile::pad() * 2.0).max(1.0)
}

// `quick.page.reveal.*`: where in the sheet's growth (0 … 1) the header
// comes in (`header-start` to `header-end`), and the body's first row
// (`body`); each row after that comes `step` later, over `span`, rising
// `rise` into place.
use quick::page::reveal;

/// `widget` as far in as `shown` (0 … 1): fading in, rising into place.
fn revealed(widget: impl Widget + 'static, shown: f32) -> Rectangle {
    let height = widget.height();
    Rectangle::new()
        .width(Parent)
        .height(height)
        .opacity(shown)
        .translate(0.0, (1.0 - shown) * reveal::rise())
        .child(widget)
}

/// How far in the `index`th row after the header is at `reveal`.
fn row_shown(reveal: f32, index: usize) -> f32 {
    let from = (reveal::body() + reveal::step() * index as f32).min(1.0 - reveal::span());
    ramp(reveal, from, from + reveal::span())
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
    status_bar: Option<Indicator>,
    footer: Option<(String, Rc<dyn Fn()>)>,
    back: Option<Rc<dyn Fn()>>,
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
            status_bar: None,
            footer: None,
            back: None,
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

    /// A row near the bottom that shows or hides `indicator` in the bar.
    pub fn status_bar(mut self, indicator: Indicator) -> Self {
        self.status_bar = Some(indicator);
        self
    }

    /// A last row that opens the app with the rest of the settings.
    pub fn footer(mut self, label: impl Into<String>, open: impl Fn() + 'static) -> Self {
        self.footer = Some((label.into(), Rc::new(open)));
        self
    }

    /// What the back chevron does, for a page inside a page; back to the
    /// grid by default.
    pub fn on_back(mut self, back: impl Fn() + 'static) -> Self {
        self.back = Some(Rc::new(back));
        self
    }

    /// The page, `height` tall, `reveal` of the way in (see the module docs).
    pub fn build(self, height: f32, reveal: f32) -> Column {
        let row = sys::size::row::height();
        let gap = sys::space::gap::s();

        // a page opened on its own has no grid to go back to; a page inside
        // a page still goes back to the one it's in
        let on_back = self
            .back
            .or_else(|| (!ALONE.get()).then(|| Rc::new(back) as Rc<dyn Fn()>));

        // the title has what the back chevron and the switch leave
        let width = page_width();
        let gap_s = sys::space::gap::s();
        let mut text_w = width;
        if on_back.is_some() {
            text_w -= sys::size::control::height() + gap_s;
        }
        if self.switch.is_some() {
            text_w -= control_size::current().switch().width + gap_s;
        }
        let mut bar = TopAppBar::new(self.title).elide_to(text_w.max(1.0));
        if let Some(on_back) = on_back {
            bar = bar.back(format!("{}.back", self.id), move || on_back());
        }
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
            .height(row)
            .align_child(Start, Center)
            .child(bar.build());

        let pinned = usize::from(self.status_bar.is_some()) + usize::from(self.footer.is_some());
        let footer_h = row * pinned as f32;
        let body_h = (height - row - Divider::height() - gap - footer_h).max(0.0);
        let body: Vec<Box<dyn Widget>> = list::with_row_width(width, || {
            self.body.map_or_else(Vec::new, |body| body(body_h))
        })
        .into_iter()
        .enumerate()
        .map(|(index, row)| boxed(revealed(Stack::new(vec![row]), row_shown(reveal, index))))
        .collect();
        let rows_in = body.len();
        let body = Rectangle::new()
            .width(Parent)
            .height(body_h)
            .align_child(Start, Start)
            .clip()
            .child(Column::new(body).gap(sys::space::gap::l()).width(Parent));

        let header_shown = ramp(reveal, reveal::header_start(), reveal::header_end());
        let mut rows = vec![
            boxed(revealed(header, header_shown)),
            boxed(revealed(Divider::build(), header_shown)),
            boxed(body),
        ];
        // the pinned rows come in after the body's
        let pinned_shown = row_shown(reveal, rows_in);
        if let Some(indicator) = self.status_bar {
            let shown = indicator.shown();
            rows.push(boxed(revealed(
                ListItem::new(format!("{}.status-bar", self.id), "Show in Status Bar")
                    .trailing(
                        Switch::new(format!("{}.status-bar.switch", self.id), shown)
                            .on_toggle(move |on| indicator.set_shown(on))
                            .build(),
                    )
                    .on_press(move || indicator.set_shown(!shown))
                    .build(),
                pinned_shown,
            )));
        }
        if let Some((label, open)) = self.footer {
            rows.push(boxed(revealed(
                ListItem::new(format!("{}.footer", self.id), label)
                    .trailing(list::chevron())
                    .on_press(move || open())
                    .build(),
                pinned_shown,
            )));
        }

        // the divider carries its own room; the body keeps `gap` from it
        Column::new(rows)
            .gap(gap / 3.0)
            .width(Parent)
            .height(height)
    }
}
