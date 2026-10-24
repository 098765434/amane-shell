//! The power menu — GNOME's Power Off submenu as a page: Suspend, then
//! Restart…, Power Off… and Log Out…, each shown when logind allows it.
//! The ones that end the session ask for a second click first, which stands
//! in for GNOME's confirmation dialog. (Switch User is left out: the
//! display manager here has no user switching.)

use std::cell::Cell as Slot;

use amane::{Service, Widget};

use crate::components::icons::{ICON_LOG_OUT, ICON_POWER, ICON_RESTART, ICON_SLEEP};
use crate::components::layout::boxed;
use crate::components::list::{self, List, ListItem};
use crate::components::quick_settings::detail::DetailPage;
use crate::components::taskbar::close;
use crate::services::session::Session;
use crate::tokens::sys;

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

    /// Whether a click only asks "are you sure?" first.
    fn confirms(self) -> bool {
        self != Self::Suspend
    }

    fn confirm_text(self) -> &'static str {
        match self {
            Self::Suspend => "",
            Self::Restart => "Click again to restart",
            Self::PowerOff => "Click again to power off",
            Self::LogOut => "Click again to log out",
        }
    }

    fn run(self) {
        close();
        match self {
            Self::Suspend => Session::suspend(),
            Self::Restart => Session::reboot(),
            Self::PowerOff => Session::power_off(),
            Self::LogOut => Session::log_out(),
        }
    }
}

thread_local! {
    // the action waiting for its second click
    static ARMED: Slot<Option<Action>> = const { Slot::new(None) };
}

/// Forgets a first click; the page calls this as it's left.
pub(crate) fn reset() {
    ARMED.set(None);
}

/// What a click on `action` does, given the one waiting for a second click.
fn click(action: Action, armed: Option<Action>) -> Option<Action> {
    (action.confirms() && armed != Some(action)).then_some(action)
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

    let armed = ARMED.get();
    let items: Vec<ListItem> = actions
        .iter()
        .map(|&action| {
            let mut item = ListItem::new(format!("amane.qs.session.{action:?}"), action.label())
                .leading(action.glyph())
                .on_press(move || match click(action, ARMED.get()) {
                    Some(armed) => {
                        ARMED.set(Some(armed));
                        amane::mark_all();
                    }
                    None => {
                        ARMED.set(None);
                        action.run();
                    }
                });
            if armed == Some(action) {
                item = item.selected(true).trailing(list::trailing_text(
                    action.confirm_text(),
                    sys::color::status::critical::bg(),
                ));
            }
            item
        })
        .collect();

    let rows = items.len() as f32;
    let height = rows * sys::size::row::height() + (rows - 1.0) * sys::space::gap::xs();
    let body: Vec<Box<dyn Widget>> = vec![boxed(List::new(items).build())];

    DetailPage::new("amane.qs.session.page", ICON_POWER, "Power Off").body(body, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ending_the_session_takes_two_clicks() {
        assert_eq!(click(Action::Restart, None), Some(Action::Restart));
        assert_eq!(click(Action::Restart, Some(Action::Restart)), None);
        // a first click elsewhere doesn't confirm this one
        assert_eq!(
            click(Action::PowerOff, Some(Action::Restart)),
            Some(Action::PowerOff)
        );
        assert_eq!(click(Action::Suspend, None), None);
    }
}
