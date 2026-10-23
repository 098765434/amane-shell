//! Notifications, macOS/iOS-style: toasts that slide in at the top-right,
//! a notification center in a sidebar, and Do Not Disturb.
//!
//! amane is the notification daemon (its `Notifications` service holds the
//! list and talks to senders); everything here is how the shell shows it:
//!
//! - [`toasts`] — a banner per arriving notification, for its lifetime.
//! - [`center`] — the session's notifications, stacked by app, in a
//!   right-edge sidebar the clock opens.
//! - [`dnd`] — Do Not Disturb, on or until a time, kept across restarts.
//! - [`card`] — the card both draw; [`motion`](crate::components::motion)
//!   moves their cards.
//!
//! See `docs/notifications.md`.

pub mod card;
pub mod center;
pub mod dnd;
pub mod toasts;

use amane::{Notifications, mark_all};

pub use center::{CENTER_NS, side_sheet, unseen};
pub use dnd::DoNotDisturb;
pub use toasts::{TOAST_NS, TOASTS};

use crate::components::taskbar::{Panel, toggle};
use dnd::Until;

/// `amane-shell` IPC: `notifications [toggle | clear | dnd [on|off|toggle]]`.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["toggle"] => toggle(Panel::Notifications),
        ["clear"] => Notifications::clear(),
        ["dnd"] | ["dnd", "toggle"] => DoNotDisturb::toggle(),
        ["dnd", "on"] => DoNotDisturb::set(Some(Until::Forever)),
        ["dnd", "off"] => DoNotDisturb::set(None),
        _ => return String::from("usage: notifications [toggle | clear | dnd [on|off|toggle]]"),
    }
    mark_all();
    String::from("ok")
}
