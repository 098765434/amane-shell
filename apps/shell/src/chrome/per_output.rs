//! State a surface keeps for each monitor, by its connector name, as the
//! dock, the widgets and the bar do. A monitor's state goes once it's
//! unplugged: [`follow_outputs`] (once, at startup) follows amane's
//! `Outputs`, and each map lets go of what's gone the next time it's used.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Once, PoisonError};

use amane::{Outputs, Service};

// the connected outputs' names, and a count that moves on when they change
static NAMES: Mutex<Option<Vec<String>>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Follows the connected outputs, so the maps forget unplugged ones.
pub fn follow_outputs() {
    static FOLLOW: Once = Once::new();
    FOLLOW.call_once(|| {
        // a subscription only hears changes: the service is started (peeked at, not watched)
        drop(Outputs::peek());
        Outputs::subscribe(|outputs| {
            let names: Vec<String> = outputs.all().iter().map(|o| o.name.clone()).collect();
            let mut known = NAMES.lock().unwrap_or_else(PoisonError::into_inner);
            // none known yet (not niri, or not listed yet) keeps everything
            if !names.is_empty() && known.as_ref() != Some(&names) {
                *known = Some(names);
                GENERATION.fetch_add(1, Ordering::Relaxed);
            }
        })
        .forever();
    });
}

/// A `T` for each monitor, made the first time it's asked for.
pub struct PerOutput<T> {
    map: RefCell<BTreeMap<String, T>>,
    // the outputs' generation the map last let go against
    seen: Cell<u64>,
}

impl<T> Default for PerOutput<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PerOutput<T> {
    pub const fn new() -> Self {
        Self {
            map: RefCell::new(BTreeMap::new()),
            seen: Cell::new(0),
        }
    }

    /// Runs `f` on `output`'s `T`, made by `init` if there's none yet.
    pub fn with<R>(
        &self,
        output: &str,
        init: impl FnOnce() -> T,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        self.forget_gone();
        let mut map = self.map.borrow_mut();
        if !map.contains_key(output) {
            map.insert(output.to_string(), init());
        }
        let value = map.get_mut(output).expect("inserted above");
        f(value)
    }

    /// A copy of `output`'s `T`, if it has one.
    pub fn get(&self, output: &str) -> Option<T>
    where
        T: Clone,
    {
        self.forget_gone();
        self.map.borrow().get(output).cloned()
    }

    /// Runs `f` on every monitor's `T`, by name.
    pub fn all<R>(&self, f: impl FnOnce(&mut BTreeMap<String, T>) -> R) -> R {
        self.forget_gone();
        f(&mut self.map.borrow_mut())
    }

    /// Keeps only the monitors in `outputs`.
    pub fn retain(&self, outputs: &[String]) {
        self.map
            .borrow_mut()
            .retain(|name, _| outputs.iter().any(|o| o == name));
    }

    fn forget_gone(&self) {
        let now = GENERATION.load(Ordering::Relaxed);
        if self.seen.replace(now) == now {
            return;
        }
        let names = NAMES.lock().unwrap_or_else(PoisonError::into_inner).clone();
        if let Some(names) = names {
            self.retain(&names);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_monitor_has_its_own_and_an_unplugged_one_goes() {
        let map: PerOutput<u32> = PerOutput::new();
        map.with("eDP-1", || 1, |n| *n += 1);
        map.with("HDMI-A-1", || 10, |_| {});
        assert_eq!(map.get("eDP-1"), Some(2));
        map.retain(&[String::from("eDP-1")]);
        assert_eq!(map.get("HDMI-A-1"), None);
        assert_eq!(map.with("HDMI-A-1", || 10, |n| *n), 10);
    }
}
