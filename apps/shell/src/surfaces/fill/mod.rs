//! Password fill in other apps: the shell is the seat's input method, so when an app's
//! password field has the keyboard a small popup by the field offers that app's Bitwarden
//! entries, and a click fills one in. See `docs/shell/passwords.md`.
//!
//! - **What it trusts.** Who owns the field comes from the compositor, and the app id the
//!   window claims is only believed once [`identity`] checked it against what's installed. An
//!   unverified app gets no suggestions, only "Search vault…".
//! - **How it fills.** On a click, the password is read from rbw and handed to
//!   `FocusedField::fill`. niri puts it in only if the same field of the same app still has
//!   the keyboard. Nothing is typed with a virtual keyboard, and the popup never takes the
//!   keyboard: the app keeps its keys.
//! - **Where it stays away.** The shell's own fields, terminals, browsers (their Bitwarden
//!   extension checks the site), apps set aside in Shell Settings, and while the screen is
//!   shared. Username fields only get it when asked for in Settings, and only with a
//!   remembered or named entry.
//!
//! [`identity`]: crate::services::vault::identity

mod actions;
mod ipc;
mod view;

use actions::*;
use amane_kit::sync::Locked as _;
pub use ipc::run;
#[cfg(test)]
use ipc::{valid_email, valid_server};
use view::*;

use amane_kit::shell_ipc::Passwords as Command;
use std::sync::Mutex;

use amane::{
    Center, Color, Column, FieldPurpose, FillError, FocusedField, Popup, Rectangle, Row, Service,
    Size, Text, Widget,
};
use amane_kit::buttons::{Actions, hovered, interactive};
use amane_kit::layout::{boxed, icon, pad};
use amane_kit::settings::{self, AppLink, Passwords};
use amane_kit::style::{self as theme, px};
use amane_kit::tokens::Styled;

use crate::services::desktop::privacy::Privacy;
use crate::services::vault::identity::{Identity, Verified};
use crate::services::vault::matching::{self, Match, Reason};
use crate::services::vault::secret::Secret;
use crate::services::vault::{self, Identities, Status, Vault, rbw};
use amane_kit::icons::{ICON_CLOSE, ICON_LOCK, ICON_SEARCH, ICON_WARNING};

const NS: &str = "amane.fill";

/// The most entries a popup shows.
const ROWS: usize = 5;

const WIDTH: f32 = 300.0;

/// What a field gets filled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum What {
    Password,
    Username,
    Code,
}

// what the popup remembers between frames, by the field's generation
struct State {
    // the field the user closed the popup for
    dismissed: Option<u64>,

    // the field the user asked suggestions for, with the hotkey
    forced: Option<u64>,

    // the row asking "fill anyway?", over a field that has text
    confirming: Option<(u64, String)>,

    // what the last fill said, for that field
    note: Option<(u64, String)>,

    // whether a fill is on its way
    filling: bool,

    remember: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    dismissed: None,
    forced: None,
    confirming: None,
    note: None,
    filling: false,
    remember: true,
});

amane_kit::marker! {
    /// Something the popup shows changed outside a service.
    pub FillChanged
}

fn with<T>(edit: impl FnOnce(&mut State) -> T) -> T {
    let mut state = STATE.locked();
    let value = edit(&mut state);
    drop(state);

    value
}

/// The popup by the focused field, for `App::input_method`.
pub fn popup() -> Popup {
    FillChanged::watch();

    let hidden = || {
        Popup::new(NS)
            .size(1, 1)
            .anchor_rect(0, 0, 1, 1)
            .visible(false)
    };

    let field = FocusedField::read();
    let generation = field.generation();

    let Some(shown) = plan(&field) else {
        return hidden();
    };
    drop(field);

    let (rows, height) = content(&shown, generation);

    Popup::new(NS)
        .size(WIDTH.ceil() as u32, height.ceil() as u32)
        .anchor_rect(0, 0, 1, 1)
        .visible(true)
        .child(
            Rectangle::new()
                .width(Size::Fixed(WIDTH))
                .height(Size::Fixed(height))
                .radius(px(8.0))
                .fill(theme::menu_fill())
                .border(1.0, theme::field_border())
                .padding(pad(px(4.0), px(4.0)))
                .clip()
                .child(Column::new(rows).width(Size::Parent)),
        )
}

// what the popup shows for the field, or none to stay away
struct Shown {
    what: What,
    identity: Option<Identity>,
    status: Status,
    matches: Vec<Match>,
    empty: Option<bool>,
}

/*
 * fetches the secret off the UI thread and hands it to the field of `generation`; niri puts
 * it in only if that field still has the keyboard. A verified app the user lets remember gets
 * the entry linked to it
 */
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_must_be_safe() {
        assert!(valid_server("https://vault.tail8c36e9.ts.net"));
        assert!(valid_server("http://localhost:8080"));
        assert!(valid_server("http://[::1]:8080"));
        assert!(!valid_server("http://localhost.evil.com"));
        assert!(!valid_server("http://vault.example.com"));
        assert!(!valid_server("ftp://vault"));
        assert!(!valid_server("https://"));
        assert!(!valid_server("https://a b"));
        assert!(valid_email("me@ocbk.no"));
        assert!(!valid_email("me"));
        assert!(!valid_email("me @x.no"));
    }
}
