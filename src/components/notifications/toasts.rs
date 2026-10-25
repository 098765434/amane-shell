//! Toasts — the banners that slide in at the top-right as notifications
//! arrive, and slide out again.
//!
//! Which toasts show is worked out fresh each time from the notification
//! list and the clock ([`alive`]): one shows from its arrival until its
//! lifetime runs out, unless the user swiped it away, Do Not Disturb is on
//! (critical ones still show), or the center is open. A small [`Toasts`]
//! service ticks to notice a lifetime running out and keeps the little the
//! list can't say: swiped toasts, and the pointer resting on one, which
//! holds them all.
//!
//! Each toast is its own layer surface (a fixed set of [`SLOTS`]), sized to
//! its card, so niri's blur frosts exactly the card; sliding and swiping
//! move the surface itself.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime};

use amane::{
    Horizontal, Keyboard, LayerWindow, Margin, Notification, Notifications, Point, Service,
    Timeout, Urgency, Vertical,
};

use super::card::{self, Measured, Place, critical};
use super::dnd::DoNotDisturb;
use crate::components::layout::blank;
use crate::components::motion::{self, Arrive, Item, Pose, Shown};
use crate::components::panel::{Panel, is_open, overlay};
use crate::tokens::{notification, sys};

/// Layer-surface namespace of every toast; the niri `layer-rule` matches it.
pub const TOAST_NS: &str = "amane.notifications.toast";

/// Toast surfaces: room for `notification.toast.max-visible` plus the ones
/// still sliding out.
pub const SLOTS: usize = 5;

/// One view per toast surface, for `App::window`.
pub const TOASTS: [fn() -> LayerWindow; SLOTS] =
    [|| slot(0), || slot(1), || slot(2), || slot(3), || slot(4)];

/// How long a toast shows when its sender leaves it to the shell.
const LIFETIME: Duration = Duration::from_secs(5);

/// How long toasts stay once the pointer leaves them.
const GRACE: Duration = Duration::from_secs(2);

/// How far a press may move and still be a click.
const SLOP: f32 = 4.0;

/// How much of its width a toast must be swiped to go.
const SWIPE_AWAY: f32 = 0.3;

/// A drag quieter than this is over (the pointer has no release event).
const SWIPE_IDLE: Duration = Duration::from_millis(250);

const SCOPE: &str = "toasts";

// ───── state ─────────────────────────────────────────────────────────────────

pub struct Toasts {
    /// Swiped away (or put away by opening the center); keyed with the
    /// arrival time, so a sender replacing it shows it again.
    hidden: HashSet<(u32, SystemTime)>,
    /// The pointer is on a toast, which holds every shown one.
    hovered: bool,
    /// How long each toast stays after the pointer left, at the least.
    held: HashMap<u32, SystemTime>,
    /// What showed at the last tick.
    shown: Vec<u32>,
}

impl Service for Toasts {
    fn new() -> Self {
        Self {
            hidden: HashSet::new(),
            hovered: false,
            held: HashMap::new(),
            shown: Vec::new(),
        }
    }

    // a lifetime ends at most a tenth of a second late
    fn interval() -> Duration {
        Duration::from_millis(100)
    }

    fn update(&mut self) -> bool {
        let now = SystemTime::now();
        let dnd = DoNotDisturb::read().on();

        let (shown, finished) = {
            let notifications = Notifications::read();
            let list = notifications.list();

            // forget what belongs to notifications that are gone
            let ids: HashSet<u32> = list.iter().map(Notification::id).collect();
            self.hidden.retain(|(id, _)| ids.contains(id));
            self.held.retain(|id, _| ids.contains(id));

            let shown: Vec<u32> = list
                .iter()
                .filter(|n| self.shows(n, now, dnd))
                .map(Notification::id)
                .collect();
            // a transient notification is done once its toast is
            let finished: Vec<u32> = list
                .iter()
                .filter(|n| n.transient() && !self.alive(n, now))
                .map(Notification::id)
                .collect();
            (shown, finished)
        };

        for id in finished {
            Notifications::expire(id);
        }

        if shown == self.shown {
            return false;
        }
        self.shown = shown;
        true
    }
}

impl Toasts {
    fn alive(&self, n: &Notification, now: SystemTime) -> bool {
        alive(Toast::from(n), now, self)
    }

    /// Whether `n`'s toast is up: alive, and let through Do Not Disturb.
    fn shows(&self, n: &Notification, now: SystemTime, dnd: bool) -> bool {
        self.alive(n, now) && (!dnd || critical(n))
    }

    /// The pointer came onto a toast (`true`) or left them.
    pub fn hover(inside: bool) {
        if Self::read().hovered == inside {
            return;
        }
        let mut toasts = Self::write();
        toasts.hovered = inside;
        if !inside {
            let until = SystemTime::now() + GRACE;
            for id in toasts.shown.clone() {
                toasts.held.insert(id, until);
            }
        }
    }

    /// Puts one toast away; it stays in the center.
    pub fn hide(id: u32, received: SystemTime) {
        Self::write().hidden.insert((id, received));
    }

    /// Puts every toast away, as the center opens.
    pub fn hide_all() {
        let all: Vec<(u32, SystemTime)> = Notifications::read()
            .list()
            .iter()
            .map(|n| (n.id(), n.received()))
            .collect();
        let mut toasts = Self::write();
        toasts.hidden.extend(all);
        toasts.hovered = false;
    }
}

/// What decides whether one notification's toast is up.
#[derive(Clone, Copy)]
struct Toast {
    id: u32,
    received: SystemTime,
    timeout: Timeout,
    urgency: Urgency,
}

impl From<&Notification> for Toast {
    fn from(n: &Notification) -> Self {
        Self {
            id: n.id(),
            received: n.received(),
            timeout: n.timeout(),
            urgency: n.urgency(),
        }
    }
}

/// How long a toast shows: what the sender asked for, the shell's default,
/// or (`None`) until the user acts — always so for a critical one.
fn lifetime(timeout: Timeout, urgency: Urgency) -> Option<Duration> {
    if urgency == Urgency::Critical {
        return None;
    }
    match timeout {
        Timeout::After(duration) => Some(duration),
        Timeout::Default => Some(LIFETIME),
        Timeout::Never => None,
    }
}

/// Whether `toast` is up at `now`, Do Not Disturb aside.
fn alive(toast: Toast, now: SystemTime, state: &Toasts) -> bool {
    if state.hidden.contains(&(toast.id, toast.received)) {
        return false;
    }
    if state.hovered && state.shown.contains(&toast.id) {
        return true;
    }
    let Some(lifetime) = lifetime(toast.timeout, toast.urgency) else {
        return true;
    };
    let end = toast.received + lifetime;
    let end = state.held.get(&toast.id).map_or(end, |held| end.max(*held));
    now < end
}

// ───── layout ────────────────────────────────────────────────────────────────

/// The toasts to draw this frame: the newest few, top to bottom, and any
/// still sliding out, each with its surface slot.
fn frame(now: SystemTime) -> Vec<(usize, Shown<Measured>)> {
    let dnd = DoNotDisturb::read().on();
    let center = is_open(Panel::Notifications);

    // one service lock at a time: the Toasts tick holds its own while it
    // reads and expires notifications
    let all: Vec<Notification> = Notifications::read().list().to_vec();
    let max_visible = notification::toast::max_visible() as usize;
    // the rest of the slots are for toasts still sliding out
    debug_assert!(
        max_visible < SLOTS,
        "notification.toast.max-visible needs a spare slot"
    );
    let list: Vec<Notification> = {
        let toasts = Toasts::read();
        all.into_iter()
            .rev()
            .filter(|n| !center && toasts.shows(n, now, dnd))
            .take(max_visible)
            .collect()
    };

    let off = Place::Toast.width() + sys::space::gap::m();
    let mut y = 0.0;
    let items = list
        .into_iter()
        .map(|n| {
            let m = Measured::new(n, Place::Toast);
            let height = m.layout.height;
            let pose = Pose::at(y);
            y += height + sys::space::gap::s();
            Item {
                key: u64::from(m.notification.id()),
                value: m,
                height,
                pose,
                enter: pose.moved(off),
                exit: pose.moved(off),
            }
        })
        .collect();

    let shown = motion::animate(SCOPE, items, Arrive::Enter);
    assign(shown)
}

thread_local! {
    // which toast each surface shows, so a toast keeps its surface
    static SLOT_KEYS: RefCell<[Option<u64>; SLOTS]> = const { RefCell::new([None; SLOTS]) };
}

fn assign(shown: Vec<Shown<Measured>>) -> Vec<(usize, Shown<Measured>)> {
    SLOT_KEYS.with_borrow_mut(|slots| {
        for slot in slots.iter_mut() {
            if slot.is_some_and(|key| !shown.iter().any(|s| s.key == key)) {
                *slot = None;
            }
        }
        shown
            .into_iter()
            .filter_map(|s| {
                let index = match slots.iter().position(|slot| *slot == Some(s.key)) {
                    Some(index) => index,
                    None => {
                        let free = slots.iter().position(Option::is_none)?;
                        slots[free] = Some(s.key);
                        free
                    }
                };
                Some((index, s))
            })
            .collect()
    })
}

// ───── swipe ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Swipe {
    id: u32,
    // where the press landed, surface-local
    grab: f32,
    // how far right the toast has been pulled
    offset: f32,
    at: Instant,
}

thread_local! {
    static SWIPE: Cell<Option<Swipe>> = const { Cell::new(None) };
}

fn swipe_offset(id: u32) -> f32 {
    SWIPE
        .with(Cell::get)
        .filter(|swipe| swipe.id == id)
        .map_or(0.0, |swipe| swipe.offset)
}

/// A drag on toast `id`. The surface moves with the pull, so the point
/// (surface-local) is turned back into a distance from where it was
/// grabbed.
fn drag(id: u32, point: Point) {
    let now = Instant::now();
    let swipe = match SWIPE.with(Cell::get) {
        Some(swipe) if swipe.id == id && now - swipe.at < SWIPE_IDLE => Swipe {
            offset: (point.x + swipe.offset - swipe.grab).max(0.0),
            at: now,
            ..swipe
        },
        _ => Swipe {
            id,
            grab: point.x,
            offset: 0.0,
            at: now,
        },
    };
    SWIPE.with(|s| s.set(Some(swipe)));
}

/// Lets go of a swipe: far enough and the toast goes, else it springs
/// back. Returns whether there was a swipe (and so no click).
fn let_go(id: u32, pose: Pose, received: SystemTime) -> bool {
    let Some(swipe) = SWIPE.with(Cell::take).filter(|swipe| swipe.id == id) else {
        return false;
    };
    if swipe.offset <= SLOP {
        return false;
    }
    motion::jump(SCOPE, u64::from(id), pose.moved(swipe.offset));
    if swipe.offset > Place::Toast.width() * SWIPE_AWAY {
        Toasts::hide(id, received);
    }
    true
}

// ───── view ──────────────────────────────────────────────────────────────────

fn slot(index: usize) -> LayerWindow {
    let now = SystemTime::now();
    let shown = frame(now).into_iter().find(|(slot, _)| *slot == index);

    let base = LayerWindow::new().width(Place::Toast.width());
    let Some((_, toast)) = shown else {
        let base = base.height(1.0).visible(false);
        return overlay(base, TOAST_NS, Vertical::Top, Horizontal::Right).child(blank());
    };

    let m = toast.value;
    let n = &m.notification;
    let id = n.id();
    let received = n.received();
    let pose = toast.pose;
    let gap = sys::space::gap::m();
    let offset = swipe_offset(id);

    let mut view = card::card(&m, Place::Toast, now, 1.0);
    if !toast.leaving {
        view = card::interactive(
            view,
            id,
            move |inside| {
                Toasts::hover(inside);
                if !inside {
                    let_go(id, pose, received);
                }
            },
            move || {
                if !let_go(id, pose, received) {
                    Notifications::click(id);
                }
            },
        )
        .on_drag(move |point| drag(id, point));
    }

    let base = base
        .height(toast.height)
        .margin(Margin {
            top: (gap + pose.y).round() as i32,
            right: (gap - pose.x - offset).round() as i32,
            bottom: 0,
            left: 0,
        })
        // None so a toast never takes focus from the app being typed in
        .keyboard(Keyboard::None)
        .visible(true);
    overlay(base, TOAST_NS, Vertical::Top, Horizontal::Right).child(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: SystemTime = SystemTime::UNIX_EPOCH;

    fn toast(id: u32, timeout: Timeout, urgency: Urgency) -> Toast {
        Toast {
            id,
            received: T0,
            timeout,
            urgency,
        }
    }

    fn at(secs: u64) -> SystemTime {
        T0 + Duration::from_secs(secs)
    }

    #[test]
    fn a_toast_lives_its_lifetime() {
        let state = Toasts::new();
        let normal = toast(1, Timeout::Default, Urgency::Normal);
        let short = toast(2, Timeout::After(Duration::from_secs(2)), Urgency::Normal);
        let forever = toast(3, Timeout::Never, Urgency::Normal);
        let critical = toast(4, Timeout::After(Duration::from_secs(1)), Urgency::Critical);

        assert!(alive(normal, at(4), &state));
        assert!(!alive(normal, at(5), &state));
        assert!(alive(short, at(1), &state));
        assert!(!alive(short, at(2), &state));
        assert!(alive(forever, at(3600), &state));
        assert!(alive(critical, at(3600), &state));
    }

    #[test]
    fn a_swiped_toast_stays_away_until_replaced() {
        let mut state = Toasts::new();
        state.hidden.insert((1, T0));

        let swiped = toast(1, Timeout::Never, Urgency::Normal);
        assert!(!alive(swiped, at(0), &state));

        let replaced = Toast {
            received: at(10),
            ..swiped
        };
        assert!(alive(replaced, at(10), &state));
    }

    #[test]
    fn hovering_holds_the_shown_toasts() {
        let mut state = Toasts::new();
        let normal = toast(1, Timeout::Default, Urgency::Normal);
        state.hovered = true;
        state.shown = vec![1];
        assert!(alive(normal, at(60), &state));

        // after the pointer leaves, it stays for the grace time
        state.hovered = false;
        state.held.insert(1, at(62));
        assert!(alive(normal, at(61), &state));
        assert!(!alive(normal, at(62), &state));
    }
}
