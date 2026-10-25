//! The shell's motion: the springs and fades every control moves with
//! ([`spring`], [`sheet`], [`fade`]), and keyed motion for lists of cards —
//! the toasts and the center's stacks ([`animate`]).
//!
//! Where motion is reduced (the settings, else the system:
//! [`theme::reduced_motion`]), every spring is a short fade instead, and
//! what swells or travels in ([`scale`], [`travel`]) arrives in place. An
//! `Animation` takes its curve when it's made, so one kept for the shell's
//! life (a panel's opening) follows a change after a restart; [`scale`] and
//! [`travel`] follow it at once.
//!
//! For a list, the view says where each item should be (a [`Pose`]) every time it
//! runs; this springs each item there from wherever it is, keeping its
//! `Animation` across rebuilds under its key, like the segmented button's
//! thumb. An item that drops out of the list isn't gone yet: it keeps its
//! last value and moves to its exit pose, and the view keeps drawing it
//! until it gets there.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use amane::{Animation, Blend, Easing, mark_all};

use crate::theme;

// iOS-like motion. TODO(token): these become `sys.motion.*` tokens once
// Penpot has them (manual §1.1: no style literals in component code).
/// How long a spring takes to settle: a knob, a thumb, a card moving.
const SPRING: Duration = Duration::from_millis(450);
const SPRING_BOUNCE: f32 = 0.15;
/// A whole sheet sliding in: longer, and calmer.
const SHEET: Duration = Duration::from_millis(500);
const SHEET_BOUNCE: f32 = 0.1;
/// A popover swelling in over the bar: quick, with a breath of overshoot.
pub const POP: Duration = Duration::from_millis(380);
const POP_BOUNCE: f32 = 0.08;
/// A selection fading in or out.
pub const FADE: Duration = Duration::from_millis(200);
/// A card on its way out.
pub const EXIT: Duration = Duration::from_millis(300);
/// The hover layer coming and going under the pointer.
pub const HOVER: Duration = Duration::from_millis(120);
/// A pressed control swelling back to its size.
pub const PRESS: Duration = Duration::from_millis(220);
/// A popover going: quicker than it came ([`POP`]).
pub const CLOSE: Duration = Duration::from_millis(160);
/// How small a card is as it enters, growing to its size.
pub const ENTER_SCALE: f32 = 0.9;

/// Whether motion is reduced.
fn reduced() -> bool {
    theme::reduced_motion()
}

/// `from`, the scale something swells in from, or 1 where motion is
/// reduced: it fades in at its size.
pub fn scale(from: f32) -> f32 {
    if reduced() { 1.0 } else { from }
}

/// `distance`, how far something travels in, or 0 where motion is reduced:
/// it fades in in its place.
pub fn travel(distance: f32) -> f32 {
    if reduced() { 0.0 } else { distance }
}

/// The spring things move on, starting at rest at `value`.
pub fn spring<T: Blend>(value: T) -> Animation<T> {
    if reduced() {
        return fade(value, FADE);
    }
    Animation::new(value)
        .duration(SPRING)
        .easing(Easing::Spring {
            bounce: SPRING_BOUNCE,
        })
}

/// The spring a whole sheet slides in on.
pub fn sheet<T: Blend>(value: T) -> Animation<T> {
    if reduced() {
        return fade(value, FADE);
    }
    Animation::new(value)
        .duration(SHEET)
        .easing(Easing::Spring {
            bounce: SHEET_BOUNCE,
        })
}

/// A page growing out of the control that opens it: a sheet's spring
/// with less overshoot, as the page fills the card it's clipped to.
const ZOOM_BOUNCE: f32 = 0.05;

/// The spring a page grows out of its control on.
pub fn zoom<T: Blend>(value: T) -> Animation<T> {
    if reduced() {
        return fade(value, FADE);
    }
    Animation::new(value)
        .duration(SHEET)
        .easing(Easing::Spring {
            bounce: ZOOM_BOUNCE,
        })
}

/// The spring a popover swells in on.
pub fn pop<T: Blend>(value: T) -> Animation<T> {
    if reduced() {
        return fade(value, FADE);
    }
    Animation::new(value)
        .duration(POP)
        .easing(Easing::Spring { bounce: POP_BOUNCE })
}

/// How long a spring takes to settle, for motion that runs on its own clock.
pub const fn settle() -> Duration {
    SPRING
}

/// The spring's curve at `progress` (0 … 1 of [`settle`]), for motion that
/// runs on its own clock rather than an `Animation`.
pub fn spring_curve(progress: f32) -> f32 {
    if reduced() {
        return Easing::Out.apply(progress.clamp(0.0, 1.0));
    }
    Easing::Spring {
        bounce: SPRING_BOUNCE,
    }
    .apply(progress.clamp(0.0, 1.0))
}

/// An ease-out over `duration`, for fades and exits.
pub fn fade<T: Blend>(value: T, duration: Duration) -> Animation<T> {
    Animation::new(value).duration(duration).easing(Easing::Out)
}

/// Redraws every window once `delay` has passed, for a view whose output
/// changes with time alone (a wait running out). amane has no timers, so a
/// thread sleeps it.
pub fn redraw_after(delay: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        mark_all();
    });
}

/// Where one card is and how it looks: moved by `x`/`y`, scaled about its
/// center, the whole card at `opacity` and its content at `content` (a
/// stacked card behind another is a blank plate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub content: f32,
}

impl Pose {
    /// Fully shown at `y`.
    pub const fn at(y: f32) -> Self {
        Self {
            x: 0.0,
            y,
            scale: 1.0,
            opacity: 1.0,
            content: 1.0,
        }
    }

    pub const fn moved(self, x: f32) -> Self {
        Self {
            x: self.x + x,
            ..self
        }
    }

    pub const fn faded(self) -> Self {
        Self {
            opacity: 0.0,
            ..self
        }
    }

    /// How far down its visible bottom edge is, scale included.
    pub fn bottom(self, height: f32) -> f32 {
        self.y + height * (1.0 + self.scale) / 2.0
    }
}

impl Blend for Pose {
    fn blend(from: Self, to: Self, amount: f32) -> Self {
        Self {
            x: f32::blend(from.x, to.x, amount),
            y: f32::blend(from.y, to.y, amount),
            scale: f32::blend(from.scale, to.scale, amount),
            opacity: f32::blend(from.opacity, to.opacity, amount),
            content: f32::blend(from.content, to.content, amount),
        }
    }

    // along whichever way it moves most: across, down, then its scale and fade
    fn along(from: Self, to: Self, value: Self) -> Option<f32> {
        let ways = [
            (from.x, to.x, value.x),
            (from.y, to.y, value.y),
            (from.scale, to.scale, value.scale),
            (from.opacity, to.opacity, value.opacity),
        ];
        let moved = ways
            .iter()
            .take(2)
            .max_by(|a, b| (a.1 - a.0).abs().total_cmp(&(b.1 - b.0).abs()))
            .filter(|(from, to, _)| (to - from).abs() > 0.5);
        let (from, to, value) = moved.or_else(|| {
            ways.iter()
                .skip(2)
                .find(|(from, to, _)| (to - from).abs() > f32::EPSILON)
        })?;
        f32::along(*from, *to, *value)
    }
}

/// One entry: where it should be now, where it comes in from the first
/// time, and where it goes once it's no longer listed.
pub struct Item<T> {
    pub key: u64,
    pub value: T,
    pub height: f32,
    pub pose: Pose,
    pub enter: Pose,
    pub exit: Pose,
}

/// One entry as it shows this frame.
pub struct Shown<T> {
    pub key: u64,
    pub value: T,
    pub height: f32,
    pub pose: Pose,
    pub leaving: bool,
}

/// Whether items in a list seen for the first time come in from their
/// `enter` pose, or are simply there (opening a panel shouldn't animate
/// everything in it).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Arrive {
    Enter,
    Rest,
}

struct Track {
    key: u64,
    motion: Animation<Pose>,
    exit: Pose,
    // set once it has dropped out of the list
    left: Option<Instant>,
    height: f32,
    value: Rc<dyn Any>,
}

thread_local! {
    static LISTS: RefCell<HashMap<String, Vec<Track>>> = RefCell::new(HashMap::new());
}

/// Steps the list `scope` towards `items` and gives what to draw: the items
/// that are leaving first (so they pass under the rest), then `items` in
/// order.
pub fn animate<T: Clone + 'static>(
    scope: &str,
    items: Vec<Item<T>>,
    arrive: Arrive,
) -> Vec<Shown<T>> {
    LISTS.with_borrow_mut(|lists| {
        let fresh = !lists.contains_key(scope);
        let tracks = lists.entry(scope.to_string()).or_default();

        // what dropped out starts leaving; what has left is forgotten
        for track in tracks.iter_mut() {
            if track.left.is_none() && !items.iter().any(|item| item.key == track.key) {
                let from = track.motion.value();
                track.motion = fade(from, EXIT);
                track.motion.to(track.exit);
                track.left = Some(Instant::now());
            }
        }
        tracks.retain(|track| track.left.is_none_or(|left| left.elapsed() < EXIT));

        let mut shown: Vec<Shown<T>> = tracks
            .iter()
            .filter(|track| track.left.is_some())
            .filter_map(|track| {
                Some(Shown {
                    key: track.key,
                    value: track.value.downcast_ref::<T>()?.clone(),
                    height: track.height,
                    pose: track.motion.value(),
                    leaving: true,
                })
            })
            .collect();

        for item in items {
            let track = match tracks.iter_mut().position(|track| track.key == item.key) {
                Some(index) => &mut tracks[index],
                None => {
                    let start = if fresh && arrive == Arrive::Rest {
                        item.pose
                    } else {
                        item.enter
                    };
                    tracks.push(Track {
                        key: item.key,
                        motion: spring(start),
                        exit: item.exit,
                        left: None,
                        height: item.height,
                        value: Rc::new(()),
                    });
                    tracks.last_mut().expect("just pushed")
                }
            };

            // back before it finished leaving: spring from where it got to
            if track.left.take().is_some() {
                track.motion = spring(track.motion.value());
            }
            track.motion.to(item.pose);
            track.exit = item.exit;
            track.height = item.height;
            track.value = Rc::new(item.value.clone());

            shown.retain(|leaving| leaving.key != item.key);
            shown.push(Shown {
                key: item.key,
                value: item.value,
                height: item.height,
                pose: track.motion.value(),
                leaving: false,
            });
        }

        shown
    })
}

/// Puts an item at `pose` right away, springing on to its target from
/// there — where a swipe let go, say.
pub fn jump(scope: &str, key: u64, pose: Pose) {
    LISTS.with_borrow_mut(|lists| {
        let Some(track) = lists
            .get_mut(scope)
            .and_then(|tracks| tracks.iter_mut().find(|track| track.key == key))
        else {
            return;
        };
        let target = track.motion.value();
        track.motion = spring(pose);
        track.motion.to(target);
    });
}

/// Forgets every list whose scope starts with `prefix`, so it rests on
/// its next build.
pub fn forget(prefix: &str) {
    LISTS.with_borrow_mut(|lists| lists.retain(|scope, _| !scope.starts_with(prefix)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: u64, y: f32) -> Item<&'static str> {
        Item {
            key,
            value: "card",
            height: 10.0,
            pose: Pose::at(y),
            enter: Pose::at(y).moved(100.0),
            exit: Pose::at(y).moved(100.0),
        }
    }

    #[test]
    fn a_resting_list_opens_still_and_later_items_enter() {
        let shown = animate("test.rest", vec![item(1, 0.0)], Arrive::Rest);
        assert_eq!(shown[0].pose, Pose::at(0.0));

        let shown = animate("test.rest", vec![item(1, 0.0), item(2, 20.0)], Arrive::Rest);
        assert!(
            shown[1].pose.x > 50.0,
            "the new item starts off to the side"
        );
    }

    #[test]
    fn the_first_item_of_an_entering_list_enters() {
        let shown = animate("test.enter", vec![item(1, 0.0)], Arrive::Enter);
        assert!(shown[0].pose.x > 50.0);
    }

    #[test]
    fn a_dropped_item_shows_until_it_has_left() {
        animate(
            "test.exit",
            vec![item(1, 0.0), item(2, 20.0)],
            Arrive::Enter,
        );

        let shown = animate("test.exit", vec![item(2, 20.0)], Arrive::Enter);
        assert_eq!(shown.len(), 2);
        assert!(shown[0].leaving && shown[0].key == 1);
        assert!(!shown[1].leaving);

        std::thread::sleep(EXIT + Duration::from_millis(20));
        let shown = animate("test.exit", vec![item(2, 20.0)], Arrive::Enter);
        assert_eq!(shown.len(), 1);
    }

    #[test]
    fn an_item_that_comes_back_stops_leaving() {
        animate("test.back", vec![item(1, 0.0)], Arrive::Enter);
        animate::<&str>("test.back", Vec::new(), Arrive::Enter);

        let shown = animate("test.back", vec![item(1, 0.0)], Arrive::Enter);
        assert_eq!(shown.len(), 1);
        assert!(!shown[0].leaving);
    }

    #[test]
    fn bottom_counts_the_scale() {
        let half = Pose {
            scale: 0.5,
            ..Pose::at(0.0)
        };
        assert_eq!(Pose::at(5.0).bottom(10.0), 15.0);
        assert_eq!(half.bottom(10.0), 7.5);
    }
}
