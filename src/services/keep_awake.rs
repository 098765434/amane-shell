//! Keep Awake — holds the screen on and, if asked, makes the laptop ignore
//! its lid, for as long as the user says.
//!
//! Nothing here writes a power setting: every hold is an inhibitor tied to
//! the shell itself, so a shell that dies lets go of everything and the
//! machine goes back to normal on its own. The screen is held through
//! `org.freedesktop.ScreenSaver` (niri serves it, and an inhibit there
//! keeps hypridle from dimming, locking or turning the monitors off), on
//! amane's one session connection, which the owner drops with the shell.
//! The lid is held by logind's `handle-lid-switch` block inhibitor, which
//! needs no root and lasts as long as the file descriptor it hands back is
//! open.
//!
//! A time limit is a wall-clock instant, so a two-hour hold that spans a
//! suspend has run out when the machine wakes. Locking hands the screen
//! back to the idle timers, so the lock screen blanks as it would without
//! this; the lid stays held, since a docked laptop shut while locked is the
//! case the lid hold is for. swaylock says nothing when it's unlocked, so
//! the screen hold comes back once its process is gone.

use std::os::fd::OwnedFd;
use std::thread;
use std::time::{Duration, SystemTime};

use amane::{Argument, Bus, Service};

use super::{Latest, run};

const SCREENSAVER: &str = "org.freedesktop.ScreenSaver";
const SCREENSAVER_PATH: &str = "/org/freedesktop/ScreenSaver";

const LOGIND: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION_PATH: &str = "/org/freedesktop/login1/session/auto";
const SESSION: &str = "org.freedesktop.login1.Session";

/// The locker hypridle runs on logind's Lock signal.
const LOCKER: &str = "swaylock";

/// How long the locker has to show up after the Lock signal.
const LOCKER_START: Duration = Duration::from_secs(5);

/// Before turning the screen off: the click's own release is input, and
/// would wake it straight back up.
const SCREEN_OFF_DELAY: Duration = Duration::from_secs(1);

const WHO: &str = "amane-shell";
const WHY: &str = "Keep Awake";

/// The time limits the page offers; `None` holds until turned off.
pub const LIMITS: [(&str, Option<Duration>); 5] = [
    ("No limit", None),
    ("15 min", Some(Duration::from_secs(15 * 60))),
    ("30 min", Some(Duration::from_secs(30 * 60))),
    ("1 h", Some(Duration::from_secs(60 * 60))),
    ("2 h", Some(Duration::from_secs(2 * 60 * 60))),
];

/// Takes and lets go of the inhibitors off the UI thread, one at a time.
static SYNC: Latest<()> = Latest::new();

pub struct KeepAwake {
    screen: bool,
    lid: bool,
    /// The limit picked from `LIMITS`, and when it runs out.
    limit: Option<Duration>,
    until: Option<SystemTime>,
    locked: bool,
    cookie: Option<u32>,
    lid_fd: Option<OwnedFd>,
    screen_present: bool,
    lid_present: bool,
}

impl Service for KeepAwake {
    fn new() -> Self {
        let screen_present = Bus::session()
            .call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                &[Argument::from(SCREENSAVER)],
            )
            .bool();
        let lid_present = Bus::system().property(LOGIND, MANAGER_PATH, MANAGER, "BlockInhibited")
            != amane::Value::Nothing;
        Self {
            screen: false,
            lid: false,
            limit: None,
            until: None,
            locked: false,
            cookie: None,
            lid_fd: None,
            screen_present,
            lid_present,
        }
    }

    // only a time limit changes by itself, and half a minute late is fine
    fn interval() -> Duration {
        Duration::from_secs(30)
    }

    fn update(&mut self) -> bool {
        if expired(self.until, SystemTime::now()) {
            self.release();
            SYNC.send((), sync);
            return true;
        }
        // the time left on the tile moves on
        self.until.is_some()
    }

    fn listen() {
        thread::spawn(watch_lock);
        loop {
            thread::sleep(Self::interval());
            // a write redraws whatever reads this, so only with a limit running
            if Self::read().until.is_some() {
                Self::write().update();
            }
        }
    }
}

impl KeepAwake {
    /// Whether anything is overridden: the tile lights for the lid too, so
    /// a dark tile always means the machine behaves as normal.
    pub fn on(&self) -> bool {
        self.screen || self.lid
    }

    pub fn screen(&self) -> bool {
        self.screen
    }

    pub fn lid(&self) -> bool {
        self.lid
    }

    pub fn limit(&self) -> Option<Duration> {
        self.limit
    }

    /// How long until the hold runs out, `None` without a limit.
    pub fn left(&self, now: SystemTime) -> Option<Duration> {
        self.until
            .map(|until| until.duration_since(now).unwrap_or_default())
    }

    /// Whether there's a screensaver service to ask.
    pub fn present(&self) -> bool {
        self.screen_present
    }

    /// Whether logind is there to hold the lid.
    pub fn lid_present(&self) -> bool {
        self.lid_present
    }

    /// The tile: on holds the screen with no limit; off lets go of
    /// everything, the lid too, so off always means nothing's overridden.
    pub fn set(on: bool) {
        {
            let mut keep = Self::write();
            if on {
                keep.screen = true;
                keep.limit = None;
                keep.until = None;
            } else {
                keep.release();
            }
        }
        SYNC.send((), sync);
    }

    pub fn set_screen(on: bool) {
        Self::write().hold(Some(on), None);
        SYNC.send((), sync);
    }

    pub fn set_lid(on: bool) {
        Self::write().hold(None, Some(on));
        SYNC.send((), sync);
    }

    /// Picking a limit from `LIMITS` starts it now, and holds the screen
    /// if nothing was held.
    pub fn set_limit(limit: Option<Duration>) {
        {
            let mut keep = Self::write();
            keep.limit = limit;
            keep.until = limit.map(|limit| SystemTime::now() + limit);
            if !keep.on() {
                keep.screen = true;
            }
        }
        SYNC.send((), sync);
    }

    /// Turns the monitors off without locking or letting go of anything;
    /// niri turns them back on at the next input.
    pub fn screen_off_now() {
        thread::spawn(|| {
            thread::sleep(SCREEN_OFF_DELAY);
            run(
                "niri",
                ["msg", "action", "power-off-monitors"]
                    .map(String::from)
                    .to_vec(),
            );
        });
    }

    /// One switch from the page; the last one off turns it all off.
    fn hold(&mut self, screen: Option<bool>, lid: Option<bool>) {
        self.screen = screen.unwrap_or(self.screen);
        self.lid = lid.unwrap_or(self.lid);
        if !self.on() {
            self.release();
        }
    }

    fn release(&mut self) {
        self.screen = false;
        self.lid = false;
        self.limit = None;
        self.until = None;
    }
}

fn expired(until: Option<SystemTime>, now: SystemTime) -> bool {
    until.is_some_and(|until| now >= until)
}

/// Takes or lets go of each inhibitor to match what's asked, without
/// holding the service while the bus answers.
fn sync((): ()) {
    let (screen, lid, mut cookie, mut fd) = {
        let mut keep = KeepAwake::write();
        let screen = keep.screen && !keep.locked;
        (screen, keep.lid, keep.cookie.take(), keep.lid_fd.take())
    };

    match (screen, cookie) {
        (true, None) => cookie = inhibit_screen(),
        (false, Some(held)) => {
            Bus::session().call(
                SCREENSAVER,
                SCREENSAVER_PATH,
                SCREENSAVER,
                "UnInhibit",
                &[Argument::from(held)],
            );
            cookie = None;
        }
        _ => {}
    }
    if !lid {
        // closing it is what lets go
        fd = None;
    } else if fd.is_none() {
        fd = inhibit_lid();
    }

    let mut keep = KeepAwake::write();
    keep.cookie = cookie;
    if lid && fd.is_none() {
        // logind said no: show it as off rather than held
        keep.hold(None, Some(false));
    }
    keep.lid_fd = fd;
}

fn inhibit_screen() -> Option<u32> {
    let cookie = Bus::session().call(
        SCREENSAVER,
        SCREENSAVER_PATH,
        SCREENSAVER,
        "Inhibit",
        &[Argument::from(WHO), Argument::from(WHY)],
    );
    match cookie {
        amane::Value::Number(cookie) => Some(cookie as u32),
        _ => {
            eprintln!("amane-shell: keep awake: the screensaver refused the inhibit");
            None
        }
    }
}

/// amane's bus calls don't carry file descriptors, so this one goes to
/// zbus itself.
fn inhibit_lid() -> Option<OwnedFd> {
    let answer = zbus::blocking::Connection::system().and_then(|bus| {
        let reply = bus.call_method(
            Some(LOGIND),
            MANAGER_PATH,
            Some(MANAGER),
            "Inhibit",
            &("handle-lid-switch", WHO, WHY, "block"),
        )?;
        let fd: zbus::zvariant::OwnedFd = reply.body().deserialize()?;
        Ok(OwnedFd::from(fd))
    });
    answer
        .inspect_err(|e| eprintln!("amane-shell: keep awake: lid: {e}"))
        .ok()
}

/// Lets the screen blank while the session is locked: on logind's Lock
/// signal for this session, until the locker has gone.
fn watch_lock() {
    let bus = Bus::system();
    let id = bus.property(LOGIND, SESSION_PATH, SESSION, "Id");
    if id.text().is_empty() {
        return;
    }
    let path = bus.call(
        LOGIND,
        MANAGER_PATH,
        MANAGER,
        "GetSession",
        &[Argument::from(id.text())],
    );
    let path = path.text().to_string();

    for signal in bus.signals(SESSION, "Lock") {
        if signal.path() != path {
            continue;
        }
        KeepAwake::write().locked = true;
        SYNC.send((), sync);

        wait_for_unlock();

        KeepAwake::write().locked = false;
        SYNC.send((), sync);
    }
}

fn wait_for_unlock() {
    let mut waited = Duration::ZERO;
    while !locker_running() {
        if waited >= LOCKER_START {
            return;
        }
        thread::sleep(Duration::from_secs(1));
        waited += Duration::from_secs(1);
    }
    while locker_running() {
        thread::sleep(Duration::from_secs(1));
    }
}

fn locker_running() -> bool {
    std::fs::read_dir("/proc").is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            std::fs::read_to_string(entry.path().join("comm"))
                .is_ok_and(|comm| comm.trim_end() == LOCKER)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keep() -> KeepAwake {
        KeepAwake {
            screen: false,
            lid: false,
            limit: None,
            until: None,
            locked: false,
            cookie: None,
            lid_fd: None,
            screen_present: true,
            lid_present: true,
        }
    }

    #[test]
    fn the_last_switch_off_lets_go_of_everything() {
        let mut keep = keep();
        keep.limit = LIMITS[1].1;
        keep.until = Some(SystemTime::now());
        keep.hold(Some(true), Some(true));
        keep.hold(Some(false), None);
        assert!(keep.on() && keep.lid);
        assert!(keep.until.is_some());

        keep.hold(None, Some(false));
        assert!(!keep.on());
        assert_eq!((keep.limit, keep.until), (None, None));
    }

    #[test]
    fn a_limit_runs_out_at_its_instant() {
        let now = SystemTime::now();
        assert!(!expired(None, now));
        assert!(!expired(Some(now + Duration::from_secs(1)), now));
        assert!(expired(Some(now), now));
        assert!(expired(Some(now - Duration::from_secs(60)), now));
    }

    #[test]
    fn whats_left_never_goes_below_zero() {
        let now = SystemTime::now();
        let mut keep = keep();
        assert_eq!(keep.left(now), None);
        keep.until = Some(now - Duration::from_secs(5));
        assert_eq!(keep.left(now), Some(Duration::ZERO));
    }
}
