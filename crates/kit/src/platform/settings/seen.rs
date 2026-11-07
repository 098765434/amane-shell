//! What the shell has seen, kept in its cache folder so Shell Settings can list it when it
//! isn't there: tray apps, programs with their own item, and apps that showed an activity.
//! Each file is read again only when it changed, and written only when something is new.

use super::*;
use crate::sync::Locked as _;

/// A file in the shell's cache folder.
fn cache_file(name: &str) -> PathBuf {
    crate::paths::cache().join(crate::app_ids::SHELL).join(name)
}

/// Writes what's been seen to its file whole. A failure is reported, since
/// what it carried is lost.
fn keep(name: &str, seen: &impl amane::serde::Serialize) {
    let file = cache_file(name);
    if let Err(e) = crate::fs::save_json(&file, seen) {
        log::warn!("couldn't save {}: {e}", file.display());
    }
}

/// Remembers `id` with what it calls itself, once; a name it had before is
/// kept when it now has none.
fn remember_named(name: &'static str, id: &str, called: &str) {
    let mut seen: BTreeMap<String, String> = seen_cached(name);
    if seen
        .get(id)
        .is_some_and(|known| called.is_empty() || known == called)
    {
        return;
    }
    seen.insert(String::from(id), String::from(called));
    keep(name, &seen);
}

/// A cache file's contents, read again only when the file changed: the shell remembers into
/// these on every status-item update, and Shell Settings lists them on every rebuild.
fn seen_cached<T>(name: &'static str) -> T
where
    T: amane::serde::de::DeserializeOwned + Clone + Default + Send + 'static,
{
    type Cached = (Option<std::time::SystemTime>, Box<dyn std::any::Any + Send>);
    static CACHE: std::sync::Mutex<Option<std::collections::HashMap<&'static str, Cached>>> =
        std::sync::Mutex::new(None);

    let file = cache_file(name);
    let modified = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
    let mut cache = CACHE.locked();
    let cache = cache.get_or_insert_with(std::collections::HashMap::new);
    if let Some((when, value)) = cache.get(name)
        && *when == modified
        && let Some(value) = value.downcast_ref::<T>()
    {
        return value.clone();
    }
    let value: T = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    cache.insert(name, (modified, Box::new(value.clone())));
    value
}

const STATUS_APPS: &str = "status-apps-seen.json";
const ACTIVITIES: &str = "activities-seen.json";
const TRAY: &str = "tray-seen.json";

/// Every program seen with its own item in the bar (`ItemSpec`), as (id,
/// what it calls itself), by id.
pub fn status_apps_seen() -> BTreeMap<String, String> {
    seen_cached(STATUS_APPS)
}

/// Remembers a program's item, once; a name it had before is kept when it
/// now has none.
pub fn remember_status_app(id: &str, name: &str) {
    remember_named(STATUS_APPS, id, name);
}

/// Every app seen showing an activity, by name, so Shell Settings can
/// list them when none is going on.
pub fn activities_seen() -> std::collections::BTreeSet<String> {
    seen_cached(ACTIVITIES)
}

/// Remembers an app that showed an activity, once.
pub fn remember_activity(app: &str) {
    let mut seen = activities_seen();
    if !app.is_empty() && seen.insert(String::from(app)) {
        keep(ACTIVITIES, &seen);
    }
}

/// Every tray app seen, as (id, title), by id.
pub fn tray_seen() -> BTreeMap<String, String> {
    seen_cached(TRAY)
}

/// Remembers a tray app, once; a title it had before is kept when it
/// now has none.
pub fn remember_tray(id: &str, title: &str) {
    remember_named(TRAY, id, title);
}
