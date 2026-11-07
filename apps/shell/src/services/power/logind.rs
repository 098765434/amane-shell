//! The shell's one logind client. What amane's `Logind` has (the session
//! locking, the computer going to sleep, inhibitors) comes from it; the few
//! calls it lacks (what policy allows, sleeping and restarting, locking the
//! session, a light's brightness) are here, so no other module names
//! logind's bus paths.

use std::os::fd::OwnedFd;
use std::sync::{Mutex, PoisonError};

use amane::{Argument, Bus, Logind, Service};

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION_PATH: &str = "/org/freedesktop/login1/session/auto";
const SESSION: &str = "org.freedesktop.login1.Session";

/// Something the power menu asks logind to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Suspend,
    Reboot,
    PowerOff,
}

impl Action {
    fn method(self) -> &'static str {
        match self {
            Action::Suspend => "Suspend",
            Action::Reboot => "Reboot",
            Action::PowerOff => "PowerOff",
        }
    }
}

/// Whether policy lets this user do `action`; asked of logind, so call it
/// from a service's thread.
pub fn can(action: Action) -> bool {
    let answer = Bus::system().call(
        LOGIND,
        MANAGER_PATH,
        MANAGER,
        &format!("Can{}", action.method()),
        &[],
    );
    allowed(answer.text())
}

/// logind answers "yes", "no", "challenge" (a password first) or "na".
fn allowed(answer: &str) -> bool {
    matches!(answer, "yes" | "challenge")
}

/// Does `action`, off the UI thread; interactive, so polkit may ask for a
/// password.
pub fn power(action: Action) {
    std::thread::spawn(move || {
        Bus::system().call(
            LOGIND,
            MANAGER_PATH,
            MANAGER,
            action.method(),
            &[Argument::from(true)],
        );
    });
}

/// Asks logind to lock this session, off the UI thread: whatever locker the
/// session runs on its Lock signal shows.
pub fn lock_session() {
    std::thread::spawn(|| {
        Bus::system().call(LOGIND, SESSION_PATH, SESSION, "Lock", &[]);
    });
}

/// Sets the light `name` (under `/sys/class/<subsystem>`) to `level`,
/// which logind may do for the session's user.
pub fn set_brightness(subsystem: &str, name: &str, level: u32) {
    let arguments = [
        Argument::from(subsystem),
        Argument::from(name),
        Argument::from(level),
    ];
    Bus::system().call(LOGIND, SESSION_PATH, SESSION, "SetBrightness", &arguments);
}

/// Whether logind is there to hold inhibitors.
pub fn can_inhibit() -> bool {
    Bus::system().property(LOGIND, MANAGER_PATH, MANAGER, "BlockInhibited") != amane::Value::Nothing
}

/// Holds `what` off (as `handle-lid-switch`) until the descriptor is dropped.
pub fn inhibit(what: &str, who: &str, why: &str) -> Option<OwnedFd> {
    Logind::inhibit(what, who, why, "block")
        .inspect_err(|error| log::warn!("couldn't hold off {what}: {error}"))
        .ok()
}

/// Runs `on_lock` each time the session is asked to lock, and `on_sleep`
/// each time the computer is about to sleep, on amane's `Logind` thread:
/// for long work, start a thread of its own.
pub fn follow(
    on_lock: impl Fn() + Send + Sync + 'static,
    on_sleep: impl Fn() + Send + Sync + 'static,
) {
    // what was seen last, so each callback runs once a time; a subscription only hears
    // changes, so the service is started (peeked at, not watched) and its start is what's seen
    let seen = {
        let logind = Logind::peek();
        Mutex::new(Some((logind.lock_requests(), logind.preparing_for_sleep())))
    };
    Logind::subscribe(move |logind| {
        let now = (logind.lock_requests(), logind.preparing_for_sleep());
        let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
        let before = seen.replace(now);
        let Some((locks, sleeping)) = before else {
            return;
        };
        if now.0 > locks {
            on_lock();
        }
        if now.1 && !sleeping {
            on_sleep();
        }
    })
    .forever();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_challenge_still_offers_the_action() {
        assert!(allowed("yes"));
        assert!(allowed("challenge"));
        assert!(!allowed("no"));
        assert!(!allowed("na"));
        // no logind at all
        assert!(!allowed(""));
    }
}
