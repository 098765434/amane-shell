//! Activities in the bar: what's going on now. The bar's own come
//! straight from their services: Now Playing,
//! the launcher's timer and screen sharing. Other programs' come over the
//! bar's bus service ([`activities`](crate::services::desktop::activities)).
//!
//! The most relevant show **compact**: the leading picture and the
//! trailing text, countdown or ring (`StatusBar::activities_compact` of
//! them). The rest show **minimal**, the picture alone. A click opens the
//! **expanded** view under the bar, with the title, a progress bar and up to
//! three buttons. An update that alerts brings that view up for a few
//! seconds, but not under Do Not Disturb.

mod chip;
mod expanded;
mod sources;

pub use chip::widgets;
use chip::*;
pub(super) use chip::{clip, picture};
pub use expanded::popup;
use expanded::*;
use sources::*;

use emaki_kit::shell_ipc::Activities;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use amane::{
    Apps, Arc, Canvas, Cap, Center, Color, ConstraintAdjustment, Gravity, Image, Media, Parent,
    Popup, Rectangle, Row, Service, Shape, Start, Text, Widget, window_size,
};
use emaki_kit::color::Accent;
use emaki_kit::icons::{
    ICON_MUSIC, ICON_PAUSE, ICON_PLAY, ICON_RECORD, ICON_SCREEN_SHARE, ICON_SKIP_NEXT,
    ICON_SKIP_PREVIOUS, ICON_TIMER, named,
};
use emaki_kit::settings::{BUILTIN_ACTIVITIES, status_bar};
use emaki_extension::{ActivityButton, ActivityContent, ItemIcon, Tint, Trailing};

use crate::services::desktop::activities::{self as registry, Live};
use crate::services::desktop::privacy::{CastTarget, Privacy, stop_cast};
use crate::services::do_not_disturb::DoNotDisturb;
use crate::services::launcher::timer;
use crate::surfaces::launcher;
use crate::surfaces::quick_settings::{self, Page};
use crate::tokens::{Styled, sys};
use emaki_kit::buttons::{DrawnButton, IconButton, PushButton, Style};
use emaki_kit::card::{Card, Variant};
use emaki_kit::layout::{boxed, cursor, icon, text_width};
use emaki_kit::material::{self, FillLevel, Level, Material};
use emaki_kit::motion;

use super::{BAR_NS, item, menu_edge};

/// The expanded view's popup namespace; it hangs under the bar, whose
/// layer rule frosts its popups.
pub const POPUP_NS: &str = "emaki.status-bar.activity";

/// How long a paused player keeps its activity.
const PAUSED_FOR: Duration = Duration::from_secs(5 * 60);

/// How wide a compact activity's text may be, and Now Playing's title.
const TEXT: f32 = 120.0;
const TITLE: f32 = 160.0;

/// The leading picture's size expanded; compact, it's [`leading`].
const LEADING_BIG: f32 = 36.0;

/// The leading picture's size in the bar: a little over a glyph.
fn leading() -> f32 {
    super::metrics().icon + 2.0
}

/// The expanded view's width, and its rows' heights.
const CARD_WIDTH: f32 = 320.0;
const HEADER: f32 = 40.0;
const BAR: f32 = 4.0;
const BUTTONS: f32 = 30.0;

/// A beat past each second turning over, so the frame finds it turned.
const TICK: Duration = Duration::from_millis(20);

/// The apps the bar's own activities go under, for the permission list.
pub const NOW_PLAYING: &str = BUILTIN_ACTIVITIES[0];
pub const TIMER: &str = BUILTIN_ACTIVITIES[1];
pub const SHARING: &str = BUILTIN_ACTIVITIES[2];

emaki_kit::marker! {
    /// A countdown's second turned over, or the expanded view opened or
    /// closed: the bar and the view draw again.
    pub Tick
}

/// One activity as the bar shows it.
#[derive(Clone)]
struct Shown {
    /// What tells it apart: `now-playing`, `timer`, `sharing` or `bus.<handle>`.
    key: String,
    content: ActivityContent,
    /// Out of date, or ended and showing its last state: dimmed.
    dim: bool,
    /// A button by its id.
    act: Rc<dyn Fn(&str)>,
    /// What a middle click does, as Now Playing plays or pauses.
    middle: Option<Rc<dyn Fn()>>,
}

/// The expanded view showing.
#[derive(Clone)]
struct Open {
    key: String,
    /// Where it hangs from on the bar.
    x: i32,
    /// Opened by a click, so it takes a grab; an alert's doesn't.
    clicked: bool,
}

thread_local! {
    // when something last played
    static LAST_PLAYED: Cell<Option<Instant>> = const { Cell::new(None) };

    // the expanded view showing
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };

    // the instant the next countdown frame was asked for
    static TICK_DUE: Cell<Option<Instant>> = const { Cell::new(None) };

    // the alert that already opened the view, so closing it keeps it closed
    static ALERTED: Cell<Option<u32>> = const { Cell::new(None) };
}

// ───── the expanded view ─────────────────────────────────────────────────────

/// `status-bar activities list | end <key>`: each going on, most relevant
/// first, as `key<TAB>app<TAB>title`; ending one is the user's, as an
/// activity can be dismissed.
pub fn run(command: Activities) -> String {
    match command {
        Activities::List => gather()
            .iter()
            .map(|activity| {
                format!(
                    "{}\t{}\t{}",
                    activity.key, activity.content.app, activity.content.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Activities::End(key) => match key.strip_prefix("bus.").and_then(|h| h.parse().ok()) {
            Some(handle) => {
                registry::dismiss(handle);
                String::from("ok")
            }
            None => format!("only other programs' activities end here, as bus.<handle>; not {key}"),
        },
        // what a click on one of its buttons does
        Activities::Press { key, button } => {
            match key.strip_prefix("bus.").and_then(|h| h.parse().ok()) {
                Some(handle) => {
                    registry::act(handle, &button);
                    String::from("ok")
                }
                None => format!("only other programs' activities are pressed here; not {key}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdowns_never_go_below_nothing() {
        let (text, _) = trailing_text(&Trailing::Countdown(0), false).expect("a countdown");
        assert_eq!(text, "0:00");
    }

    #[test]
    fn text_needs_something_to_show() {
        assert!(trailing_text(&Trailing::Text(String::new()), false).is_none());
        assert!(trailing_text(&Trailing::None, false).is_none());
        assert!(trailing_text(&Trailing::Progress(0.5), false).is_none());
    }
}
