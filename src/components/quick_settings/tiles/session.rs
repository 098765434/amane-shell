//! The power menu — GNOME's Power Off submenu as a page: Suspend, then
//! Restart…, Power Off… and Log Out…, each shown when logind allows it.
//! The ones that end the session ask for a second click first, which stands
//! in for GNOME's confirmation dialog. (Switch User is left out: the
//! display manager here has no user switching.)

use std::cell::Cell;
use std::time::{Duration, Instant};

use amane::Service;

use crate::components::icons::{ICON_LOG_OUT, ICON_POWER, ICON_RESTART, ICON_SLEEP};
use crate::components::layout::boxed;
use crate::components::list::{self, List, ListItem};
use crate::components::motion::redraw_after;
use crate::components::quick_settings::detail::DetailPage;
use crate::components::taskbar::close;
use crate::services::session::Session;
use crate::tokens::sys;

/// How long a first click waits for its second before it's forgotten.
const ARMED_FOR: Duration = Duration::from_secs(5);

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

    /// What the row says while it waits for the second click.
    fn confirm_text(self) -> &'static str {
        match self {
            Self::Suspend => "",
            Self::Restart => "Click to Restart",
            Self::PowerOff => "Click to Power Off",
            Self::LogOut => "Click to Log Out",
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
    // the action waiting for its second click, and since when
    static ARMED: Cell<Option<(Action, Instant)>> = const { Cell::new(None) };
}

/// Forgets a first click; the page calls this as it's left.
pub(crate) fn reset() {
    ARMED.set(None);
}

/// The action still waiting for its second click, if its wait isn't over.
fn armed() -> Option<Action> {
    let (action, since) = ARMED.get()?;
    (since.elapsed() < ARMED_FOR).then_some(action)
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

    DetailPage::new("amane.qs.session.page", "Power Off").body(move |_| {
        let armed = armed();
        let items: Vec<ListItem> = actions
            .iter()
            .map(|&action| {
                let waiting = armed == Some(action);
                let label = if waiting {
                    action.confirm_text()
                } else {
                    action.label()
                };
                let mut item = ListItem::new(format!("amane.qs.session.{action:?}"), label)
                    .leading(action.glyph())
                    .on_press(move || match click(action, self::armed()) {
                        Some(armed) => {
                            ARMED.set(Some((armed, Instant::now())));
                            redraw_after(ARMED_FOR);
                            amane::mark_all();
                        }
                        None => {
                            ARMED.set(None);
                            action.run();
                        }
                    });
                if waiting {
                    item = item
                        .selected(true)
                        .color(sys::color::status::critical::text())
                        .trailing(list::trailing_text(
                            "Click again",
                            sys::color::status::critical::text(),
                        ));
                }
                item
            })
            .collect();
        vec![boxed(List::new(items).build())]
    })
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
