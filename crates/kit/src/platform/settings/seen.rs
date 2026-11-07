//! What the shell has seen, kept in its cache folder so Shell Settings can list it when it
//! isn't there: tray apps, programs with their own item, and apps that showed an activity.
//! Each file is read again only when it changed, and written only when something is new.

use super::*;

/// A file in the shell's cache folder.
pub(super) fn cache_file(name: &str) -> Option<PathBuf> {
    let cache = crate::paths::cache();
    Some(cache.join("amane-shell").join(name))
}

/// Writes `text` to `file` whole, through a part file. A failure is
/// reported, since the change it carried is lost.
pub(super) fn write_whole(file: &std::path::Path, text: &str) {
    let write = || {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let partial = file.with_extension("part");
        std::fs::write(&partial, text)?;
        std::fs::rename(partial, file)
    };
    if let Err(e) = write() {
        eprintln!("amane-kit: couldn't save {}: {e}", file.display());
    }
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

    let Some(file) = cache_file(name) else {
        return T::default();
    };
    let modified = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

/// Where the shell keeps the tray apps it has seen, so Shell Settings can
/// list them when they aren't running.
fn tray_seen_file() -> Option<PathBuf> {
    cache_file("tray-seen.json")
}

/// Every program seen with its own item in the bar (`ItemSpec`), as (id,
/// what it calls itself), by id.
pub fn status_apps_seen() -> BTreeMap<String, String> {
    seen_cached("status-apps-seen.json")
}

/// Remembers a program's item, once; a name it had before is kept when it
/// now has none.
pub fn remember_status_app(id: &str, name: &str) {
    let mut seen = status_apps_seen();
    if seen
        .get(id)
        .is_some_and(|known| name.is_empty() || known == name)
    {
        return;
    }
    seen.insert(String::from(id), String::from(name));
    let (Some(file), Ok(text)) = (
        cache_file("status-apps-seen.json"),
        serde_json::to_string_pretty(&seen),
    ) else {
        return;
    };
    write_whole(&file, &text);
}

/// Every app seen showing an activity, by name, so Shell Settings can
/// list them when none is going on.
pub fn activities_seen() -> std::collections::BTreeSet<String> {
    seen_cached("activities-seen.json")
}

/// Remembers an app that showed an activity, once.
pub fn remember_activity(app: &str) {
    let mut seen = activities_seen();
    if app.is_empty() || !seen.insert(String::from(app)) {
        return;
    }
    let (Some(file), Ok(text)) = (
        cache_file("activities-seen.json"),
        serde_json::to_string_pretty(&seen),
    ) else {
        return;
    };
    write_whole(&file, &text);
}

/// Every tray app seen, as (id, title), by id.
pub fn tray_seen() -> BTreeMap<String, String> {
    seen_cached("tray-seen.json")
}

/// Remembers a tray app, once; a title it had before is kept when it
/// now has none.
pub fn remember_tray(id: &str, title: &str) {
    let mut seen = tray_seen();
    let known = seen.get(id);
    if known.is_some_and(|known| title.is_empty() || known == title) {
        return;
    }
    seen.insert(String::from(id), String::from(title));
    let Some(file) = tray_seen_file() else {
        return;
    };
    let Ok(text) = serde_json::to_string_pretty(&seen) else {
        return;
    };
    write_whole(&file, &text);
}
