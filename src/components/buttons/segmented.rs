//! Segmented button — a set of related options on one track.
//!
//! Modelled on Material 3's segmented buttons (2–5 equal segments, a label
//! and optional leading icon each, single- or multi-select, disabled
//! segments), but drawn in the shell's language rather than M3's outlined
//! one: a `surface.raised` track with the caller's corner radius, and an
//! inset accent thumb whose corners stay concentric with the track's.
//!
//! It moves like iOS's segmented control: in single-select one thumb springs
//! across to the new segment, each label brightening as the thumb passes
//! under it, and in multi-select each segment's thumb fades in and out. A
//! click moves the thumb at once; if the service doesn't follow within
//! [`PENDING_TIMEOUT`], it goes back to what the service says.
//!
//! [`PENDING_TIMEOUT`]: super::PENDING_TIMEOUT

use std::rc::Rc;
use std::time::Instant;

use amane::{
    Animation, Blend, Center, Color, Radius, Rectangle, Row, Size, Stack, Widget, request_frame,
};

use super::{Actions, Pending, interactive, label_row, state_fill, with_existing, with_state};
use crate::components::color::faded;
use crate::components::layout::{boxed, inner_width, pad};
use crate::components::motion::{self, FADE};
use crate::tokens::sys;

/// One button's motion, kept across rebuilds under its id.
struct Motion {
    // the thumb's position in segments (0.0 = the first), single-select
    thumb: Animation,
    // each segment's thumb opacity, multi-select
    fades: Vec<Animation>,
    pending: Vec<Pending<bool>>,
}

impl Motion {
    // at rest on the current selection, so opening a panel doesn't animate
    fn new(selected: &[bool]) -> Self {
        let first = selected.iter().position(|&on| on).unwrap_or(0);
        Self {
            thumb: motion::spring(first as f32),
            fades: selected
                .iter()
                .map(|&on| motion::fade(if on { 1.0 } else { 0.0 }, FADE))
                .collect(),
            pending: vec![Pending::NONE; selected.len()],
        }
    }
}

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
/// Like the panel controls it reflects state: `on_select(index, on)` asks
/// for a change, and the service's answer is what's drawn once it arrives.
/// `id` keys the motion, so it must be unique and stable, like a
/// `ScrollArea`'s.
pub struct SegmentedButton {
    id: String,
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
    pub fn new(id: impl Into<String>, segments: Vec<Segment>, selected: Option<usize>) -> Self {
        let selected = (0..segments.len()).map(|i| Some(i) == selected).collect();
        Self::with(id, segments, selected, false)
    }

    /// Multi-select: clicking a segment asks for the other state.
    pub fn multi(id: impl Into<String>, segments: Vec<Segment>, mut selected: Vec<bool>) -> Self {
        selected.resize(segments.len(), false);
        Self::with(id, segments, selected, true)
    }

    fn with(
        id: impl Into<String>,
        segments: Vec<Segment>,
        selected: Vec<bool>,
        multi: bool,
    ) -> Self {
        Self {
            id: id.into(),
            segments,
            selected,
            multi,
            radius: Radius::Fixed(sys::radius::raised()),
            width: inner_width(sys::size::popover::width()),
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
        let count = self.segments.len();
        let segment_w = ((self.width - inset * 2.0) / count.max(1) as f32).max(1.0);
        let thumb_radius = match self.radius {
            Radius::Full => Radius::Full,
            Radius::Fixed(r) => Radius::Fixed((r - inset).max(0.0)),
        };

        let (shown, thumb, fades) = self.animate();

        // how selected each segment looks right now, from 0 to 1
        let lit: Vec<f32> = match thumb {
            Some(x) => (0..count)
                .map(|i| (1.0 - (x - i as f32).abs()).clamp(0.0, 1.0))
                .collect(),
            None if self.multi => fades.clone(),
            None => vec![0.0; count],
        };

        let id = self.id.clone();
        let multi = self.multi;
        let segments: Vec<Box<dyn Widget>> = self
            .segments
            .into_iter()
            .enumerate()
            .map(|(index, segment)| {
                let on_select = self.on_select.clone();
                let on = !(multi && shown[index]);
                let fill = match multi {
                    true => faded(sys::color::accent::bg(), fades[index]),
                    false => Color::TRANSPARENT,
                };
                let button = id.clone();
                boxed(segment_view(
                    format!("{id}.{index}"),
                    segment,
                    lit[index],
                    fill,
                    (segment_w, segment_h),
                    thumb_radius,
                    move || {
                        clicked(&button, index, on, multi);
                        on_select(index, on);
                    },
                ))
            })
            .collect();

        let mut layers: Vec<Box<dyn Widget>> = Vec::new();
        if let Some(x) = thumb {
            layers.push(boxed(
                Rectangle::new()
                    .width(Size::Fixed(segment_w))
                    .height(Size::Fixed(segment_h))
                    .radius(thumb_radius)
                    .fill(sys::color::accent::bg())
                    .translate(x * segment_w, 0.0),
            ));
        }
        // last, so the segments take the clicks
        layers.push(boxed(Row::new(segments).align(Center)));

        Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(height))
            .radius(self.radius)
            .fill(sys::color::surface::raised::bg())
            .padding(pad(inset, inset))
            .align_child(Center, Center)
            .child(Stack::new(layers))
    }

    /// Steps this button's motion towards what should show: the selection
    /// with any pending clicks laid over it. Gives that selection, the
    /// sliding thumb's position (single-select, when something's selected)
    /// and each segment's fade (multi-select).
    fn animate(&self) -> (Vec<bool>, Option<f32>, Vec<f32>) {
        let now = Instant::now();
        let count = self.selected.len();

        let init = || Motion::new(&self.selected);
        with_state(&self.id, init, |motion: &mut Motion| {
            if motion.pending.len() != count {
                *motion = Motion::new(&self.selected);
            }

            let shown: Vec<bool> = (0..count)
                .map(|i| motion.pending[i].shown(self.selected[i], now))
                .collect();

            if self.multi {
                let fades = motion
                    .fades
                    .iter_mut()
                    .zip(&shown)
                    .map(|(fade, &on)| {
                        fade.to(if on { 1.0 } else { 0.0 });
                        fade.value()
                    })
                    .collect();
                return (shown, None, fades);
            }

            let thumb = shown.iter().position(|&on| on).map(|target| {
                motion.thumb.to(target as f32);
                // an overshoot past either end stays inside the track
                motion.thumb.value().clamp(0.0, count as f32 - 1.0)
            });
            (shown, thumb, vec![0.0; count])
        })
    }
}

/// Shows a click before the service answers it.
fn clicked(id: &str, index: usize, on: bool, multi: bool) {
    let now = Instant::now();

    with_existing(id, |motion: &mut Motion| {
        if multi {
            motion.pending[index].set(on, now);
        } else {
            for (i, pending) in motion.pending.iter_mut().enumerate() {
                pending.set(i == index, now);
            }
        }
    });

    request_frame();
}

fn segment_view(
    id: String,
    segment: Segment,
    lit: f32,
    fill: Color,
    (width, height): (f32, f32),
    radius: Radius,
    click: impl Fn() + 'static,
) -> Rectangle {
    let color = Color::blend(
        sys::color::content::secondary(),
        sys::color::surface::panel::fg(),
        lit,
    );

    let content = label_row(
        segment.icon,
        sys::size::icon::s(),
        segment.label,
        sys::r#type::caption(),
        color,
    );

    // the hover layer sits on the segment, over the thumb when it's there
    let base = Rectangle::new()
        .width(Size::Fixed(width))
        .height(Size::Fixed(height))
        .radius(radius)
        .fill(state_fill(&id, fill, segment.disabled))
        .align_child(Center, Center)
        .child(content);

    let actions = Actions {
        press: Some(Rc::new(click)),
        secondary: None,
    };
    interactive(base, id, segment.disabled, actions)
}
