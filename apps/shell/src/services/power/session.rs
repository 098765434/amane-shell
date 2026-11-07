//! Session — locking the screen, sleeping, restarting, powering off and
//! logging out, the power menu's actions, through logind.
//!
//! What logind allows is asked once: it changes with policy, not while the
//! shell runs. Locking goes through logind too (`Session.Lock`), so whatever
//! locker the session runs on its Lock signal (swaylock under hypridle, say)
//! shows. Logging out quits niri, which ends the session.

use amane::{Argument, Bus, Service};

use crate::services::run;

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION_PATH: &str = "/org/freedesktop/login1/session/auto";
const SESSION: &str = "org.freedesktop.login1.Session";

pub struct Session {
    suspend: bool,
    reboot: bool,
    power_off: bool,
}

impl Service for Session {
    fn new() -> Self {
        Self {
            suspend: can("CanSuspend"),
            reboot: can("CanReboot"),
            power_off: can("CanPowerOff"),
        }
    }

    fn listen() {}
}

/// logind answers "yes", "no", "challenge" (a password first) or "na".
fn can(method: &str) -> bool {
    let answer = Bus::system().call(LOGIND, MANAGER_PATH, MANAGER, method, &[]);
    allowed(answer.text())
}

fn allowed(answer: &str) -> bool {
    matches!(answer, "yes" | "challenge")
}

impl Session {
    pub fn can_suspend(&self) -> bool {
        self.suspend
    }

    pub fn can_reboot(&self) -> bool {
        self.reboot
    }

    pub fn can_power_off(&self) -> bool {
        self.power_off
    }

    pub fn lock() {
        std::thread::spawn(|| {
            Bus::system().call(LOGIND, SESSION_PATH, SESSION, "Lock", &[]);
        });
    }

    pub fn suspend() {
        manager("Suspend");
    }

    pub fn reboot() {
        manager("Reboot");
    }

    pub fn power_off() {
        manager("PowerOff");
    }

    pub fn log_out() {
        run(
            "niri",
            ["msg", "action", "quit", "--skip-confirmation"]
                .map(String::from)
                .to_vec(),
        );
    }
}

/// A Manager action, interactive so polkit may ask for a password.
fn manager(method: &'static str) {
    std::thread::spawn(move || {
        Bus::system().call(
            LOGIND,
            MANAGER_PATH,
            MANAGER,
            method,
            &[Argument::from(true)],
        );
    });
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
