//! The order the launcher's glass comes and goes in.
//!
//! Opening, the sheet starts as a drop at the pill's centre and stretches
//! out sideways into the pill (the sheet's own spring, from
//! [`shape::seed`](super::shape::seed)); the mode buttons bud off its end
//! one after another; the search row's text comes last. Opened on a mode,
//! the glass holds as the pill for a moment before growing into the panel,
//! so it unfolds drop, pill, panel.
//!
//! Closing runs the other way and quicker: the text goes first, the
//! buttons tuck back into the pill from the far end in, and the pill
//! draws back towards its centre as it fades.
//!
//! What's under the search row rises in row by row when it first shows
//! (a search's results, a mode's list or grid), not with each key typed.
//!
//! Where motion is reduced, each part fades at once
//! ([`Stagger`](emaki_kit::stagger::Stagger)).

use std::cell::Cell;
use std::time::Duration;

use amane::Easing;
use emaki_kit::stagger::Stagger;

use crate::tokens::sys;

// TODO(token): `launcher.open.*`, `launcher.close.*` and `launcher.enter.*`
// once Penpot has them and the component tier has room (manual §1.1, §3).
/// Opening: how long the glass holds as the pill before growing into a
/// mode's panel; when the first mode button buds off, how far apart the
/// next ones follow, and how long each takes; how small one starts.
const STRETCH: Duration = Duration::from_millis(160);
const BUD_DELAY: Duration = Duration::from_millis(110);
const BUD_STEP: Duration = Duration::from_millis(45);
const BUD_LENGTH: Duration = Duration::from_millis(460);
pub(super) const BUD_SCALE: f32 = 0.55;
/// Opening: when the search row's text comes in, how long it takes, and
/// how far it slides in from.
const TEXT_DELAY: Duration = Duration::from_millis(170);
const TEXT_LENGTH: Duration = Duration::from_millis(240);
const TEXT_SLIDE: f32 = 8.0;
/// Closing: how far apart the buttons tuck in, how long each takes, and
/// how quickly the text goes.
const CLOSE_STEP: Duration = Duration::from_millis(22);
const CLOSE_LENGTH: Duration = Duration::from_millis(110);
const CLOSE_TEXT: Duration = Duration::from_millis(80);
/// Rows rising in: how far apart, how long each takes, how far below
/// they start, how small an app's icon starts, and after how many rows
/// the rest come in together.
const ENTER_STEP: Duration = Duration::from_millis(22);
const ENTER_LENGTH: Duration = Duration::from_millis(280);
const ENTER_RISE: f32 = 8.0;
pub(super) const ENTER_SCALE: f32 = 0.94;
const ENTER_ROWS: usize = 8;

thread_local! {
    // since the launcher last turned towards open, and towards closed
    static OPENING: Cell<Stagger> = const { Cell::new(Stagger::new()) };
    static CLOSING: Cell<Stagger> = const { Cell::new(Stagger::new()) };
    // since what's under the search row last changed kind
    static ENTERING: Cell<Stagger> = const { Cell::new(Stagger::new()) };
}

/// How many mode buttons bud off the pill.
const BUDS: usize = 4;
/// How far open the glass may be when it turns back towards open and
/// still play the whole opening: past it, it was only just closing, and
/// carries on from where it is.
const FROM_CLOSED: f32 = 0.05;

/// The launcher turned towards `open`, `was` far open.
pub(super) fn turned(open: bool, was: f32) {
    if open {
        CLOSING.set(Stagger::new());
        let mut clock = Stagger::new();
        if was < FROM_CLOSED {
            clock.start();
        }
        OPENING.set(clock);
    } else {
        let mut clock = Stagger::new();
        clock.start();
        CLOSING.set(clock);
    }
}

/// Gone from the screen: every part rests.
pub(super) fn reset() {
    OPENING.set(Stagger::new());
    CLOSING.set(Stagger::new());
    ENTERING.set(Stagger::new());
}

/// Whether the glass is opening from closed, with motion not reduced: the
/// sheet starts as a drop rather than in place.
pub(super) fn from_drop() -> bool {
    OPENING.get().started() && !emaki_kit::theme::reduced_motion()
}

/// Whether the glass is still stretching into the pill, and so holds as
/// one even when a mode is open under it.
pub(super) fn holding() -> bool {
    OPENING.get().within(STRETCH)
}

/// How far mode button `index` has budded off the pill, 0 tucked into it
/// … 1 at rest (a little past it on the spring).
pub(super) fn bud(index: usize, opening: bool) -> f32 {
    if opening {
        OPENING.get().nth(
            index,
            BUD_DELAY,
            BUD_STEP,
            BUD_LENGTH,
            Easing::Spring {
                bounce: sys::motion::spring::bounce(),
            },
        )
    } else {
        1.0 - CLOSING.get().nth(
            BUDS - 1 - index.min(BUDS - 1),
            Duration::ZERO,
            CLOSE_STEP,
            CLOSE_LENGTH,
            Easing::Out,
        )
    }
}

/// How strongly the search row's text shows, 0 … 1: last in as the glass
/// opens, first out as it closes.
pub(super) fn text(opening: bool) -> f32 {
    if opening {
        OPENING.get().progress(TEXT_DELAY, TEXT_LENGTH, Easing::Out)
    } else {
        1.0 - CLOSING
            .get()
            .progress(Duration::ZERO, CLOSE_TEXT, Easing::Out)
    }
}

/// How far the search row's text slides in from, at `text` shown.
pub(super) fn text_slide(text: f32) -> f32 {
    -emaki_kit::motion::travel(TEXT_SLIDE) * (1.0 - text)
}

/// What's under the search row changed kind: it rises in afresh, once the
/// glass has stopped holding as the pill.
pub(super) fn enter() {
    let opening = OPENING.get();
    let hold = STRETCH;
    let mut clock = Stagger::new();
    match opening.elapsed() {
        Some(elapsed) if opening.within(hold) => clock.start_in(hold - elapsed),
        _ => clock.start(),
    }
    ENTERING.set(clock);
}

/// The clock what's under the search row rises in on.
pub(super) fn entering() -> Stagger {
    ENTERING.get()
}

/// How far row `index` (in view, from the top) has risen in, 0 … 1, on
/// `clock`. Rows past the first few come in together.
pub(super) fn entered(clock: Stagger, index: usize) -> f32 {
    clock.nth(
        index.min(ENTER_ROWS),
        Duration::ZERO,
        ENTER_STEP,
        ENTER_LENGTH,
        Easing::Out,
    )
}

/// How far a row `entered` this far is still below where it rests.
pub(super) fn rise(entered: f32) -> f32 {
    emaki_kit::motion::travel(ENTER_RISE) * (1.0 - entered)
}
