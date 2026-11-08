//! Notifications: toasts that slide in at the top-right,
//! a notification center in a sidebar, and Do Not Disturb.
//!
//! amane is the notification daemon (its `Notifications` service holds the
//! list and talks to senders); everything here is how the shell shows it:
//!
//! - [`toasts`] — a banner per arriving notification, for its lifetime.
//! - [`center`] — the session's notifications, stacked by app, in a
//!   right-edge sidebar the clock opens.
//! - [`dnd_menu()`] — Do Not Disturb's timed choices; Do Not Disturb itself
//!   is a service (`services::do_not_disturb`).
//! - [`card`] — the card both draw; [`motion`](amane_kit::motion)
//!   moves their cards.
//!
//! See `docs/user/features/notifications.md`.

mod calendar_card;
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
use amane_kit::shell_ipc::{Notifications as Command, Switch};

/// `amane-shell` IPC: `notifications [toggle | clear | dnd [on|off|toggle]]`.
pub fn run(command: Command) -> String {
    match command {
        Command::Toggle => toggle(Panel::Notifications),
        Command::Clear => Notifications::clear(),
        Command::DoNotDisturb(Switch::Toggle) => DoNotDisturb::toggle(),
        Command::DoNotDisturb(Switch::On) => DoNotDisturb::set(Some(Until::Forever)),
        Command::DoNotDisturb(Switch::Off) => DoNotDisturb::set(None),
    }
    // each writes its own service, and toggling the panel marks the panels
    String::from("ok")
}
