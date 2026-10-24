//! The shell's own services: system state Quick Settings shows that the
//! amane library has no service for. Each is an amane [`Service`] like the
//! library's — live state a view reads, redrawing only when it changes, and
//! a sane "not present" when its daemon or device is missing — so a tile
//! reads it the same way it reads `Network` or `Bluetooth`.
//!
//! - [`session`] — lock, suspend, restart, power off and log out (logind).
//! - [`night_light`] — a warm screen, by running `wlsunset`.
//! - [`dark_style`] — the system's light/dark preference (dconf).
//! - [`rfkill`] — airplane mode: every radio blocked.
//! - [`keyboard_backlight`] — the keyboard's light (sysfs and logind).
//! - [`connections`] — wired, VPN and hotspot (NetworkManager's `nmcli`).
//! - [`privacy`] — what's using the microphone, the camera or the screen.
//! - [`background_apps`] — apps running without a window (the portal).
//! - [`tailscale`] — the tailnet, its exit nodes and settings (the CLI).
//! - [`keep_awake`] — the screen held on and the lid ignored (inhibitors).
//!
//! See `docs/quick-settings.md`.
//!
//! [`Service`]: amane::Service

pub mod background_apps;
pub mod connections;
pub mod dark_style;
pub mod keep_awake;
pub mod keyboard_backlight;
pub mod night_light;
pub mod privacy;
pub mod rfkill;
pub mod session;
pub mod tailscale;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The file `name` under the shell's state directory
/// (`$XDG_STATE_HOME/amane-shell`), where the few settings it keeps live.
pub fn state_file(name: &str) -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    state.join("amane-shell").join(name)
}

/// Writes `text` to the state file `name`, saying so on failure.
pub fn save_state(name: &str, text: &str) {
    let path = state_file(name);
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text));
    if let Err(e) = result {
        eprintln!("amane-shell: {}: {e}", path.display());
    }
}

/// Runs `program` with `args` to the end, off the UI thread, and says so if
/// it couldn't start or failed.
pub fn run(program: &'static str, args: Vec<String>) {
    std::thread::spawn(move || {
        let _ = run_now(program, &args);
    });
}

/// Runs `program` with `args` to the end and gives its stdout, or `None`
/// when it couldn't start or failed. Blocks, so never call it from a view.
pub fn run_now(program: &str, args: &[String]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(out) if out.status.success() => Some(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            eprintln!("amane-shell: {program}: {}", err.trim());
            None
        }
        Err(e) => {
            eprintln!("amane-shell: {program}: {e}");
            None
        }
    }
}

/// Whether `program` is on `PATH`, for a link to an app that may not be
/// installed.
pub fn installed(program: &str) -> bool {
    thread_local! {
        // asked every frame a page shows its settings link; PATH doesn't
        // change under a running shell
        static KNOWN: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    }
    KNOWN.with_borrow_mut(|known| {
        *known.entry(program.to_string()).or_insert_with(|| {
            std::env::var_os("PATH").is_some_and(|path| {
                std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
            })
        })
    })
}

/// Applies only the newest of a run of values on a worker thread, so a
/// slider dragged faster than its daemon answers sends what it was let go
/// at, not every step on the way.
pub struct Latest<T> {
    next: Mutex<Option<T>>,
    busy: AtomicBool,
}

impl<T: Send + 'static> Latest<T> {
    pub const fn new() -> Self {
        Self {
            next: Mutex::new(None),
            busy: AtomicBool::new(false),
        }
    }

    pub fn send(&'static self, value: T, apply: fn(T)) {
        *self.next.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
        if self.busy.swap(true, Ordering::AcqRel) {
            // the running worker picks it up
            return;
        }
        std::thread::spawn(move || {
            loop {
                let next = self.next.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(value) = next {
                    apply(value);
                    continue;
                }
                self.busy.store(false, Ordering::Release);
                // a value sent between the take and the store would be left behind
                let more = self
                    .next
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_some();
                if !more || self.busy.swap(true, Ordering::AcqRel) {
                    return;
                }
            }
        });
    }
}
