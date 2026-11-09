//! Status apps: a program's own item in emaki's status bar, and its
//! activities (something going on now, like a download or a train on its
//! way), over the bar's session-bus service `org.emaki.StatusBar1`.
//!
//! The wire types ([`ItemSpec`], [`ActivityContent`] and the rest, sent as
//! versioned JSON) are shared with the bar, which serves the interface. A
//! program uses the client: [`StatusItem::register`] and
//! [`LiveActivity::start`], which register again whenever the bar restarts.
//! See `docs/developer/integration/status-apps.md#writing-a-status-app`.

#![deny(missing_docs)]
// no unsafe code: what needs it (syscalls, FFI) lives in emaki-base or amane
#![forbid(unsafe_code)]

mod client;
mod protocol;

pub use client::{LiveActivity, StatusItem};
pub use protocol::{
    Accepts, ActivityButton, ActivityContent, ActivityEvent, BUS_NAME, BarEdge, Dismiss, INTERFACE,
    ItemButton, ItemEvent, ItemIcon, ItemSpec, MenuEntry, PATH, ProtocolError, Tint, Trailing,
    VERSION, encode,
};
