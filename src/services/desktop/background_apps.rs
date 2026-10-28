//! Background apps — apps running without a window, as the desktop portal
//! sees them (`org.freedesktop.background.Monitor`), which is what GNOME's
//! Background Apps lists. Mostly sandboxed (Flatpak) apps that asked to run
//! in the background.
//!
//! The list is read once, then again on each `PropertiesChanged` the
//! monitor sends. Closing one asks the app to quit
//! (`org.freedesktop.Application.ActivateAction("quit")`), as GNOME does,
//! and kills it with `flatpak kill` when it doesn't answer that.

use amane::{Bus, Service, Value};

use crate::services::run_now;

const MONITOR: &str = "org.freedesktop.background.Monitor";
const PATH: &str = "/org/freedesktop/background/monitor";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundApp {
    pub app_id: String,
    /// What the app says it's doing, if anything.
    pub message: String,
}

#[derive(Default, PartialEq)]
pub struct BackgroundApps {
    apps: Vec<BackgroundApp>,
}

impl Service for BackgroundApps {
    fn new() -> Self {
        Self { apps: fetch() }
    }

    fn listen() {
        let changes = Bus::session().signals(PROPERTIES, "PropertiesChanged");

        // a change between new() and watching would otherwise wait for the next one
        refresh();

        for signal in changes {
            if signal.path() == PATH {
                refresh();
            }
        }
    }
}

impl BackgroundApps {
    pub fn apps(&self) -> &[BackgroundApp] {
        &self.apps
    }

    pub fn quit(app_id: &str) {
        let app_id = app_id.to_string();
        std::thread::spawn(move || {
            let path = format!("/{}", app_id.replace('.', "/").replace('-', "_"));
            let asked = run_now(
                "gdbus",
                &[
                    "call".into(),
                    "--session".into(),
                    "--dest".into(),
                    app_id.clone(),
                    "--object-path".into(),
                    path,
                    "--method".into(),
                    "org.freedesktop.Application.ActivateAction".into(),
                    "quit".into(),
                    "[]".into(),
                    "{}".into(),
                ],
            );
            if asked.is_none() {
                let _ = run_now("flatpak", &["kill".into(), app_id]);
            }
        });
    }
}

fn refresh() {
    let apps = fetch();
    if BackgroundApps::read().apps != apps {
        BackgroundApps::write().apps = apps;
    }
}

fn fetch() -> Vec<BackgroundApp> {
    parse(&Bus::session().property(MONITOR, PATH, MONITOR, "BackgroundApps"))
}

/// The monitor's `aa{sv}`: one map per app, with `app_id` and `message`.
fn parse(value: &Value) -> Vec<BackgroundApp> {
    let mut apps: Vec<BackgroundApp> = value
        .list()
        .iter()
        .filter(|app| !app.get("app_id").text().is_empty())
        .map(|app| BackgroundApp {
            app_id: app.get("app_id").text().to_string(),
            message: app.get("message").text().to_string(),
        })
        .collect();
    apps.sort_by(|a, b| a.app_id.cmp(&b.app_id));
    apps.dedup_by(|a, b| a.app_id == b.app_id);
    apps
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn app(id: &str, message: &str) -> Value {
        let mut map = BTreeMap::new();
        map.insert("app_id".to_string(), Value::Text(id.into()));
        map.insert("message".to_string(), Value::Text(message.into()));
        map.insert("instance".to_string(), Value::Text("1".into()));
        Value::Map(map)
    }

    #[test]
    fn reads_each_app_once_in_order() {
        let value = Value::List(vec![
            app("org.signal.Signal", ""),
            app("com.valvesoftware.Steam", "Updating"),
            app("org.signal.Signal", ""),
        ]);
        assert_eq!(
            parse(&value),
            [
                BackgroundApp {
                    app_id: "com.valvesoftware.Steam".into(),
                    message: "Updating".into()
                },
                BackgroundApp {
                    app_id: "org.signal.Signal".into(),
                    message: String::new()
                },
            ]
        );
    }

    #[test]
    fn no_portal_is_no_apps() {
        assert!(parse(&Value::Nothing).is_empty());
        assert!(parse(&Value::List(vec![Value::Nothing])).is_empty());
    }
}
