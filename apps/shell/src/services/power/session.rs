//! Session — locking the screen, sleeping, restarting, powering off and
//! logging out, the power menu's actions, through logind.
//!
//! What logind allows is asked once: it changes with policy, not while the
//! shell runs. Locking goes through logind too (`Session.Lock`), so whatever
//! locker the session runs on its Lock signal (swaylock under hypridle, say)
//! shows. Logging out quits niri, which ends the session.

use amane::Service;

use super::logind::{self, Action};

pub struct Session {
    suspend: bool,
    reboot: bool,
    power_off: bool,
}

impl Service for Session {
    fn new() -> Self {
        Self {
            suspend: logind::can(Action::Suspend),
            reboot: logind::can(Action::Reboot),
            power_off: logind::can(Action::PowerOff),
        }
    }

    fn listen() {}
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
        logind::lock_session();
    }

    pub fn suspend() {
        logind::power(Action::Suspend);
    }

    pub fn reboot() {
        logind::power(Action::Reboot);
    }

    pub fn power_off() {
        logind::power(Action::PowerOff);
    }

    pub fn log_out() {
        std::thread::spawn(|| {
            if let Err(err) = amane::niri::quit() {
                log::warn!("couldn't log out: {err}");
            }
        });
    }
}
