//! Running programs off the UI thread, and the newest of a run of values
//! applied on a worker. The runner itself is [`amane_process`], whose
//! [`Cmd`] is here too.

pub use amane_process::{Cmd, Failure, Output, Running, Stopper};

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Runs `program` with `args` to the end, off the UI thread, and says so if
/// it couldn't start or failed.
pub fn run(program: &'static str, args: Vec<String>) {
    std::thread::spawn(move || {
        // run_now has logged a failure; nobody waits for this one
        let _ = run_now(program, &args);
    });
}

/// Runs `program` with `args` to the end and gives its stdout, or `None`
/// when it couldn't start or failed. Blocks, so never call it from a view.
pub fn run_now(program: &str, args: &[String]) -> Option<String> {
    match Cmd::new(program).args(args).stdout() {
        Ok(out) => Some(out),
        Err(message) => {
            log::warn!("{program}: {message}");
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

impl<T: Send + 'static> Default for Latest<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + 'static> Latest<T> {
    /// Nothing waiting yet; a `static` holds one.
    pub const fn new() -> Self {
        Self {
            next: Mutex::new(None),
            busy: AtomicBool::new(false),
        }
    }

    /// Applies `value` with `apply` on a worker thread, unless a newer value comes before the
    /// worker gets to it.
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
