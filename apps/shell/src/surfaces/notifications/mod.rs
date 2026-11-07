//! Notifications: toasts that slide in at the top-right,
//! a notification center in a sidebar, and Do Not Disturb.
//!
//! amane is the notification daemon (its `Notifications` service holds the
//! list and talks to senders); everything here is how the shell shows it:
//!
//! - [`toasts`] — a banner per arriving notification, for its lifetime.
//! - [`center`] — the session's notifications, stacked by app, in a
//!   right-edge sidebar the clock opens.
//! - [`dnd_menu`] — Do Not Disturb's timed choices; Do Not Disturb itself
//!   is a service (`services::do_not_disturb`).
//! - [`card`] — the card both draw; [`motion`](amane_kit::motion)
//!   moves their cards.
//!
//! See `docs/shell/notifications.md`.

pub mod card;
pub mod center;
mod dnd_menu;
pub mod toasts;

use amane::Notifications;

pub use center::{CENTER_NS, register_panel, side_sheet, unseen};
pub use dnd_menu::open as dnd_menu;
pub use toasts::{TOAST_NS, TOASTS};

use crate::chrome::panel::{Panel, toggle};
use crate::services::do_not_disturb::{DoNotDisturb, Until};

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
    // each writes its own service, and toggling the panel marks the panels
    String::from("ok")
}
