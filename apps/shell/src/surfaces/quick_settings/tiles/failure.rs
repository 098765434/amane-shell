//! How long a service's failure shows: amane keeps a failure until the next
//! try, and a row shows it for [`SHOWN_FOR`] after it happened, then draws
//! again without it.

use std::cell::Cell;
use std::time::{Duration, Instant};

use amane::{Service, ServiceError};
use amane_kit::motion;

/// How long a row shows a failure after it happened.
pub const SHOWN_FOR: Duration = Duration::from_secs(10);

amane_kit::marker! {
    /// A failure's time to show ran out: the rows that showed it draw again.
    Expired
}

thread_local! {
    // the soonest frame asked for, so a row drawn often asks once
    static DUE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// `failure` while it is recent enough to show, asking for a frame for when
/// it no longer is.
pub fn recent(failure: Option<&ServiceError>) -> Option<&ServiceError> {
    let failure = failure?;
    let age = failure.at.elapsed();
    if age >= SHOWN_FOR {
        return None;
    }
    drop(Expired::read());
    let left = SHOWN_FOR - age;
    let now = Instant::now();
    let due = now + left;
    if DUE.get().is_some_and(|asked| asked > now && asked <= due) {
        return Some(failure);
    }
    DUE.set(Some(due));
    motion::redraw_after::<Expired>(left);
    Some(failure)
}
