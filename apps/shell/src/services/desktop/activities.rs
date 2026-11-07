//! Activities other programs put in the bar: a timer, an upload, a
//! recording. They arrive over the bar's
//! bus service (`org.amane.StatusBar1`) and are kept here, each by a handle;
//! the bar draws them beside its own (Now Playing, the timer, screen
//! sharing), which it reads straight from their services.
//!
//! An activity ends when its program ends it, after eight hours, or when
//! its program leaves the bus. Ending it "after" a few seconds keeps its
//! last state up that long. amane has no timers, so threads sleep those out.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use amane::status_item::{ActivityContent, ActivityEvent, Dismiss};
use amane_kit::settings::remember_activity;

amane_kit::marker! {
    /// An activity started, changed or ended: the bar draws again.
    pub Activities
}

/// The longest an activity lasts.
pub const MAX_AGE: Duration = Duration::from_secs(8 * 60 * 60);

/// How long an alerting update keeps the expanded view up.
pub const ALERT_FOR: Duration = Duration::from_secs(4);

/// Where an activity's buttons and its leaving are told.
pub type Sink = Arc<dyn Fn(ActivityEvent) + Send + Sync>;

/// One activity going on.
#[derive(Clone)]
pub struct Live {
    pub handle: u32,
    /// Who may change it: a program's bus name, like ":1.42".
    pub owner: String,
    pub content: ActivityContent,
    pub started: Instant,
    pub updated: Instant,
    /// It was ended and leaves then, showing its last state till then.
    pub leaving: Option<Instant>,
    sink: Option<Sink>,
}

impl Live {
    /// Whether it went longer without an update than it said it would.
    pub fn stale(&self, now: Instant) -> bool {
        self.content
            .stale_after
            .is_some_and(|after| now.duration_since(self.updated) >= Duration::from_secs(after))
    }
}

#[derive(Default)]
struct Registry {
    next: u32,
    live: Vec<Live>,
    /// The activity whose update alerted, and until when.
    alert: Option<(u32, Instant)>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    next: 0,
    live: Vec::new(),
    alert: None,
});

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts an activity for `owner`, telling `sink` of its buttons; its
/// handle.
pub fn start(owner: &str, content: ActivityContent, sink: Option<Sink>) -> u32 {
    remember_activity(&content.app);
    let now = Instant::now();
    let stale = content.stale_after;
    let handle = {
        let mut registry = registry();
        registry.next = registry.next.wrapping_add(1).max(1);
        let handle = registry.next;
        registry.live.push(Live {
            handle,
            owner: owner.to_string(),
            content,
            started: now,
            updated: now,
            leaving: None,
            sink,
        });
        handle
    };
    after(MAX_AGE, move || {
        let old = registry()
            .live
            .iter()
            .any(|live| live.handle == handle && live.started == now);
        if old {
            remove(handle);
        }
    });
    redraw_when_stale(stale);
    Activities::changed();
    handle
}

/// Changes `owner`'s activity `handle`; `alert` brings its expanded view
/// up for a moment. False when it has none by that handle.
pub fn update(owner: &str, handle: u32, content: ActivityContent, alert: bool) -> bool {
    let stale = content.stale_after;
    {
        let mut registry = registry();
        let Some(live) = registry
            .live
            .iter_mut()
            .find(|live| live.handle == handle && live.owner == owner && live.leaving.is_none())
        else {
            return false;
        };
        live.content = content;
        live.updated = Instant::now();
        if alert {
            registry.alert = Some((handle, Instant::now() + ALERT_FOR));
            after(ALERT_FOR, Activities::changed);
        }
    }
    redraw_when_stale(stale);
    Activities::changed();
    true
}

/// Ends `owner`'s activity `handle`, now or after its last state has shown
/// a while. False when it has none by that handle.
pub fn end(owner: &str, handle: u32, dismiss: Dismiss) -> bool {
    let wait = match dismiss {
        Dismiss::Now => Duration::ZERO,
        Dismiss::After(seconds) => Duration::from_secs(u64::from(seconds.min(60 * 60))),
    };
    {
        let mut registry = registry();
        let Some(live) = registry
            .live
            .iter_mut()
            .find(|live| live.handle == handle && live.owner == owner)
        else {
            return false;
        };
        if !wait.is_zero() {
            live.leaving = Some(Instant::now() + wait);
        }
    }
    if wait.is_zero() {
        remove(handle);
    } else {
        after(wait, move || remove(handle));
        Activities::changed();
    }
    true
}

/// Ends every activity of a program that left the bus, telling nobody.
pub fn end_all_of(owner: &str) {
    let mut registry = registry();
    let before = registry.live.len();
    registry.live.retain(|live| live.owner != owner);
    if registry.live.len() != before {
        drop(registry);
        Activities::changed();
    }
}

/// Takes an activity out as the user asked, telling its program.
pub fn dismiss(handle: u32) {
    remove(handle);
}

/// Takes the activity out and tells its program it left.
fn remove(handle: u32) {
    let removed = {
        let mut registry = registry();
        let index = registry.live.iter().position(|live| live.handle == handle);
        index.map(|index| registry.live.remove(index))
    };
    let Some(removed) = removed else {
        return;
    };
    if let Some(sink) = removed.sink {
        sink(ActivityEvent::Dismissed);
    }
    Activities::changed();
}

/// Every activity going on, oldest first; the bar redraws when they change.
pub fn list() -> Vec<Live> {
    Activities::watch();
    registry().live.clone()
}

/// The activity whose update is alerting now, if one is.
pub fn alerting() -> Option<u32> {
    Activities::watch();
    let registry = registry();
    let (handle, until) = registry.alert?;
    (Instant::now() < until && registry.live.iter().any(|live| live.handle == handle))
        .then_some(handle)
}

/// Stops an alert showing, as when its view is closed.
pub fn quiet_alert() {
    registry().alert = None;
}

/// One of `handle`'s buttons was pressed: its program hears.
pub fn act(handle: u32, button: &str) {
    let sink = registry()
        .live
        .iter()
        .find(|live| live.handle == handle)
        .and_then(|live| live.sink.clone());
    if let Some(sink) = sink {
        let button = button.to_string();
        // on a thread, so a slow bus never holds up the click
        std::thread::spawn(move || sink(ActivityEvent::Action(button)));
    }
}

/// Milliseconds since 1970 now, as a countdown's end is given.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

/// The bar draws again once an activity could have gone stale.
fn redraw_when_stale(stale_after: Option<u64>) {
    if let Some(after_secs) = stale_after {
        after(Duration::from_secs(after_secs), Activities::changed);
    }
}

fn after(wait: Duration, then: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        std::thread::sleep(wait);
        then();
    });
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use amane::status_item::ItemIcon;

    use super::*;

    fn content(title: &str) -> ActivityContent {
        ActivityContent::new("", ItemIcon::default(), title)
    }

    #[test]
    fn only_the_owner_changes_or_ends_it() {
        let handle = start(":1.1", content("Tea"), None);
        assert!(!update(":1.2", handle, content("Coffee"), false));
        assert!(update(":1.1", handle, content("Green tea"), false));
        assert!(!end(":1.2", handle, Dismiss::Now));
        let live = list().into_iter().find(|live| live.handle == handle);
        assert_eq!(
            live.map(|live| live.content.title).as_deref(),
            Some("Green tea")
        );
        assert!(end(":1.1", handle, Dismiss::Now));
        assert!(!list().iter().any(|live| live.handle == handle));
    }

    #[test]
    fn ending_later_keeps_it_and_tells_once_it_goes() {
        let told = Arc::new(AtomicU32::new(0));
        let sink_told = told.clone();
        let sink: Sink = Arc::new(move |event| {
            if event == ActivityEvent::Dismissed {
                sink_told.fetch_add(1, Ordering::SeqCst);
            }
        });
        let handle = start(":1.3", content("Upload"), Some(sink));
        assert!(end(":1.3", handle, Dismiss::After(5)));
        let live = list().into_iter().find(|live| live.handle == handle);
        assert!(live.is_some_and(|live| live.leaving.is_some()));
        // an ended activity takes no more updates
        assert!(!update(":1.3", handle, content("Again"), false));
        remove(handle);
        assert_eq!(told.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_program_leaving_takes_its_activities() {
        let first = start(":1.4", content("A"), None);
        let second = start(":1.4", content("B"), None);
        let other = start(":1.5", content("C"), None);
        end_all_of(":1.4");
        let handles: Vec<u32> = list().iter().map(|live| live.handle).collect();
        assert!(!handles.contains(&first) && !handles.contains(&second));
        assert!(handles.contains(&other));
        end_all_of(":1.5");
    }

    #[test]
    fn stale_after_its_time_without_an_update() {
        let mut stale = content("Build");
        stale.stale_after = Some(60);
        let now = Instant::now();
        let live = Live {
            handle: 0,
            owner: String::new(),
            content: stale,
            started: now,
            updated: now,
            leaving: None,
            sink: None,
        };
        assert!(!live.stale(now + Duration::from_secs(59)));
        assert!(live.stale(now + Duration::from_secs(60)));
    }
}
