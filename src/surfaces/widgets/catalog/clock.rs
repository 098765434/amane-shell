//! The clock: the time in pen strokes, each digit that changes erased
//! along its line and the next drawn in, like the stroke clock mockup.
//!
//! Every digit and colon is one canvas, so the face is one vello run that
//! amane draws only in its own box. Between changes the frames are the
//! same, so amane lays the kept result again; a change asks for frames
//! only until its strokes arrive. Minutes come from the bar's `Clock`,
//! seconds from one frame asked for as each second turns over.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use amane::{Canvas, Cap, Circle, Color, Easing, Join, Service, Shape, Size, Widget};
use chrono::{Local, Timelike};

use super::digits::{self, HEIGHT, WIDTH};
use super::{Face, accent, caption_style, line, place};
use crate::components::layout::{text_height, text_width};
use crate::components::motion;
use crate::surfaces::widgets::kind::WidgetSize;
use crate::services::launcher::clock::Clock;
use crate::theme;

amane_kit::marker! {
    /// A second turned over, for clocks that show seconds.
    pub ClockSecond
}

/// How long one digit takes to change: the old one erased over the
/// first part, the new one drawn in starting near the erase's end.
const CHANGE: Duration = Duration::from_millis(900);
const ERASE: f32 = 0.45;
const DRAW_FROM: f32 = 0.8;
/// How much later each further digit that changes at once starts, from
/// the right, like a carry.
const STAGGER: f32 = 0.12;

/// The room between digits, and a colon's width, in the digits' units.
const GAP: f32 = 4.0;
const COLON: f32 = 20.0;
/// The seconds under a stacked clock, against its hours and minutes.
const SECONDS_SCALE: f32 = 0.6;

/// A beat past each second turning over, so the frame finds it turned.
const TICK: Duration = Duration::from_millis(20);

/// The line's thickness in the digits' units, by the `line` setting.
fn thickness(line: Option<&str>) -> f32 {
    match line {
        Some("thin") => 12.0,
        Some("bold") => 22.0,
        _ => 18.0,
    }
}

/// One digit changing: what it showed and since when.
#[derive(Clone, Copy)]
struct Slot {
    shown: u8,
    from: Option<(u8, Instant)>,
}

thread_local! {
    // each clock's digits, by widget id
    static SLOTS: RefCell<HashMap<u32, Vec<Slot>>> = RefCell::new(HashMap::new());
    // the second a frame was last asked for
    static SECOND_DUE: Cell<Option<i64>> = const { Cell::new(None) };
}

/// Where one stroke goes in the content box.
enum Mark {
    Digit {
        slot: usize,
        x: f32,
        y: f32,
        scale: f32,
    },
    Colon {
        x: f32,
        y: f32,
        scale: f32,
    },
}

pub(super) fn face(id: u32, face: &Face) -> Vec<Box<dyn Widget>> {
    let settings = face.settings;
    let seconds = settings.seconds.as_deref() == Some("on");
    let twelve = settings.hours.as_deref() == Some("12");
    let color = settings
        .color
        .as_deref()
        .map_or_else(|| accent(false), Color::from);
    let thickness = thickness(settings.line.as_deref());

    // the minute from the bar's clock, which marks once a minute
    drop(Clock::read());
    let now = Local::now();
    if seconds {
        ClockSecond::watch();
        let due = now.timestamp();
        if SECOND_DUE.replace(Some(due)) != Some(due) {
            let left = 1_000_000_000 - u64::from(now.nanosecond().min(999_999_999));
            motion::redraw_after::<ClockSecond>(Duration::from_nanos(left) + TICK);
        }
    }

    let hour = if twelve {
        (now.hour() + 11) % 12 + 1
    } else {
        now.hour()
    };
    let mut values = vec![
        (hour / 10) as u8,
        (hour % 10) as u8,
        (now.minute() / 10) as u8,
        (now.minute() % 10) as u8,
    ];
    if seconds {
        values.extend([(now.second() / 10) as u8, (now.second() % 10) as u8]);
    }

    let dated = face.size != WidgetSize::Small;
    let date_height = if dated {
        text_height(caption_style()) + 8.0
    } else {
        0.0
    };
    let room = (face.width, face.height - date_height);
    let marks = match face.size {
        WidgetSize::Medium => one_row(values.len(), room),
        _ => stacked(values.len(), room),
    };

    let mut shapes: Vec<Box<dyn Shape>> = Vec::new();
    let strokes = strokes(id, &values);
    for mark in &marks {
        match *mark {
            Mark::Digit { slot, x, y, scale } => {
                let pen = |digit: u8| {
                    digits::digit(digit, x, y, scale)
                        .stroke(thickness * scale, color)
                        .cap(Cap::Round)
                        .join(Join::Round)
                };
                match strokes[slot] {
                    Stroke::Still(digit) => shapes.push(Box::new(pen(digit))),
                    Stroke::Changing {
                        from,
                        to,
                        erased,
                        drawn,
                    } => {
                        if erased < 1.0 {
                            shapes.push(Box::new(pen(from).trim(erased, 1.0)));
                        }
                        if drawn > 0.0 {
                            shapes.push(Box::new(pen(to).trim(0.0, drawn)));
                        }
                    }
                }
            }
            Mark::Colon { x, y, scale } => {
                for dot in [38.0, 70.0] {
                    shapes.push(Box::new(
                        Circle::new()
                            .center(x + COLON / 2.0 * scale, y + dot * scale)
                            .radius(Size::Fixed(thickness * 0.42 * scale))
                            .fill(color),
                    ));
                }
            }
        }
    }

    let mut content = vec![place(
        Canvas::new()
            .width(Size::Fixed(room.0))
            .height(Size::Fixed(room.1))
            .shapes(shapes),
        0.0,
        0.0,
        room.0,
        room.1,
    )];
    if dated {
        let style = caption_style();
        let text = now.format("%a, %B %-d").to_string();
        let width = (text_width(&text, style) + 1.0).min(face.width);
        content.push(line(
            text,
            style,
            color,
            ((face.width - width) / 2.0, face.height - text_height(style)),
            width,
            false,
        ));
    }
    content
}

/// The digits in one row with colons between each pair, as big as fits
/// `room` and centred in it.
fn one_row(count: usize, (width, height): (f32, f32)) -> Vec<Mark> {
    let pairs = count / 2;
    let items = count + pairs - 1;
    let units = count as f32 * WIDTH + (pairs - 1) as f32 * COLON + (items - 1) as f32 * GAP;
    let scale = (width / units).min(height / HEIGHT);
    let mut x = (width - units * scale) / 2.0;
    let y = (height - HEIGHT * scale) / 2.0;
    let mut marks = Vec::new();
    for slot in 0..count {
        if slot > 0 && slot % 2 == 0 {
            marks.push(Mark::Colon { x, y, scale });
            x += (COLON + GAP) * scale;
        }
        marks.push(Mark::Digit { slot, x, y, scale });
        x += (WIDTH + GAP) * scale;
    }
    marks
}

/// Hours over minutes, and smaller seconds under them, as big as fits
/// `room` and centred in it.
fn stacked(count: usize, (width, height): (f32, f32)) -> Vec<Mark> {
    let row_width = 2.0 * WIDTH + GAP;
    let seconds = count > 4;
    let rows_height = if seconds {
        HEIGHT * (2.0 + SECONDS_SCALE) + 2.0 * GAP
    } else {
        2.0 * HEIGHT + GAP
    };
    let scale = (width / row_width).min(height / rows_height);
    let mut y = (height - rows_height * scale) / 2.0;
    let mut marks = Vec::new();
    for row in 0..count / 2 {
        let scale = if row == 2 {
            scale * SECONDS_SCALE
        } else {
            scale
        };
        let x = (width - row_width * scale) / 2.0;
        for column in 0..2 {
            marks.push(Mark::Digit {
                slot: row * 2 + column,
                x: x + column as f32 * (WIDTH + GAP) * scale,
                y,
                scale,
            });
        }
        y += (HEIGHT + GAP) * scale;
    }
    marks
}

/// How one digit is drawn this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stroke {
    Still(u8),
    /// The old digit erased from its start up to `erased`, the new one
    /// drawn up to `drawn`, both fractions of the line's length.
    Changing {
        from: u8,
        to: u8,
        erased: f32,
        drawn: f32,
    },
}

/// Clock `id` shows its digits still on its next frame, with no change
/// drawn: for a desktop shown again after a while.
pub fn settle(id: u32) {
    SLOTS.with_borrow_mut(|clocks| clocks.remove(&id));
}

/// Each digit's stroke for clock `id` showing `values` now, starting a
/// change for each that differs from what it showed, and asking for the
/// next frame while any is changing.
fn strokes(id: u32, values: &[u8]) -> Vec<Stroke> {
    let now = Instant::now();
    let still = theme::reduced_motion();
    SLOTS.with_borrow_mut(|clocks| {
        let slots = clocks.entry(id).or_default();
        // a clock that just showed, or changed its digits, starts still
        if slots.len() != values.len() {
            *slots = values
                .iter()
                .map(|&shown| Slot { shown, from: None })
                .collect();
        }
        // the rightmost change starts first
        let mut later = 0;
        for (slot, &value) in slots.iter_mut().zip(values).rev() {
            if slot.shown != value {
                let delay = CHANGE.mul_f32(STAGGER * later as f32);
                later += 1;
                slot.from = (!still).then_some((slot.shown, now + delay));
                slot.shown = value;
            }
        }
        let mut moving = false;
        let strokes = slots
            .iter_mut()
            .map(|slot| {
                let Some((from, start)) = slot.from else {
                    return Stroke::Still(slot.shown);
                };
                let stroke = changing(from, slot.shown, now.saturating_duration_since(start));
                if stroke == Stroke::Still(slot.shown) {
                    slot.from = None;
                } else {
                    moving = true;
                }
                stroke
            })
            .collect();
        if moving {
            amane::request_frame();
        }
        strokes
    })
}

/// A change from `from` to `to`, `elapsed` into it.
fn changing(from: u8, to: u8, elapsed: Duration) -> Stroke {
    let change = CHANGE.as_secs_f32();
    let erase = change * ERASE;
    let draw_from = erase * DRAW_FROM;
    let at = elapsed.as_secs_f32();
    let drawn = ((at - draw_from) / (change - draw_from)).clamp(0.0, 1.0);
    if drawn >= 1.0 {
        return Stroke::Still(to);
    }
    Stroke::Changing {
        from,
        to,
        erased: Easing::InOut.apply((at / erase).clamp(0.0, 1.0)),
        drawn: Easing::Out.apply(drawn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_erases_then_draws_and_ends_still() {
        assert_eq!(
            changing(3, 4, Duration::ZERO),
            Stroke::Changing {
                from: 3,
                to: 4,
                erased: 0.0,
                drawn: 0.0
            }
        );
        let Stroke::Changing { erased, drawn, .. } = changing(3, 4, CHANGE.mul_f32(ERASE)) else {
            panic!("still changing");
        };
        assert_eq!(erased, 1.0);
        assert!(drawn > 0.0 && drawn < 1.0);
        assert_eq!(changing(3, 4, CHANGE), Stroke::Still(4));
    }

    #[test]
    fn a_clock_starts_still_and_changes_its_digits() {
        assert_eq!(
            strokes(7, &[1, 2, 3, 4]),
            vec![
                Stroke::Still(1),
                Stroke::Still(2),
                Stroke::Still(3),
                Stroke::Still(4)
            ]
        );
        let changed = strokes(7, &[1, 2, 3, 5]);
        assert_eq!(changed[0], Stroke::Still(1));
        assert!(matches!(
            changed[3],
            Stroke::Changing { from: 4, to: 5, .. }
        ));
    }

    #[test]
    fn rows_fit_their_room() {
        let fits = |marks: Vec<Mark>, (width, height): (f32, f32)| {
            marks.iter().all(|mark| {
                let (x, y, scale, w) = match *mark {
                    Mark::Digit { x, y, scale, .. } => (x, y, scale, WIDTH),
                    Mark::Colon { x, y, scale } => (x, y, scale, COLON),
                };
                x >= -0.01
                    && y >= -0.01
                    && x + w * scale <= width + 0.01
                    && y + HEIGHT * scale <= height + 0.01
            })
        };
        assert!(fits(one_row(4, (312.0, 110.0)), (312.0, 110.0)));
        assert!(fits(one_row(6, (312.0, 110.0)), (312.0, 110.0)));
        assert!(fits(stacked(4, (132.0, 132.0)), (132.0, 132.0)));
        assert!(fits(stacked(6, (312.0, 290.0)), (312.0, 290.0)));
        assert_eq!(one_row(6, (312.0, 110.0)).len(), 8);
    }
}
