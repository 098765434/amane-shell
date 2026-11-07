//! What a service's own listener does with what it reads.

use amane::Service;

/// For a service's listener, which reads its source and keeps what it found.
pub trait ServiceExt: Service + PartialEq {
    /// Puts `fresh` in place when it differs from what the service has, so
    /// only a change redraws; says whether it did. It compares through
    /// [`Service::peek`]: a listener's own look doesn't count as something
    /// watching the service, which would keep it from resting while idle.
    fn replace(fresh: Self) -> bool {
        if *Self::peek() == fresh {
            return false;
        }
        *Self::write() = fresh;
        true
    }
}

impl<S: Service + PartialEq> ServiceExt for S {}
