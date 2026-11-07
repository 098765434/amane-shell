//! Locks that outlive a panic.
//!
//! A thread that panics while holding a lock poisons it. Everything kept
//! behind a lock here is a cache or a latest reading, still sound after a
//! panic elsewhere, so a poisoned lock is used as it was left.

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// [`Mutex::lock`], poisoned or not.
pub trait Locked<T: ?Sized> {
    /// Takes the lock, waiting for it.
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// [`RwLock::read`] and [`RwLock::write`], poisoned or not.
pub trait Shared<T: ?Sized> {
    /// Takes the lock to read, waiting for any writer.
    fn reading(&self) -> RwLockReadGuard<'_, T>;
    /// Takes the lock to write, waiting for everyone.
    fn writing(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T: ?Sized> Shared<T> for RwLock<T> {
    fn reading(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn writing(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_is_used_as_it_was_left() {
        let lock = std::sync::Arc::new(Mutex::new(1));
        let held = lock.clone();
        let _ = std::thread::spawn(move || {
            let _guard = held.lock();
            panic!("poisons the lock");
        })
        .join();
        assert!(lock.is_poisoned());
        *lock.locked() += 1;
        assert_eq!(*lock.locked(), 2);

        let shared = RwLock::new(String::new());
        shared.writing().push('a');
        assert_eq!(*shared.reading(), "a");
    }
}
