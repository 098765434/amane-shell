//! `org.amane.StatusBar1`: where other programs put their own items in the
//! bar (status items) and their activities. Each payload is
//! one JSON string of amane's `status_item` types, so a script can call it
//! with `busctl` too. Items are kept here; activities go to the bar's
//! [`activities`](super::desktop::activities). What happens to an item
//! (a click, a menu entry, files let go on it) goes back to the program that
//! registered it, as a signal to it alone.
//!
//! Everything a program put up goes when it leaves the bus.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use amane::{Argument, Bus, Method};
use amane_kit::settings::remember_status_app;
use amane_status::{
    ActivityContent, ActivityEvent, BUS_NAME, Dismiss, INTERFACE, ItemEvent, ItemSpec, PATH, encode,
};

use super::desktop::activities;

amane_kit::marker! {
    /// A program's item came, changed or went: the bar draws again.
    pub StatusApps
}

const INTROSPECTABLE: &str = "org.freedesktop.DBus.Introspectable";

const INTROSPECTION: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.amane.StatusBar1">
    <method name="RegisterItem"><arg name="spec" type="s" direction="in"/></method>
    <method name="UpdateItem"><arg name="spec" type="s" direction="in"/></method>
    <method name="RemoveItem"><arg name="id" type="s" direction="in"/></method>
    <method name="StartActivity">
      <arg name="content" type="s" direction="in"/>
      <arg name="handle" type="u" direction="out"/>
    </method>
    <method name="UpdateActivity">
      <arg name="handle" type="u" direction="in"/>
      <arg name="content" type="s" direction="in"/>
      <arg name="alert" type="b" direction="in"/>
    </method>
    <method name="EndActivity">
      <arg name="handle" type="u" direction="in"/>
      <arg name="dismiss" type="s" direction="in"/>
    </method>
    <signal name="ItemEvent"><arg name="id" type="s"/><arg name="event" type="s"/></signal>
    <signal name="ActivityEvent"><arg name="handle" type="u"/><arg name="event" type="s"/></signal>
  </interface>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect"><arg name="data" type="s" direction="out"/></method>
  </interface>
</node>
"#;

/// Errors a caller gets back.
const INVALID: &str = "org.amane.StatusBar1.Error.Invalid";
const TAKEN: &str = "org.amane.StatusBar1.Error.IdTaken";
const UNKNOWN: &str = "org.amane.StatusBar1.Error.Unknown";

/// A program's item in the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct AppItem {
    /// Its program's bus name, like ":1.42".
    pub owner: String,
    pub spec: ItemSpec,
}

impl AppItem {
    /// What the bar keeps its place by: `app:<id>`.
    pub fn name(&self) -> String {
        format!("app:{}", self.spec.id)
    }
}

static ITEMS: Mutex<Vec<AppItem>> = Mutex::new(Vec::new());

fn items_now() -> MutexGuard<'static, Vec<AppItem>> {
    ITEMS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Every program's item, in the order they came; the bar redraws when they
/// change.
pub fn items() -> Vec<AppItem> {
    StatusApps::watch();
    items_now().clone()
}

/// The item with this id, if a program has one.
pub fn item(id: &str) -> Option<AppItem> {
    items_now().iter().find(|item| item.spec.id == id).cloned()
}

/// Tells the program whose item `id` is what happened to it, off the
/// event loop.
pub fn send(id: &str, event: ItemEvent) {
    let Some(owner) = item(id).map(|item| item.owner) else {
        return;
    };
    let id = id.to_string();
    thread::spawn(move || {
        Bus::session().emit_to(
            &owner,
            PATH,
            INTERFACE,
            "ItemEvent",
            &[Argument::Text(id), Argument::Text(encode(&event))],
        );
    });
}

/// Serves `org.amane.StatusBar1` on threads of its own, unless another
/// program already does. Called once at start.
pub fn serve() {
    thread::spawn(|| {
        let bus = Bus::session();
        // watched before the name is owned, so no early call is missed
        let methods = bus.methods(PATH, INTERFACE);
        let introspect = bus.methods(PATH, INTROSPECTABLE);
        let leaving = bus.signals("org.freedesktop.DBus", "NameOwnerChanged");
        if !bus.own(BUS_NAME) {
            eprintln!("amane-shell: another program serves {BUS_NAME}; status apps go there");
            return;
        }
        thread::spawn(move || {
            for method in introspect {
                if method.name() == "Introspect" {
                    method.reply(&[Argument::Text(String::from(INTROSPECTION))]);
                }
            }
        });
        thread::spawn(move || {
            for signal in leaving {
                // a program's own name, whose owner is now nobody
                if let [name, _, new] = signal.arguments()
                    && name.text().starts_with(':')
                    && new.text().is_empty()
                {
                    forget(name.text());
                }
            }
        });
        for method in methods {
            answer(&method);
        }
    });
}

/// Everything the program `owner` put up goes, as it left the bus.
fn forget(owner: &str) {
    let gone = {
        let mut items = items_now();
        let before = items.len();
        items.retain(|item| item.owner != owner);
        items.len() != before
    };
    if gone {
        StatusApps::changed();
    }
    activities::end_all_of(owner);
}

fn answer(method: &Method) {
    let Some(sender) = method.sender() else {
        return;
    };
    let arguments = method.arguments();
    let text = |index: usize| arguments.get(index).map(|value| value.text().to_owned());
    let number = |index: usize| arguments.get(index).map(|value| value.number() as u32);

    let answered = match method.name() {
        "RegisterItem" => text(0)
            .ok_or_else(|| missing("a spec"))
            .and_then(|json| register(&sender, &json, false)),
        "UpdateItem" => text(0)
            .ok_or_else(|| missing("a spec"))
            .and_then(|json| register(&sender, &json, true)),
        "RemoveItem" => text(0)
            .ok_or_else(|| missing("an id"))
            .and_then(|id| remove(&sender, &id)),
        "StartActivity" => {
            let handle = text(0)
                .ok_or_else(|| missing("its content"))
                .and_then(|json| start(&sender, &json));
            match handle {
                Ok(handle) => {
                    method.reply(&[Argument::Unsigned(handle)]);
                    return;
                }
                Err(error) => Err(error),
            }
        }
        "UpdateActivity" => match (number(0), text(1)) {
            (Some(handle), Some(json)) => {
                let alert = arguments.get(2).is_some_and(|value| value.bool());
                update(&sender, handle, &json, alert)
            }
            _ => Err(missing("a handle and its content")),
        },
        "EndActivity" => match number(0) {
            Some(handle) => {
                let dismiss = text(1)
                    .filter(|json| !json.trim().is_empty())
                    .map_or(Ok(Dismiss::Now), |json| {
                        Dismiss::decode(&json).map_err(|error| (INVALID, error.to_string()))
                    });
                dismiss.and_then(|dismiss| {
                    activities::end(&sender, handle, dismiss)
                        .then_some(())
                        .ok_or_else(|| (UNKNOWN, format!("no activity {handle} of yours")))
                })
            }
            None => Err(missing("a handle")),
        },
        other => Err((UNKNOWN, format!("no method {other}"))),
    };
    match answered {
        Ok(()) => method.reply(&[]),
        Err((name, message)) => method.reply_error(name, &message),
    }
}

type Answer<T = ()> = Result<T, (&'static str, String)>;

fn missing(what: &str) -> (&'static str, String) {
    (INVALID, format!("the call needs {what}"))
}

/// Puts up (or, `update`, changes) the item a spec describes.
fn register(sender: &str, json: &str, update: bool) -> Answer {
    let spec = ItemSpec::decode(json).map_err(|error| (INVALID, error.to_string()))?;
    let name = spec.title.clone().unwrap_or_else(|| spec.tooltip.clone());
    let id = spec.id.clone();
    {
        let mut items = items_now();
        match items.iter_mut().find(|item| item.spec.id == spec.id) {
            Some(item) if item.owner != sender => {
                return Err((TAKEN, format!("another program has the item {}", spec.id)));
            }
            Some(item) => item.spec = spec,
            None if update => {
                return Err((UNKNOWN, format!("no item {} of yours", spec.id)));
            }
            None => items.push(AppItem {
                owner: sender.to_string(),
                spec,
            }),
        }
    }
    // Shell Settings lists it once seen; tests leave the real list alone
    if !cfg!(test) {
        remember_status_app(&id, &name);
    }
    StatusApps::changed();
    Ok(())
}

fn remove(sender: &str, id: &str) -> Answer {
    {
        let mut items = items_now();
        let Some(index) = items
            .iter()
            .position(|item| item.spec.id == id && item.owner == sender)
        else {
            return Err((UNKNOWN, format!("no item {id} of yours")));
        };
        items.remove(index);
    }
    StatusApps::changed();
    Ok(())
}

/// Starts an activity whose buttons and leaving are told to `sender`.
fn start(sender: &str, json: &str) -> Answer<u32> {
    let content = ActivityContent::decode(json).map_err(|error| (INVALID, error.to_string()))?;
    // the handle is only known once it has started
    let numbered = Arc::new(AtomicU32::new(0));
    let owner = sender.to_string();
    let told = numbered.clone();
    let sink: activities::Sink = Arc::new(move |event: ActivityEvent| {
        Bus::session().emit_to(
            &owner,
            PATH,
            INTERFACE,
            "ActivityEvent",
            &[
                Argument::Unsigned(told.load(Ordering::SeqCst)),
                Argument::Text(encode(&event)),
            ],
        );
    });
    let handle = activities::start(sender, content, Some(sink));
    numbered.store(handle, Ordering::SeqCst);
    Ok(handle)
}

fn update(sender: &str, handle: u32, json: &str, alert: bool) -> Answer {
    let content = ActivityContent::decode(json).map_err(|error| (INVALID, error.to_string()))?;
    activities::update(sender, handle, content, alert)
        .then_some(())
        .ok_or_else(|| (UNKNOWN, format!("no activity {handle} of yours")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_belongs_to_the_program_that_registered_it() {
        let spec = r#"{"id":"test.taken","title":"Mine"}"#;
        assert_eq!(register(":9.1", spec, false), Ok(()));
        assert_eq!(register(":9.1", spec, false), Ok(()));
        assert_eq!(register(":9.2", spec, false).map_err(|e| e.0), Err(TAKEN));
        assert_eq!(remove(":9.2", "test.taken").map_err(|e| e.0), Err(UNKNOWN));
        assert_eq!(
            register(":9.2", r#"{"id":"test.never"}"#, true).map_err(|e| e.0),
            Err(UNKNOWN)
        );
        forget(":9.1");
        assert!(item("test.taken").is_none());
    }

    #[test]
    fn a_bad_payload_is_refused() {
        assert_eq!(register(":9.3", "{", false).map_err(|e| e.0), Err(INVALID));
        assert_eq!(start(":9.3", r#"{"v":99}"#).map_err(|e| e.0), Err(INVALID));
    }
}
