//! One thing at a time: an uninstall, a service started or stopped. What's
//! running shows on its row, and the last failure under the list.

use std::marker::PhantomData;
use std::thread;

use amane::Service;

/// What's running of one kind of job (`K` only names the kind), and how
/// the last one failed.
pub struct Busy<K: 'static> {
    /// What the job running is for, while one runs.
    pub busy: Option<String>,
    /// What the last failed job was for, and why.
    pub error: Option<(String, String)>,
    kind: PhantomData<fn() -> K>,
}

impl<K: 'static> Service for Busy<K> {
    fn new() -> Self {
        Self {
            busy: None,
            error: None,
            kind: PhantomData,
        }
    }

    fn listen() {}
}

impl<K: 'static> Busy<K> {
    /// Runs `job` for `key` on a thread, unless one of its kind is running;
    /// then keeps how it failed, if it did, and calls `then`.
    pub fn run(key: String, job: impl FnOnce() -> Result<(), String> + Send + 'static, then: fn()) {
        {
            let mut now = Self::write();
            if now.busy.is_some() {
                now.quiet();
                return;
            }
            now.busy = Some(key.clone());
            now.error = None;
        }
        thread::spawn(move || {
            let result = job();
            {
                let mut now = Self::write();
                now.busy = None;
                if let Err(why) = result {
                    now.error = Some((key, why));
                }
            }
            then();
        });
    }

    /// Whether the job running is for `key`.
    pub fn is_busy(&self, key: &str) -> bool {
        self.busy.as_deref() == Some(key)
    }
}
