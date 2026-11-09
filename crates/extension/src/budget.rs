//! What a program may send, and how often: the numbers the bar holds every
//! program to, so none can wake it every frame or fill its memory. A payload
//! over a limit is refused or clamped where it's read; updates over a rate are
//! held and coalesced into the latest by the bar.
//!
//! See `docs/developer/extensions-plan.md` §2 for where the numbers come from.

use std::time::Duration;

/// The most JSON one call may carry, in bytes.
pub const MAX_PAYLOAD: usize = 64 * 1024;

/// The shortest time between two updates the bar applies to one item or activity.
pub const OBJECT_INTERVAL: Duration = Duration::from_millis(100);

/// The calls a program may make in a second, across everything it put up.
pub const OWNER_RATE: u32 = 20;

/// The updates the bar holds for one program at once; one more is refused as busy.
pub const QUEUE_DEPTH: usize = 10;

/// The buttons or menu entries a tile page or a widget may offer.
pub const MAX_ACTIONS: usize = 5;

/// The buttons an activity's expanded view shows.
pub const MAX_ACTIVITY_BUTTONS: usize = 3;

/// How long an activity lasts before the bar ends it.
pub const MAX_ACTIVITY_AGE: Duration = Duration::from_secs(8 * 60 * 60);

/// The entries one level of an item's menu may have.
pub const MAX_MENU_ENTRIES: usize = 64;

/// How deep submenus may nest; deeper ones are cut off.
pub const MAX_MENU_DEPTH: usize = 4;

/// The characters a title, label, badge or line of text may have.
pub const MAX_TEXT: usize = 256;

/// The characters a tooltip may have.
pub const MAX_TOOLTIP: usize = 1024;
