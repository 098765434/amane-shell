//! Segmented button — a set of related options on one track.
//!
//! Modelled on Material 3's segmented buttons (2–5 equal segments, a label
//! and optional leading icon each, single- or multi-select, disabled
//! segments), but drawn in the shell's language rather than M3's outlined
//! one: a `surface.raised` track with the caller's corner radius, and each
//! selected segment an inset accent thumb whose corners stay concentric with
//! the track's.

use std::rc::Rc;

use amane::{Button, Center, Color, Pointer, Radius, Rectangle, Row, Size, Text, Widget};

use crate::components::taskbar::{boxed, icon, pad};
use crate::tokens::{Styled, sys};

/// M3's disabled-content opacity; becomes `sys.state.disabled` once the
/// token exists in Penpot.
const DISABLED_OPACITY: f32 = 0.38;

/// One option: a label, optionally led by a Material Symbols glyph.
pub struct Segment {
    label: String,
    icon: Option<char>,
    disabled: bool,
}

impl Segment {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Dimmed and unclickable.
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// The segments, which of them are selected, and what a click asks for.
/// Like the panel controls it only reflects state: `on_select(index, on)`
/// asks for a change, and the next frame redraws from the service.
pub struct SegmentedButton {
    segments: Vec<Segment>,
    selected: Vec<bool>,
    multi: bool,
    radius: Radius,
    width: f32,
    on_select: Rc<dyn Fn(usize, bool)>,
}

impl SegmentedButton {
    /// Single-select: at most one segment selected; clicking one asks for
    /// `(index, true)`.
    pub fn new(segments: Vec<Segment>, selected: Option<usize>) -> Self {
        let selected = (0..segments.len()).map(|i| Some(i) == selected).collect();
        Self::with(segments, selected, false)
    }

    /// Multi-select: clicking a segment asks for the other state.
    pub fn multi(segments: Vec<Segment>, mut selected: Vec<bool>) -> Self {
        selected.resize(segments.len(), false);
        Self::with(segments, selected, true)
    }

    fn with(segments: Vec<Segment>, selected: Vec<bool>, multi: bool) -> Self {
        Self {
            segments,
            selected,
            multi,
            radius: Radius::Fixed(sys::radius::raised()),
            width: sys::size::popover::width() - sys::space::inset::l() * 2.0,
            on_select: Rc::new(|_, _| {}),
        }
    }

    /// The track's corners; the thumbs follow, less the inset.
    pub fn radius(mut self, radius: impl Into<Radius>) -> Self {
        self.radius = radius.into();
        self
    }

    /// The whole button's width, split equally between the segments.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_select(mut self, select: impl Fn(usize, bool) + 'static) -> Self {
        self.on_select = Rc::new(select);
        self
    }

    pub fn build(self) -> Rectangle {
        // segments are control-height; the track grows around them by the inset
        let inset = sys::space::gap::xs();
        let segment_h = sys::size::control::height();
        let height = segment_h + inset * 2.0;
        let count = self.segments.len().max(1) as f32;
        let segment_w = ((self.width - inset * 2.0) / count).max(1.0);
        let thumb = match self.radius {
            Radius::Full => Radius::Full,
            Radius::Fixed(r) => Radius::Fixed((r - inset).max(0.0)),
        };

        let segments: Vec<Box<dyn Widget>> = self
            .segments
            .into_iter()
            .zip(self.selected)
            .enumerate()
            .map(|(index, (segment, selected))| {
                let on_select = self.on_select.clone();
                let on = !(self.multi && selected);
                boxed(segment_view(
                    segment,
                    selected,
                    (segment_w, segment_h),
                    thumb,
                    move || on_select(index, on),
                ))
            })
            .collect();

        Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(height))
            .radius(self.radius)
            .fill(sys::color::surface::raised::bg())
            .padding(pad(inset, inset))
            .align_child(Center, Center)
            .child(Row::new(segments).align(Center))
    }
}

fn segment_view(
    segment: Segment,
    selected: bool,
    (width, height): (f32, f32),
    radius: Radius,
    click: impl Fn() + 'static,
) -> Rectangle {
    let (fill, color) = if selected {
        (sys::color::accent::bg(), sys::color::surface::panel::fg())
    } else {
        (Color::TRANSPARENT, sys::color::content::secondary())
    };

    let mut content: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(glyph) = segment.icon {
        content.push(boxed(icon(glyph, sys::size::icon::s(), color)));
    }
    content.push(boxed(
        Text::new(segment.label)
            .style(sys::r#type::caption())
            .color(color),
    ));

    let base = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(radius)
        .fill(fill)
        .align_child(Center, Center)
        .child(Row::new(content).gap(sys::space::gap::xs()).align(Center));

    if segment.disabled {
        return base.opacity(DISABLED_OPACITY);
    }

    base.cursor(Pointer).on_click(move |button| {
        if button == Button::Left {
            click();
        }
    })
}
