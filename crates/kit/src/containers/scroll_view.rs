//! A clipped viewport onto content longer than it, scrolled along one
//! axis, with a thin scrollbar while the content overflows.
//!
//! It scrolls by a negative padding inside the clip, as amane's
//! `ScrollArea` does: the content really moves, so the clip keeps just
//! what's in view and its hover and click areas stay where they're drawn.
//! Where it's scrolled to is the caller's (a [`Kinetic`], usually), so it
//! can follow fingers, coast and ease.
//!
//! [`Kinetic`]: crate::kinetic::Kinetic

use std::time::{Duration, Instant};

use amane::{Animation, Center, Color, Padding, Rectangle, Scroll, Service, Stack, Start, Widget};

use crate::kinetic::Axis;
use crate::layout::boxed;
use crate::material::{self, Level};
use crate::motion;

// TODO(token): `sys.scrollbar.*` once Penpot has them (manual §1.1).
/// How wide the scrollbar is, and how short its thumb gets.
const SCROLLBAR: f32 = 6.0;
const THUMB_MIN: f32 = 32.0;

/// A clipped viewport scrolled along one axis by the caller, with an optional scrollbar.
pub struct ScrollView {
    axis: Axis,
    width: f32,
    height: f32,
    // how long the content is along the axis
    content: f32,
    offset: f32,
    // how much the scrollbar shows, 0 … 1; none draws none
    scrollbar: Option<f32>,
    on_scroll: Option<Box<dyn Fn(Scroll)>>,
}

impl ScrollView {
    /// A `width` by `height` viewport scrolling along `axis`.
    pub fn new(axis: Axis, width: f32, height: f32) -> Self {
        Self {
            axis,
            width,
            height,
            content: 0.0,
            offset: 0.0,
            scrollbar: None,
            on_scroll: None,
        }
    }

    /// How long the content is along the axis.
    pub fn content(mut self, length: f32) -> Self {
        self.content = length;
        self
    }

    /// How far it's scrolled, in pixels; past the ends too, for a rubber
    /// band.
    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }

    /// Draws a scrollbar (vertical only) at `opacity` while the content
    /// overflows.
    pub fn scrollbar(mut self, opacity: f32) -> Self {
        self.scrollbar = Some(opacity);
        self
    }

    /// Runs with each wheel or touchpad scroll over the viewport; where that moves the content
    /// is the caller's.
    pub fn on_scroll(mut self, scroll: impl Fn(Scroll) + 'static) -> Self {
        self.on_scroll = Some(Box::new(scroll));
        self
    }

    /// How far the content can scroll.
    pub fn max(&self) -> f32 {
        (self.content - self.length()).max(0.0)
    }

    fn length(&self) -> f32 {
        match self.axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    /// The viewport with `child` in it, moved by the offset.
    pub fn build(self, child: impl Widget + 'static) -> Rectangle {
        let max = self.max();
        let (padding, content) = match self.axis {
            Axis::Horizontal => (
                crate::layout::pad_x(-self.offset, 0.0),
                Rectangle::new()
                    .width(self.content.max(self.width))
                    .height(self.height)
                    .align_child(Start, Center),
            ),
            Axis::Vertical => (
                Padding {
                    top: -self.offset,
                    right: 0.0,
                    bottom: 0.0,
                    left: 0.0,
                },
                Rectangle::new()
                    .width(self.width)
                    .height(self.content.max(self.height))
                    .align_child(Start, Start),
            ),
        };

        let mut viewport = Rectangle::new()
            .width(self.width)
            .height(self.height)
            .fill(Color::TRANSPARENT)
            .clip()
            .padding(padding)
            .align_child(Start, Start)
            .child(content.fill(Color::TRANSPARENT).child(child));
        if let Some(scroll) = self.on_scroll {
            viewport = viewport.on_scroll(scroll);
        }

        let opacity = match self.scrollbar {
            Some(opacity) if max > 0.0 && opacity > 0.001 && self.axis == Axis::Vertical => opacity,
            _ => return viewport,
        };
        let thumb_height = (self.height * self.height / self.content).max(THUMB_MIN);
        let along = (self.offset / max).clamp(0.0, 1.0);
        let thumb = Rectangle::new()
            .width(SCROLLBAR)
            .height(thumb_height)
            .radius(SCROLLBAR / 2.0)
            .fill(material::label(Level::Secondary))
            .opacity(opacity)
            .translate(self.width - SCROLLBAR, along * (self.height - thumb_height));
        Rectangle::new()
            .width(self.width)
            .height(self.height)
            .fill(Color::TRANSPARENT)
            .align_child(Start, Start)
            .child(Stack::new(vec![boxed(viewport), boxed(thumb)]))
    }
}

/// How much a scrollbar shows: fully while its content moves and for a moment after, then
/// fading out. Keep one per scrolled view (a `thread_local!` `RefCell` beside its other
/// state), tell it when the content [`moved`](Self::moved), and draw the bar at
/// [`opacity`](Self::opacity).
pub struct ScrollbarFade {
    scrolled: Option<Instant>,
    due: Option<Instant>,
    shown: Animation,
}

impl ScrollbarFade {
    /// How long the bar stays after the content stops.
    pub const IDLE: Duration = Duration::from_millis(800);

    /// Hidden, with nothing scrolled yet.
    pub fn new() -> Self {
        Self {
            scrolled: None,
            due: None,
            shown: motion::fade(0.0, motion::FADE),
        }
    }

    /// The content moved just now.
    pub fn moved(&mut self) {
        self.scrolled = Some(Instant::now());
    }

    /// Hidden again at once, as when the view closes.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// How much the bar shows this frame, 0 to 1. While it shows, a timer asks the windows
    /// that read `M` for the frame that starts its fade.
    pub fn opacity<M: Service>(&mut self) -> f32 {
        let now = Instant::now();
        let showing = self
            .scrolled
            .is_some_and(|at| now.duration_since(at) < Self::IDLE);
        if let Some(at) = self.scrolled.filter(|_| showing)
            && self.due.is_none_or(|due| now >= due)
        {
            let due = at + Self::IDLE;
            self.due = Some(due);
            motion::redraw_after::<M>(due.saturating_duration_since(now));
        }
        self.shown.to(if showing { 1.0 } else { 0.0 });
        self.shown.value().clamp(0.0, 1.0)
    }
}

impl Default for ScrollbarFade {
    fn default() -> Self {
        Self::new()
    }
}
