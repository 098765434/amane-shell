//! Spinner and progress bar — something under way: an
//! indeterminate spinner of fading spokes, and a thin determinate track.
//!
//! The spinner turns a step about ten times a second, and only while a
//! window shows it: each build asks for one more step, so a page that stops
//! showing it lets its window rest. Where motion is reduced its spokes stay
//! put and the whole spinner breathes instead.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use amane::{Color, Rectangle, Size, Stack, Widget};

use crate::color::with_alpha;
use crate::layout::boxed;
use crate::style;

/// How many spokes there are, and so how many steps a turn has.
const SPOKES: usize = 8;
/// How long one step lasts.
const STEP: Duration = Duration::from_millis(100);

crate::marker! {
    /// A spinner took a step.
    Tick
}

/// Whether a step is already on its way.
static PENDING: AtomicBool = AtomicBool::new(false);

/// The step every spinner is at: they turn together.
fn step() -> usize {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    (start.elapsed().as_millis() / STEP.as_millis()) as usize % SPOKES
}

/// Asks for the next step, once however many spinners are showing.
fn next_step() {
    Tick::watch();
    if !PENDING.swap(true, Ordering::AcqRel) {
        std::thread::spawn(|| {
            std::thread::sleep(STEP);
            PENDING.store(false, Ordering::Release);
            Tick::changed();
        });
    }
}

/// The opacity of spoke `index` at `step`: the newest is solid and the
/// ones behind it fade.
fn spoke_alpha(index: usize, step: usize) -> f32 {
    let behind = (step + SPOKES - index) % SPOKES;
    1.0 - behind as f32 * (0.75 / (SPOKES - 1) as f32)
}

/// The breath a still spinner takes where motion is reduced.
fn breath(step: usize) -> f32 {
    let phase = step as f32 / SPOKES as f32 * std::f32::consts::TAU;
    0.55 + 0.35 * (0.5 + 0.5 * phase.cos())
}

/// An indeterminate spinner, `size` across (16 in a row, 24 alone).
pub struct Spinner {
    size: f32,
    color: Option<Color>,
}

impl Spinner {
    /// A spinner `size` logical pixels across, in the secondary label's color.
    pub fn new(size: f32) -> Self {
        Self { size, color: None }
    }

    /// The spokes' colour; the secondary label's by default.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// The spinner at its current step; building it asks for the next step.
    pub fn build(self) -> Rectangle {
        next_step();
        let step = step();
        let reduced = crate::theme::reduced_motion();
        let color = self.color.unwrap_or_else(style::label_secondary);
        let size = self.size;
        let length = size * 0.28;
        let thickness = (size * 0.1).max(1.5);

        let spokes: Vec<Box<dyn Widget>> = (0..SPOKES)
            .map(|index| {
                let alpha = if reduced {
                    0.35 + 0.65 * (index as f32 / SPOKES as f32)
                } else {
                    spoke_alpha(index, step)
                };
                // a spoke from the rim inwards, in a frame that turns
                boxed(
                    Rectangle::new()
                        .width(Size::Fixed(size))
                        .height(Size::Fixed(size))
                        .fill(Color::TRANSPARENT)
                        .rotate(index as f32 * 360.0 / SPOKES as f32)
                        .align_child(amane::Center, amane::Start)
                        .child(
                            Rectangle::new()
                                .width(Size::Fixed(thickness))
                                .height(Size::Fixed(length))
                                .radius(thickness / 2.0)
                                .fill(with_alpha(color, alpha)),
                        ),
                )
            })
            .collect();

        let opacity = if reduced { breath(step) } else { 1.0 };
        Rectangle::new()
            .width(Size::Fixed(size))
            .height(Size::Fixed(size))
            .fill(Color::TRANSPARENT)
            .opacity(opacity)
            .child(Stack::new(spokes))
    }
}

/// A determinate progress bar: a thin rounded track, filled in the accent
/// up to `fraction`.
pub struct ProgressBar {
    fraction: f32,
    width: f32,
}

impl ProgressBar {
    /// A bar `width` logical pixels long, filled to `fraction` (kept to 0..1).
    pub fn new(fraction: f32, width: f32) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
            width,
        }
    }

    /// The track, 6 logical pixels tall, with its fill.
    pub fn build(self) -> Rectangle {
        const HEIGHT: f32 = 6.0;
        let filled =
            (self.width * self.fraction).max(if self.fraction > 0.0 { HEIGHT } else { 0.0 });
        Rectangle::new()
            .width(Size::Fixed(self.width))
            .height(Size::Fixed(HEIGHT))
            .radius(HEIGHT / 2.0)
            .fill(style::divider())
            .align_child(amane::Start, amane::Center)
            .child(
                Rectangle::new()
                    .width(Size::Fixed(filled))
                    .height(Size::Fixed(HEIGHT))
                    .radius(HEIGHT / 2.0)
                    .fill(style::tint()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_spoke_is_solid_and_the_rest_fade() {
        assert_eq!(spoke_alpha(3, 3), 1.0);
        assert!(spoke_alpha(2, 3) > spoke_alpha(1, 3));
        assert!(spoke_alpha(4, 3) > 0.2);
        assert!((0.0..=1.0).contains(&breath(5)));
    }
}
