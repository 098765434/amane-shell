//! The power menu — GNOME's Power Off submenu as a page: Suspend, then
//! Restart…, Power Off… and Log Out…, each shown when logind allows it.
//! The ones that end the session ask first in an [`alert`], as macOS does.
//! (Switch User is left out: the display manager here has no user
//! switching.)

use amane::Service;

use crate::surfaces::alert;
use crate::components::dialog::Dialog;
use crate::components::icons::{ICON_LOG_OUT, ICON_POWER, ICON_RESTART, ICON_SLEEP};
use crate::components::layout::boxed;
use crate::components::list::{List, ListItem};
use crate::components::panel::close;
use crate::surfaces::quick_settings::detail::DetailPage;
use crate::services::session::Session;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Suspend,
    Restart,
    PowerOff,
    LogOut,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Suspend => "Suspend",
            Self::Restart => "Restart…",
            Self::PowerOff => "Power Off…",
            Self::LogOut => "Log Out…",
        }
    }

    fn glyph(self) -> char {
        match self {
            Self::Suspend => ICON_SLEEP,
            Self::Restart => ICON_RESTART,
            Self::PowerOff => ICON_POWER,
            Self::LogOut => ICON_LOG_OUT,
        }
    }

    /// The question it asks before ending the session, if it does: the
    /// title, the message, and the answer's label.
    fn question(self) -> Option<(&'static str, &'static str, &'static str)> {
        let unsaved = "Apps that are open may lose unsaved changes.";
        match self {
            Self::Suspend => None,
            Self::Restart => Some((
                "Are you sure you want to restart your computer now?",
                unsaved,
                "Restart",
            )),
            Self::PowerOff => Some((
                "Are you sure you want to power off your computer now?",
                unsaved,
                "Power Off",
            )),
            Self::LogOut => Some((
                "Are you sure you want to quit all apps and log out now?",
                unsaved,
                "Log Out",
            )),
        }
    }

    fn run(self) {
        match self {
            Self::Suspend => Session::suspend(),
            Self::Restart => Session::reboot(),
            Self::PowerOff => Session::power_off(),
            Self::LogOut => Session::log_out(),
        }
    }

    /// What a click does: Quick Settings closes, and the action runs, or
    /// asks first.
    fn press(self) {
        close();
        let Some((title, message, answer)) = self.question() else {
            self.run();
            return;
        };
        let ask = Dialog::new(format!("amane.alert.session.{self:?}"), title)
            .message(message)
            .button("Cancel", || {});
        // logging out quits every app, so it's said in red
        let ask = if self == Self::LogOut {
            ask.destructive(answer, move || self.run())
        } else {
            ask.main_button(answer, move || self.run())
        };
        alert::show(ask);
    }
}

pub(crate) fn page() -> DetailPage {
    let session = Session::read();
    let actions: Vec<Action> = [
        (Action::Suspend, session.can_suspend()),
        (Action::Restart, session.can_reboot()),
        (Action::PowerOff, session.can_power_off()),
        (Action::LogOut, true),
    ]
    .into_iter()
    .filter_map(|(action, allowed)| allowed.then_some(action))
    .collect();
    drop(session);

    DetailPage::new("amane.qs.session.page", "Power Off").body(move |_| {
        let items: Vec<ListItem> = actions
            .iter()
            .map(|&action| {
                ListItem::new(format!("amane.qs.session.{action:?}"), action.label())
                    .leading(action.glyph())
                    .on_press(move || action.press())
            })
            .collect();
        vec![boxed(List::new(items).build())]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ending_the_session_asks_first() {
        assert!(Action::Suspend.question().is_none());
        for action in [Action::Restart, Action::PowerOff, Action::LogOut] {
            let (_, _, answer) = action.question().expect("it asks");
            // the answer repeats the row's label, without its ellipsis
            assert_eq!(format!("{answer}…"), action.label());
        }
    }
}
