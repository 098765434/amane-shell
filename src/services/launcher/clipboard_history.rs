//! The launcher's clipboard history: what's been copied, newest first, for
//! its Clipboard mode to paste again. Off until it's turned on, and then
//! kept in `$XDG_STATE_HOME/amane-shell/clipboard/`, an index and a file a
//! copy.
//!
//! It watches the clipboard through wl-clipboard (`wl-paste --watch`),
//! which follows the compositor's selection without needing the keyboard,
//! and reads each new copy's best kind: a PNG or JPEG image, a list of
//! files, or text. A copy a password manager marks as one
//! (`x-kde-passwordManagerHint`), or made in an app it's told to leave
//! out, isn't kept. Pasting puts a copy back with `wl-copy` and, once the
//! window it's for has the keyboard again, types Ctrl+V into it (`wtype`;
//! Ctrl+Shift+V in a terminal).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use amane::serde::{Deserialize, Serialize};
use amane::{Service, Windows};
use amane_kit::settings::ClipboardHistory as Settings;

use crate::services::{run_now, save_state, state_file};

amane_kit::marker! {
    /// The history changed: what read the clips draws again.
    pub Clips
}

/// Where the history is kept, under the shell's state directory.
const DIR: &str = "clipboard";
const INDEX: &str = "clipboard/index.json";
/// The most a single copy, and all of them together, may be.
const ITEM_BYTES: usize = 4 << 20;
const TOTAL_BYTES: usize = 64 << 20;
/// How much of a text copy its row shows.
const PREVIEW_CHARS: usize = 300;
/// How long the window a paste is for takes to have the keyboard again,
/// once the launcher has gone.
const REFOCUS: Duration = Duration::from_millis(350);

/// What a copy is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
    Files,
}

/// One copy.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde")]
pub struct Clip {
    /// A hash of what was copied: a copy made again is the same one.
    pub id: u64,
    pub kind: Kind,
    /// The kind of data it was read as, and is put back as.
    pub mime: String,
    /// The start of a text, or the files' names.
    pub preview: String,
    pub bytes: usize,
    /// When it was copied, in seconds since the epoch.
    pub at: u64,
    /// The app it was copied in, as far as the focused window says.
    pub app: Option<String>,
}

impl Clip {
    /// The file what was copied is kept in.
    pub fn path(&self) -> PathBuf {
        state_file(DIR).join(format!("{:016x}.{}", self.id, extension(&self.mime)))
    }
}

fn extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "text/uri-list" => "uri",
        _ => "txt",
    }
}

struct History {
    clips: Vec<Clip>,
    // the watcher, while the history is on
    watch: Option<Child>,
}

static HISTORY: Mutex<Option<History>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut History) -> R) -> R {
    let mut history = HISTORY.lock().unwrap_or_else(|e| e.into_inner());
    f(history.get_or_insert_with(|| History {
        clips: load(),
        watch: None,
    }))
}

fn load() -> Vec<Clip> {
    std::fs::read_to_string(state_file(INDEX))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(clips: &[Clip]) {
    if let Ok(text) = serde_json::to_string(clips) {
        save_state(INDEX, &text);
    }
}

/// The copies kept, newest first.
pub fn clips() -> Vec<Clip> {
    Clips::watch();
    with(|history| history.clips.clone())
}

/// Starts or stops watching the clipboard as `settings` say; cheap to call
/// on every frame.
pub fn sync(settings: &Settings) {
    let on = settings.enabled;
    let start = with(|history| {
        // a watcher that died (wl-paste missing, the session going) is let go
        if let Some(child) = &mut history.watch
            && child.try_wait().ok().flatten().is_some()
        {
            history.watch = None;
        }
        match (on, history.watch.is_some()) {
            (true, false) => true,
            (false, true) => {
                if let Some(mut child) = history.watch.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                false
            }
            _ => false,
        }
    });
    if start {
        watch(settings.clone());
    }
}

/// Runs `wl-paste --watch`, which runs a command at every new copy; its
/// line out is the cue to read the copy.
fn watch(settings: Settings) {
    let child = Command::new("wl-paste")
        .args(["--watch", "sh", "-c", "cat > /dev/null; echo"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            eprintln!("amane-shell: clipboard history: wl-paste: {e}");
            return;
        }
    };
    let Some(out) = child.stdout.take() else {
        return;
    };
    with(|history| history.watch = Some(child));
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines() {
            if line.is_err() {
                break;
            }
            take(&settings);
        }
    });
}

/// Reads the copy just made, and keeps it unless it shouldn't be.
fn take(settings: &Settings) {
    let Some(types) = run_now("wl-paste", &[String::from("--list-types")]) else {
        return;
    };
    let types: Vec<String> = types.lines().map(String::from).collect();
    if is_secret(&types) {
        return;
    }
    let app = Windows::read()
        .focused()
        .and_then(|window| window.app_id().map(String::from));
    if app
        .as_ref()
        .is_some_and(|app| settings.exclude_apps.iter().any(|left_out| left_out == app))
    {
        return;
    }
    let Some((mime, kind)) = pick(&types, settings.images) else {
        return;
    };
    let Ok(out) = Command::new("wl-paste")
        .args(["--no-newline", "--type", &mime])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return;
    };
    let bytes = out.stdout;
    if bytes.is_empty() || bytes.len() > ITEM_BYTES {
        return;
    }
    let preview = match kind {
        Kind::Text => match std::str::from_utf8(&bytes) {
            Ok(text) if !text.trim().is_empty() => preview(text),
            _ => return,
        },
        Kind::Files => {
            let names = file_names(&String::from_utf8_lossy(&bytes));
            if names.is_empty() {
                return;
            }
            names.join(", ")
        }
        Kind::Image => String::from("Image"),
    };
    let clip = Clip {
        id: hash(&bytes),
        kind,
        mime,
        preview,
        bytes: bytes.len(),
        at: now(),
        app,
    };
    let path = clip.path();
    if !path.exists() {
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, &bytes));
        if let Err(e) = written {
            eprintln!("amane-shell: clipboard history: {}: {e}", path.display());
            return;
        }
    }
    let gone = with(|history| {
        add(&mut history.clips, clip);
        let gone = prune(&mut history.clips, settings.max_items, TOTAL_BYTES);
        save(&history.clips);
        gone
    });
    forget(&gone);
    Clips::changed();
}

/// Puts `clip` back on the clipboard; with `paste`, types it into the
/// window the launcher was over too, once that has the keyboard again.
pub fn put_back(clip: &Clip, paste: bool) {
    let clip = clip.clone();
    std::thread::spawn(move || {
        let Ok(file) = std::fs::File::open(clip.path()) else {
            return;
        };
        // wl-copy serves the copy from the background, and returns at once
        let copied = Command::new("wl-copy")
            .args(["--type", &clip.mime])
            .stdin(file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !paste || !copied.is_ok_and(|status| status.success()) {
            return;
        }
        std::thread::sleep(REFOCUS);
        let app = run_now(
            "niri",
            &[
                String::from("msg"),
                String::from("--json"),
                String::from("focused-window"),
            ],
        )
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .and_then(|window| window.get("app_id")?.as_str().map(String::from));
        let chord: Vec<String> = paste_chord(app.as_deref())
            .iter()
            .map(|arg| arg.to_string())
            .collect();
        run_now("wtype", &chord);
    });
}

/// Forgets the copy `id`.
pub fn delete(id: u64) {
    let gone = with(|history| {
        let gone: Vec<Clip> = history
            .clips
            .iter()
            .filter(|clip| clip.id == id)
            .cloned()
            .collect();
        history.clips.retain(|clip| clip.id != id);
        save(&history.clips);
        gone
    });
    forget(&gone);
    Clips::changed();
}

/// Forgets every copy.
pub fn clear() {
    let gone = with(|history| {
        let gone = std::mem::take(&mut history.clips);
        save(&history.clips);
        gone
    });
    forget(&gone);
    Clips::changed();
}

fn forget(clips: &[Clip]) {
    for clip in clips {
        let _ = std::fs::remove_file(clip.path());
    }
}

/// Whether the app copying marks it as a password.
pub fn is_secret(types: &[String]) -> bool {
    types.iter().any(|kind| kind == "x-kde-passwordManagerHint")
}

/// The best kind a copy offering `types` can be kept as: an image (if
/// `images` are kept), files, or text.
pub fn pick(types: &[String], images: bool) -> Option<(String, Kind)> {
    let has = |kind: &str| types.iter().any(|offered| offered == kind);
    if images {
        for kind in ["image/png", "image/jpeg"] {
            if has(kind) {
                return Some((kind.to_string(), Kind::Image));
            }
        }
    }
    if has("text/uri-list") {
        return Some((String::from("text/uri-list"), Kind::Files));
    }
    [
        "text/plain;charset=utf-8",
        "UTF8_STRING",
        "text/plain",
        "TEXT",
        "STRING",
    ]
    .into_iter()
    .find(|kind| has(kind))
    .map(|kind| (kind.to_string(), Kind::Text))
}

/// `clip` at the front, the same copy made before taken out from further
/// down.
pub fn add(clips: &mut Vec<Clip>, clip: Clip) {
    clips.retain(|old| old.id != clip.id);
    clips.insert(0, clip);
}

/// Drops the oldest copies past `items`, or past `bytes` in all; gives the
/// ones dropped, to delete their files.
pub fn prune(clips: &mut Vec<Clip>, items: usize, bytes: usize) -> Vec<Clip> {
    let mut total = 0;
    let keep = clips
        .iter()
        .take(items)
        .take_while(|clip| {
            total += clip.bytes;
            total <= bytes
        })
        .count();
    clips.split_off(keep)
}

/// The keys that paste into an app with `app_id`: Ctrl+Shift+V in a
/// terminal, Ctrl+V anywhere else, as wtype's arguments.
pub fn paste_chord(app_id: Option<&str>) -> &'static [&'static str] {
    const TERMINALS: [&str; 12] = [
        "foot",
        "footclient",
        "kitty",
        "alacritty",
        "org.wezfurlong.wezterm",
        "com.mitchellh.ghostty",
        "ghostty",
        "xterm",
        "org.kde.konsole",
        "org.gnome.console",
        "org.gnome.terminal",
        "org.gnome.ptyxis",
    ];
    let terminal = app_id.is_some_and(|app| {
        let app = app.to_lowercase();
        TERMINALS.iter().any(|terminal| *terminal == app)
    });
    if terminal {
        &[
            "-M", "ctrl", "-M", "shift", "v", "-m", "shift", "-m", "ctrl",
        ]
    } else {
        &["-M", "ctrl", "v", "-m", "ctrl"]
    }
}

/// The start of `text`, its lines run together.
fn preview(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(PREVIEW_CHARS).collect()
}

/// The names of the files a `text/uri-list` lists.
fn file_names(uris: &str) -> Vec<String> {
    uris.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|uri| {
            let path = uri.strip_prefix("file://").unwrap_or(uri);
            path.rsplit('/').next().unwrap_or(path).replace("%20", " ")
        })
        .collect()
}

fn hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types(list: &[&str]) -> Vec<String> {
        list.iter().map(|kind| kind.to_string()).collect()
    }

    fn clip(id: u64, bytes: usize) -> Clip {
        Clip {
            id,
            kind: Kind::Text,
            mime: String::from("text/plain"),
            preview: String::new(),
            bytes,
            at: 0,
            app: None,
        }
    }

    #[test]
    fn a_password_is_left_alone() {
        assert!(is_secret(&types(&[
            "text/plain",
            "x-kde-passwordManagerHint"
        ])));
        assert!(!is_secret(&types(&["text/plain"])));
    }

    #[test]
    fn an_image_beats_files_beats_text() {
        let offered = types(&["text/plain", "text/uri-list", "image/png"]);
        assert_eq!(
            pick(&offered, true),
            Some((String::from("image/png"), Kind::Image))
        );
        assert_eq!(
            pick(&offered, false),
            Some((String::from("text/uri-list"), Kind::Files))
        );
        assert_eq!(
            pick(&types(&["text/plain", "UTF8_STRING"]), true),
            Some((String::from("UTF8_STRING"), Kind::Text))
        );
        assert_eq!(pick(&types(&["image/webp"]), true), None);
    }

    #[test]
    fn a_copy_made_again_moves_to_the_front() {
        let mut clips = vec![clip(1, 1), clip(2, 1), clip(3, 1)];
        add(&mut clips, clip(3, 1));
        let ids: Vec<u64> = clips.iter().map(|clip| clip.id).collect();
        assert_eq!(ids, vec![3, 1, 2]);
    }

    #[test]
    fn the_oldest_go_past_the_limits() {
        let mut clips = vec![clip(1, 10), clip(2, 10), clip(3, 10)];
        let gone = prune(&mut clips, 2, 100);
        assert_eq!(gone.iter().map(|clip| clip.id).collect::<Vec<_>>(), vec![3]);
        let mut clips = vec![clip(1, 10), clip(2, 10), clip(3, 10)];
        let gone = prune(&mut clips, 10, 25);
        assert_eq!(clips.len(), 2);
        assert_eq!(gone.len(), 1);
    }

    #[test]
    fn a_terminal_pastes_with_shift() {
        assert_eq!(paste_chord(Some("foot")).len(), 9);
        assert_eq!(paste_chord(Some("Alacritty")).len(), 9);
        assert_eq!(
            paste_chord(Some("firefox")),
            &["-M", "ctrl", "v", "-m", "ctrl"]
        );
        assert_eq!(paste_chord(None).len(), 5);
    }

    #[test]
    fn files_are_named_by_their_last_part() {
        assert_eq!(
            file_names("# comment\nfile:///home/a/My%20Doc.pdf\nfile:///tmp/x.png\n"),
            vec![String::from("My Doc.pdf"), String::from("x.png")]
        );
    }

    #[test]
    fn text_runs_together_in_its_preview() {
        assert_eq!(preview("  one\n\ntwo   three "), "one two three");
    }
}
