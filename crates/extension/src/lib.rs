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

pub mod budget;
mod client;
mod lenient;
mod protocol;

pub use client::{LiveActivity, StatusItem, hello, shell_info};
#[allow(deprecated)]
pub use protocol::VERSION;
pub use protocol::{
    Accepts, ActivityButton, ActivityContent, ActivityEvent, BUS_NAME, BarEdge, Dismiss, Hello,
    INTERFACE, ItemButton, ItemEvent, ItemIcon, ItemSpec, LEVEL, MenuEntry, OLDEST_LEVEL, PATH,
    ProtocolError, ShellInfo, Tint, Trailing, encode,
};
