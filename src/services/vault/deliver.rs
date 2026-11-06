//! A password picked in the launcher, into the window that was behind it: once that window's
//! field has the keyboard again, through the same checked fill and checks as the popup. Copy
//! puts it on the clipboard instead, only when asked, marked as a password and cleared after 30
//! seconds. Nothing is typed with a virtual keyboard.

use std::thread;
use std::time::{Duration, Instant};

use amane::{Clipboard, FillError, FocusedField, Notify, Service, Windows};

use super::rbw::{self, Entry};
use super::secret::Secret;

/// How long the launcher's window may take to hand the keyboard back.
const REFOCUS: Duration = Duration::from_millis(1200);

/// How long a copied password stays on the clipboard.
pub const CLEAR_AFTER: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Give {
    Password,
    Username,
    Code,
}

impl Give {
    pub fn noun(self) -> &'static str {
        match self {
            Give::Password => "password",
            Give::Username => "username",
            Give::Code => "code",
        }
    }
}

/// Fills the window behind the launcher, captured now as the launcher closes, with the same
/// checks as the popup: a verified app, not a browser or one set aside, not sandboxed, and a
/// password only into a password field. Nothing is copied unless asked for.
pub fn fill_behind(entry: Entry, give: Give) {
    let window = Windows::read().focused().map(|window| window.id());

    thread::spawn(move || {
        let Some(window) = window else {
            return tell("Nothing was filled", "No window was behind Spotlight.");
        };

        let Some(field) = wait_for_field(window) else {
            return tell(
                "Nothing was filled",
                "That window has no field the shell can fill. Copy password puts it on the \
                 clipboard instead.",
            );
        };

        if let Err(why) = allowed(&field, give) {
            return tell("Nothing was filled", why);
        }

        if !rbw::unlocked() {
            super::Vault::look_now();
            return tell(
                "Nothing was filled",
                "The vault is locked. Unlock it first.",
            );
        }

        let secret = match fetch(&entry, give) {
            Ok(secret) => secret,
            Err(error) => {
                return tell(
                    &format!("Couldn't get the {}", give.noun()),
                    error.describe(),
                );
            }
        };

        FocusedField::fill(secret.into_text(), field.generation, move |result| {
            if let Err(error) = result {
                tell(
                    "Nothing was filled",
                    match error {
                        FillError::Changed => "Another window took the keyboard first",
                        FillError::Locked => "The screen is locked",
                        _ => "The field couldn't take it",
                    },
                );
            }
        });
    });
}

/*
 * puts an entry's password, username or code on the clipboard, marked as a password, while the
 * launcher still has the keyboard (a compositor takes a copy only from the focused client); so
 * it runs before the launcher closes, and blocks for as long as rbw takes
 */
pub fn copy_entry(entry: &Entry, give: Give) {
    if !rbw::unlocked() {
        super::Vault::look_now();
        return tell(
            "Nothing was copied",
            "The vault is locked. Unlock it first.",
        );
    }

    match fetch(entry, give) {
        Ok(secret) => {
            Clipboard::copy_secret(secret.into_text(), CLEAR_AFTER);

            tell(
                &format!("Copied the {}", give.noun()),
                "Marked as a password, and cleared from the clipboard after 30 seconds.",
            );
        }
        Err(error) => tell(
            &format!("Couldn't get the {}", give.noun()),
            error.describe(),
        ),
    }
}

// the field that came back, as the popup checks it
struct Field {
    generation: u64,
    owner: amane::FieldOwner,
    password: bool,
}

fn allowed(field: &Field, give: Give) -> Result<(), &'static str> {
    use super::identity::{self, Identity};

    let settings = amane_kit::settings::passwords();

    if settings.excludes(&field.owner.app_id) {
        return Err("Fill isn't offered in this app (Shell Settings › Passwords).");
    }
    drop(settings);

    match identity::check(&field.owner) {
        Identity::Unverified(why) => Err(why.describe()),
        Identity::Verified(app) if app.browser => {
            Err("Browsers are left to the Bitwarden extension, which checks the site.")
        }
        Identity::Verified(_) if give == Give::Password && !field.password => {
            Err("The field there isn't a password field.")
        }
        Identity::Verified(_) => Ok(()),
    }
}

fn fetch(entry: &Entry, give: Give) -> Result<Secret, rbw::Error> {
    match give {
        Give::Password => rbw::password(&entry.id),
        Give::Code => rbw::totp(&entry.id),
        Give::Username => entry
            .user
            .as_deref()
            .and_then(|user| Secret::from_output(user.as_bytes()))
            .ok_or(rbw::Error::NotFound),
    }
}

// the field of `window` once it has the keyboard again
fn wait_for_field(window: u64) -> Option<Field> {
    let until = Instant::now() + REFOCUS;

    while Instant::now() < until {
        {
            let field = FocusedField::read();

            if field.can_fill()
                && field.active()
                && !field.own()
                && let Some(owner) = field.owner()
                && owner.window_id == Some(window)
                && owner.takes_text
            {
                return Some(Field {
                    generation: field.generation(),
                    owner: owner.clone(),
                    password: field.is_password(),
                });
            }
        }

        thread::sleep(Duration::from_millis(30));
    }

    None
}

fn tell(summary: &str, body: &str) {
    Notify::new(summary)
        .body(body)
        .icon("dialog-password")
        .send();
}
