use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Once};
use std::thread;

use amane::{Argument, Bus, BusError, run_in_order};

use crate::protocol::{
    ActivityContent, ActivityEvent, BUS_NAME, Dismiss, Hello, INTERFACE, ItemEvent, ItemSpec, PATH,
    ShellInfo, encode,
};
use emaki_base::sync::Locked as _;

type Handler<T> = Arc<dyn Fn(T) + Send + Sync>;

struct Activity {
    content: ActivityContent,

    // the bar's number for it, none until it answered or while no bar runs
    handle: Option<u32>,

    // ended here: it isn't started again when the bar comes back
    ended: bool,

    on_event: Option<Handler<ActivityEvent>>,
}

#[derive(Default)]
struct Client {
    items: HashMap<String, ItemSpec>,
    item_handlers: HashMap<String, Handler<ItemEvent>>,

    activities: HashMap<u64, Activity>,
    next: u64,

    // who this program said it is, said again to every new bar, and what that bar answered
    hello: Option<Hello>,
    shell: Option<ShellInfo>,
}

// what this program has in the bar, kept so it can be sent again when the bar restarts
static CLIENT: LazyLock<Mutex<Client>> = LazyLock::new(Mutex::default);

static LISTENING: Once = Once::new();

fn client() -> MutexGuard<'static, Client> {
    CLIENT.locked()
}

fn call(method: &str, arguments: &[Argument]) -> Result<amane::Value, BusError> {
    Bus::session().try_call(BUS_NAME, PATH, INTERFACE, method, arguments)
}

/// Tells the shell who this program is, `id` reverse-DNS like "org.emaki.drop", and gives back
/// what it serves: its level, the kinds it takes, the user's locale and the bar's size. Said
/// again to every bar that starts. None while no bar runs; a bar from before `Hello` answers
/// as [`ShellInfo::default`]. Optional: items and activities work without it.
pub fn hello(id: &str) -> Option<ShellInfo> {
    listen();
    client().hello = Some(Hello::new(id));
    say_hello()
}

/// What the shell answered the last [`hello`], if one was said and a bar answered it.
pub fn shell_info() -> Option<ShellInfo> {
    client().shell.clone()
}

fn say_hello() -> Option<ShellInfo> {
    let hello = client().hello.clone()?;
    let shell = match call("Hello", &[Argument::Text(encode(&hello))]) {
        Ok(reply) => ShellInfo::decode(reply.text()).unwrap_or_else(|error| {
            log::warn!("the shell's answer to hello didn't read: {error}");
            ShellInfo::default()
        }),
        // a bar from before hello: the bus's own answer, or the bar's "no method"
        Err(error)
            if error.name() == "org.freedesktop.DBus.Error.UnknownMethod"
                || error.name().ends_with(".Error.Unknown") =>
        {
            ShellInfo::default()
        }
        Err(error) if error.name().ends_with(".Error.Level") => {
            log::warn!(
                "the shell doesn't serve this program's level: {}",
                error.message()
            );
            return None;
        }
        Err(_) => return None,
    };
    client().shell = Some(shell.clone());
    Some(shell)
}

/// A program's own item in the bar:
///
/// ```ignore
/// let item = StatusItem::register(ItemSpec::new("org.emaki.drop", icon));
/// item.on_event(|event| ...);
/// ```
///
/// Calls go out on amane's worker in order, and events come in on a thread of
/// their own, so a handler that touches a window writes a Service and lets
/// that redraw. When the bar restarts, every item is registered again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusItem {
    id: String,
}

impl StatusItem {
    /// Puts the item in the bar and keeps it there, registering it again whenever the
    /// bar restarts; with no bar running yet, it shows once one starts.
    pub fn register(spec: ItemSpec) -> Self {
        listen();

        let id = spec.id.clone();

        client().items.insert(id.clone(), spec.clone());

        run_in_order(move || {
            // no bar yet: items are registered again when one appears
            let _ = call("RegisterItem", &[Argument::Text(encode(&spec))]);
        });

        Self { id }
    }

    /// The id it was registered with.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The whole item again; its id stays the one it was registered with.
    pub fn update(&self, mut spec: ItemSpec) {
        spec.id = self.id.clone();

        client().items.insert(self.id.clone(), spec.clone());

        run_in_order(move || {
            // no bar yet: items are registered again when one appears
            let _ = call("UpdateItem", &[Argument::Text(encode(&spec))]);
        });
    }

    /// What it shows now, to change a part of it.
    pub fn spec(&self) -> Option<ItemSpec> {
        client().items.get(&self.id).cloned()
    }

    /// Calls `handler` on the client's listening thread for each of the item's events,
    /// replacing any handler set before.
    pub fn on_event(&self, handler: impl Fn(ItemEvent) + Send + Sync + 'static) {
        client()
            .item_handlers
            .insert(self.id.clone(), Arc::new(handler));
    }

    /// Takes the item out of the bar for good, and its handler with it.
    pub fn remove(self) {
        {
            let mut client = client();

            client.items.remove(&self.id);
            client.item_handlers.remove(&self.id);
        }

        let id = self.id;

        run_in_order(move || {
            // no bar: nothing to remove
            let _ = call("RemoveItem", &[Argument::Text(id)]);
        });
    }
}

/// An activity in the bar, something going on now: started, updated as it goes,
/// and ended. An update that alerts briefly shows the expanded view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveActivity {
    local: u64,
}

impl LiveActivity {
    /// Shows a new activity in the bar, started again whenever the bar restarts until
    /// it's ended.
    pub fn start(content: ActivityContent) -> Self {
        listen();

        let local = {
            let mut client = client();

            client.next += 1;

            let local = client.next;

            client.activities.insert(
                local,
                Activity {
                    content: content.clone(),
                    handle: None,
                    ended: false,
                    on_event: None,
                },
            );

            local
        };

        run_in_order(move || start_on_bar(local, &content));

        Self { local }
    }

    /// Replaces what the activity shows; with `alert`, the bar briefly shows its
    /// expanded view. Does nothing once it has ended.
    pub fn update(&self, content: ActivityContent, alert: bool) {
        {
            let mut client = client();

            let Some(activity) = client.activities.get_mut(&self.local) else {
                return;
            };

            if activity.ended {
                return;
            }

            activity.content = content.clone();
        }

        let local = self.local;

        // after start's call on the same worker, so its handle is known by now
        run_in_order(move || {
            let Some(handle) = handle_of(local) else {
                return;
            };

            // no bar yet: activities start again when one appears
            let _ = call(
                "UpdateActivity",
                &[
                    Argument::Unsigned(handle),
                    Argument::Text(encode(&content)),
                    Argument::Bool(alert),
                ],
            );
        });
    }

    /// What it shows now, to change a part of it; none once it's gone.
    pub fn content(&self) -> Option<ActivityContent> {
        let client = client();

        let activity = client.activities.get(&self.local)?;

        (!activity.ended).then(|| activity.content.clone())
    }

    /// Calls `handler` on the client's listening thread for each of the activity's
    /// events, replacing any handler set before.
    pub fn on_event(&self, handler: impl Fn(ActivityEvent) + Send + Sync + 'static) {
        if let Some(activity) = client().activities.get_mut(&self.local) {
            activity.on_event = Some(Arc::new(handler));
        }
    }

    /// Its Dismissed event still comes, once it has left the bar.
    pub fn end(self, dismiss: Dismiss) {
        {
            let mut client = client();

            let Some(activity) = client.activities.get_mut(&self.local) else {
                return;
            };

            activity.ended = true;
        }

        let local = self.local;

        run_in_order(move || {
            let Some(handle) = handle_of(local) else {
                client().activities.remove(&local);

                return;
            };

            // no bar: nothing to end
            let _ = call(
                "EndActivity",
                &[Argument::Unsigned(handle), Argument::Text(encode(&dismiss))],
            );
        });
    }
}

fn handle_of(local: u64) -> Option<u32> {
    client().activities.get(&local)?.handle
}

fn start_on_bar(local: u64, content: &ActivityContent) {
    let Ok(reply) = call("StartActivity", &[Argument::Text(encode(content))]) else {
        return;
    };

    let handle = reply.number() as u32;

    if let Some(activity) = client().activities.get_mut(&local) {
        activity.handle = Some(handle);
    }
}

// everything this program had in the bar, sent again to a bar that just started
fn register_again() {
    say_hello();

    let (items, activities) = {
        let mut client = client();

        let items: Vec<ItemSpec> = client.items.values().cloned().collect();

        client.activities.retain(|_, activity| !activity.ended);

        let activities: Vec<(u64, ActivityContent)> = client
            .activities
            .iter_mut()
            .map(|(local, activity)| {
                activity.handle = None;

                (*local, activity.content.clone())
            })
            .collect();

        (items, activities)
    };

    for spec in items {
        // no bar yet: they go again when one appears
        let _ = call("RegisterItem", &[Argument::Text(encode(&spec))]);
    }

    for (local, content) in activities {
        start_on_bar(local, &content);
    }
}

// the bar quit: nothing it numbered means anything now
fn forget_handles() {
    let mut client = client();

    client.shell = None;

    client.activities.retain(|_, activity| !activity.ended);

    for activity in client.activities.values_mut() {
        activity.handle = None;
    }
}

fn listen() {
    LISTENING.call_once(|| {
        thread::spawn(|| {
            for signal in Bus::session().signals_from(BUS_NAME) {
                if signal.interface() != INTERFACE {
                    continue;
                }

                let arguments = signal.arguments();

                match (signal.name(), arguments) {
                    ("ItemEvent", [id, event]) => item_event(id.text(), event.text()),
                    ("ActivityEvent", [handle, event]) => {
                        activity_event(handle.number() as u32, event.text());
                    }
                    _ => {}
                }
            }
        });

        thread::spawn(|| {
            for signal in Bus::session().owner_changes(BUS_NAME) {
                let new_owner = signal
                    .arguments()
                    .get(2)
                    .map(|owner| owner.text().to_owned())
                    .unwrap_or_default();

                if new_owner.is_empty() {
                    run_in_order(forget_handles);
                } else {
                    run_in_order(register_again);
                }
            }
        });
    });
}

fn item_event(id: &str, json: &str) {
    let Ok(event) = ItemEvent::decode(json) else {
        return;
    };

    let handler = client().item_handlers.get(id).cloned();

    // run without the lock held, so a handler may update its item
    if let Some(handler) = handler {
        handler(event);
    }
}

fn activity_event(handle: u32, json: &str) {
    let Ok(event) = ActivityEvent::decode(json) else {
        return;
    };

    let handler = {
        let mut client = client();

        let Some((&local, activity)) = client
            .activities
            .iter()
            .find(|(_, activity)| activity.handle == Some(handle))
        else {
            return;
        };

        let handler = activity.on_event.clone();

        if event == ActivityEvent::Dismissed {
            client.activities.remove(&local);
        }

        handler
    };

    if let Some(handler) = handler {
        handler(event);
    }
}
