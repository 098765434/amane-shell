//! Recently Shared, as Dropzone's menu keeps it: the last links uploads
//! gave, in `$XDG_STATE_HOME/amane-drop/shared.json`, the newest first. A
//! link goes on the clipboard and a notification says so, as Settings has
//! it.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, PoisonError};

use amane::serde::{Deserialize, Serialize};
use amane::{Clipboard, Notify};

/// How many links are kept.
pub const KEPT: usize = 10;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde")]
pub struct Shared {
    /// What was shared: "photo.jpg", "3 items".
    pub title: String,
    pub link: String,
    /// Seconds since 1970.
    pub at: u64,
}

static LIST: Mutex<Option<Vec<Shared>>> = Mutex::new(None);

fn file() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
    Some(state.join("amane-drop").join("shared.json"))
}

fn with_list<R>(f: impl FnOnce(&mut Vec<Shared>) -> R) -> R {
    let mut list = LIST.lock().unwrap_or_else(PoisonError::into_inner);
    let list = list.get_or_insert_with(|| {
        file()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    });
    f(list)
}

fn save(list: &[Shared]) {
    let (Some(file), Ok(text)) = (file(), serde_json::to_string_pretty(list)) else {
        return;
    };
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let part = file.with_extension("part");
    if std::fs::write(&part, text).is_ok() {
        let _ = std::fs::rename(part, file);
    }
}

/// The links kept, the newest first.
pub fn list() -> Vec<Shared> {
    with_list(|list| list.clone())
}

/// Keeps `link` first, the oldest going past [`KEPT`].
pub fn remember(title: &str, link: &str) {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    with_list(|list| {
        list.retain(|shared| shared.link != link);
        list.insert(
            0,
            Shared {
                title: title.to_string(),
                link: link.to_string(),
                at,
            },
        );
        list.truncate(KEPT);
        save(list);
    });
    crate::refresh_item();
}

pub fn clear() {
    with_list(|list| {
        list.clear();
        save(list);
    });
    crate::refresh_item();
}

/// Puts `text` on the clipboard. An upload ends long after any input, so
/// `wl-copy` (the data-control protocol, which needs none) goes first, and
/// amane's own clipboard when it isn't there.
pub fn copy(text: &str) {
    let copied = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take()?.write_all(text.as_bytes()).ok()?;
            // wl-copy waits in the background to serve the paste
            child.wait().ok().filter(|status| status.success())
        })
        .is_some();
    if !copied {
        Clipboard::copy(text);
    }
}

/// What a task that gave `links` does with them: kept, copied and told,
/// as Settings has it. Whether they were copied.
pub fn delivered(title: &str, links: &[String]) -> bool {
    if links.is_empty() {
        return false;
    }
    for link in links.iter().rev() {
        remember(title, link);
    }
    let (copy_link, notify) = {
        let sharing = amane_kit::settings::drop_sharing();
        (sharing.copy_link, sharing.notify)
    };
    let all = links.join("\n");
    if copy_link {
        copy(&all);
    }
    if notify {
        let summary = if copy_link {
            format!("Link to {title} copied")
        } else {
            format!("Shared {title}")
        };
        Notify::new(&summary)
            .body(&all)
            .icon("emblem-shared")
            .send();
    }
    copy_link
}

/// Opens `link` in the browser.
pub fn open(link: &str) {
    let _ = Command::new("xdg-open")
        .arg(link)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
