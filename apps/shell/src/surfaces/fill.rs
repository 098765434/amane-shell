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

use std::sync::Mutex;

use amane::{
    Center, Color, Column, FieldPurpose, FillError, FocusedField, Popup, Rectangle, Row, Service,
    Size, Text, Widget,
};
use amane_kit::buttons::{Actions, hovered, interactive};
use amane_kit::layout::{boxed, icon, pad};
use amane_kit::macform::theme::{self, px};
use amane_kit::settings::{self, AppLink, Passwords};
use amane_kit::tokens::Styled;

use crate::components::icons::{ICON_CLOSE, ICON_LOCK, ICON_SEARCH, ICON_WARNING};
use crate::services::desktop::privacy::Privacy;
use crate::services::vault::identity::{Identity, Verified};
use crate::services::vault::matching::{self, Match, Reason};
use crate::services::vault::secret::Secret;
use crate::services::vault::{self, Identities, Status, Vault, rbw};

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
    let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
    let value = edit(&mut state);
    drop(state);

    value
}

/// `amane-shell` IPC, for keybinds and Shell Settings' Passwords page:
///
/// - `passwords suggest`: the popup for the field that has the keyboard, even a username or
///   terminal field;
/// - `passwords status`: the vault's state, one `key=value` a line, never a secret;
/// - `passwords unlock | sign-in | register | sync | lock`;
/// - `passwords configure <server> <email> [forget]`, `passwords lock-timeout <seconds>`.
///
/// Unlocking from here is still the shell's own rbw command, so its dialog may ask.
pub fn ipc(args: &[String]) -> String {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        ["suggest"] => {
            let generation = FocusedField::read().generation();

            with(|state| {
                state.forced = Some(generation);
                state.dismissed = None;
            });
            FillChanged::changed();

            Vault::look_now();
        }
        ["status"] => return status(),
        ["unlock"] => Vault::unlock(),
        ["sign-in"] => Vault::sign_in(),
        ["register"] => Vault::register(),
        ["sync"] => Vault::sync(),
        ["lock"] => Vault::lock(),
        ["configure", server, email, rest @ ..] => {
            if !valid_server(server) || !valid_email(email) {
                return String::from(
                    "error: the server must be https:// (or http://localhost), and the email an address",
                );
            }

            Vault::configure(
                server.to_string(),
                email.to_string(),
                rest.first() == Some(&"forget"),
            );
        }
        ["lock-timeout", seconds] => match seconds.parse() {
            Ok(seconds) => Vault::set_lock_timeout(seconds),
            Err(_) => return String::from("error: seconds"),
        },
        _ => {
            return String::from(
                "usage: passwords [suggest | status | unlock | sign-in | register | sync | lock | configure <server> <email> [forget] | lock-timeout <seconds>]",
            );
        }
    }

    String::from("ok")
}

/// A server rbw may talk to: https, or plain http only to this computer.
pub fn valid_server(server: &str) -> bool {
    let lower = server.to_ascii_lowercase();

    let rest = if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("http://") {
        let authority = rest.split('/').next().unwrap_or("");
        let host = match authority.strip_prefix('[') {
            Some(inner) => inner.split(']').next().map(|host| format!("[{host}]")),
            None => authority.split(':').next().map(String::from),
        }
        .unwrap_or_default();
        if !matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") {
            return false;
        }
        rest
    } else {
        return false;
    };

    !rest.is_empty()
        && !server
            .chars()
            .any(|letter| letter.is_whitespace() || letter.is_control())
}

fn valid_email(email: &str) -> bool {
    email.len() <= 254
        && email
            .split_once('@')
            .is_some_and(|(name, domain)| !name.is_empty() && domain.contains('.'))
        && !email
            .chars()
            .any(|letter| letter.is_whitespace() || letter.is_control())
}

// the vault's state for the Passwords page, without anything secret
fn status() -> String {
    let vault = Vault::read();
    let field = FocusedField::read();

    let config = vault.config().cloned().unwrap_or_default();

    let mut lines = vec![
        format!("status={:?}", vault.status()),
        format!(
            "entries={}",
            vault
                .entries()
                .iter()
                .filter(|entry| entry.is_login())
                .count()
        ),
        format!(
            "busy={}",
            vault
                .busy()
                .map(|busy| format!("{busy:?}"))
                .unwrap_or_default()
        ),
        format!(
            "error={}",
            vault
                .error()
                .map(|error| error.describe())
                .unwrap_or_default()
        ),
        format!("server={}", config.base_url),
        format!("email={}", config.email),
        format!("lock_timeout={}", config.lock_timeout),
        format!("pinentry_ok={}", config.pinentry == vault::pinentry_path()),
        format!(
            "input_method={}",
            if !settings::passwords().fill_in_apps {
                "off"
            } else if field.available() {
                "on"
            } else {
                "unavailable"
            }
        ),
        format!("secure_fill={}", field.can_fill()),
    ];

    if let Some(synced) = vault
        .synced()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
    {
        lines.push(format!("synced={}", synced.as_secs()));
    }

    lines.join("\n")
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

fn plan(field: &FocusedField) -> Option<Shown> {
    let settings = settings::passwords();

    if !settings.fill_in_apps || !field.can_fill() || !field.active() || field.own() {
        return None;
    }

    let generation = field.generation();

    let (dismissed, forced) = with(|state| (state.dismissed, state.forced));
    if dismissed == Some(generation) {
        return None;
    }
    let forced = forced == Some(generation);

    let owner = field.owner()?.clone();
    if !owner.takes_text || settings.excludes(&owner.app_id) {
        return None;
    }

    if settings.hide_while_sharing && Privacy::read().screen() {
        return None;
    }

    let what = match field.purpose() {
        _ if field.is_password() => What::Password,
        FieldPurpose::Terminal => {
            if !forced {
                return None;
            }
            What::Password
        }
        FieldPurpose::Digits | FieldPurpose::Pin => What::Code,
        _ => What::Username,
    };

    let vault = Vault::read();
    let status = vault.status();

    // nothing set up: only when asked, so a fresh install isn't nagged
    if status == Status::NotConfigured && !forced {
        return None;
    }

    let identity = Identities::of(&owner);

    // a browser is many sites in one app: its Bitwarden extension knows the site, the shell can't
    if matches!(&identity, Some(Identity::Verified(app)) if app.browser) {
        return None;
    }

    let matches = match (&identity, status) {
        (Some(Identity::Verified(app)), Status::Unlocked) => {
            matching::matches(app, &owner.app_id, &vault.entries(), &settings)
        }
        _ => Vec::new(),
    };
    drop(vault);

    // a username or code field only with an entry known for this app, or when asked
    let strong = matches
        .iter()
        .any(|found| matches!(found.reason, Reason::Remembered | Reason::Named));

    match what {
        What::Password => {}
        What::Username if forced || (settings.suggest_on_usernames && strong) => {}
        What::Code if forced || (field.empty() == Some(true) && strong) => {}
        _ => return None,
    }

    Some(Shown {
        what,
        identity,
        status,
        matches,
        empty: field.empty(),
    })
}

fn content(shown: &Shown, generation: u64) -> (Vec<Box<dyn Widget>>, f32) {
    let row_height = theme::menu_item_height();
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();

    rows.push(boxed(header(shown, generation)));

    let verified = match &shown.identity {
        Some(Identity::Verified(app)) => Some(app.clone()),
        _ => None,
    };

    match shown.status {
        Status::Unlocked => {
            for found in shown.matches.iter().take(ROWS) {
                rows.push(boxed(entry_row(shown, found, verified.clone(), generation)));
            }
        }
        Status::Locked => rows.push(boxed(action_row(
            "fill.unlock",
            ICON_LOCK,
            "Unlock Bitwarden…",
            Vault::unlock,
        ))),
        Status::LoggedOut => rows.push(boxed(action_row(
            "fill.sign-in",
            ICON_LOCK,
            "Sign in to Bitwarden…",
            Vault::sign_in,
        ))),
        Status::NotConfigured => rows.push(boxed(action_row(
            "fill.set-up",
            ICON_LOCK,
            "Set up Passwords…",
            open_settings,
        ))),
    }

    rows.push(boxed(action_row(
        "fill.search",
        ICON_SEARCH,
        "Search vault…",
        search,
    )));

    let note = with(|state| state.note.clone()).filter(|(at, _)| *at == generation);
    if let Some((_, note)) = note {
        rows.push(boxed(note_row(&note)));
    }

    if shown.status == Status::Unlocked && verified.is_some() && !shown.matches.is_empty() {
        rows.push(boxed(remember_row(verified.as_ref())));
    }

    let height = rows.len() as f32 * row_height + px(8.0);

    (rows, height)
}

// who the field belongs to, as checked, and a close button
fn header(shown: &Shown, generation: u64) -> Rectangle {
    let (text, warn) = match &shown.identity {
        Some(Identity::Verified(app)) => (format!("Passwords for {}", app.name), false),
        Some(Identity::Unverified(why)) => (format!("Unverified app · {}", why.describe()), true),
        None => (String::from("Checking the app…"), false),
    };

    let color = if warn {
        theme::error()
    } else {
        theme::label_secondary()
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();
    if warn {
        parts.push(boxed(icon(ICON_WARNING, px(13.0), color)));
    }
    parts.push(boxed(
        Text::new(text)
            .style(theme::subtitle_font())
            .color(color)
            .elide(),
    ));

    let close = interactive(
        Rectangle::new()
            .width(Size::Fixed(px(20.0)))
            .height(Size::Fixed(px(20.0)))
            .radius(px(10.0))
            .fill(hover("fill.close"))
            .align_child(Center, Center)
            .child(icon(ICON_CLOSE, px(13.0), theme::label_secondary())),
        String::from("fill.close"),
        false,
        press(move || {
            with(|state| state.dismissed = Some(generation));
            FillChanged::changed();
        }),
    );
    parts.push(boxed(close));

    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(theme::menu_item_height()))
        .padding(pad(px(8.0), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Row::new(parts)
                .gap(px(6.0))
                .align(Center)
                .width(Size::Parent),
        )
}

fn entry_row(shown: &Shown, found: &Match, app: Option<Verified>, generation: u64) -> Rectangle {
    let entry = found.entry.clone();
    let what = shown.what;
    let key = format!("fill.entry.{}", entry.id);

    let confirming = with(|state| state.confirming.clone()) == Some((generation, entry.id.clone()));
    let has_text = shown.empty == Some(false);

    let label = if confirming {
        String::from("The field has text: fill anyway?")
    } else {
        match (&entry.user, what) {
            (Some(user), What::Password | What::Code) => format!("{} · {user}", entry.name),
            _ => entry.name.clone(),
        }
    };

    let detail = match what {
        What::Password => "password",
        What::Username => "username",
        What::Code => "code",
    };

    let parts: Vec<Box<dyn Widget>> = vec![
        boxed(
            Text::new(label)
                .style(theme::control_font())
                .color(theme::label_primary())
                .elide(),
        ),
        boxed(
            Text::new(detail)
                .style(theme::subtitle_font())
                .color(theme::label_tertiary()),
        ),
    ];

    let id = entry.id.clone();
    let remembered = found.reason == Reason::Remembered;

    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(theme::menu_item_height()))
            .radius(px(6.0))
            .fill(hover(&key))
            .padding(pad(px(10.0), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Row::new(parts)
                    .gap(px(8.0))
                    .align(Center)
                    .width(Size::Parent),
            ),
        key,
        false,
        press(move || {
            if has_text && !confirming {
                with(|state| state.confirming = Some((generation, id.clone())));
                FillChanged::changed();
                return;
            }

            fill(
                &entry,
                what,
                generation,
                app.clone().filter(|_| !remembered),
            );
        }),
    )
}

fn action_row(key: &'static str, glyph: char, text: &str, run: fn()) -> Rectangle {
    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(theme::menu_item_height()))
            .radius(px(6.0))
            .fill(hover(key))
            .padding(pad(px(10.0), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Row::new(vec![
                    boxed(icon(glyph, px(15.0), theme::label_secondary())),
                    boxed(
                        Text::new(text.to_string())
                            .style(theme::control_font())
                            .color(theme::label_primary()),
                    ),
                ])
                .gap(px(8.0))
                .align(Center),
            ),
        String::from(key),
        false,
        press(run),
    )
}

fn note_row(note: &str) -> Rectangle {
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(theme::menu_item_height()))
        .padding(pad(px(10.0), 0.0))
        .align_child(amane::Start, Center)
        .child(
            Text::new(note.to_string())
                .style(theme::subtitle_font())
                .color(theme::error())
                .elide(),
        )
}

// whether a fill is remembered for this app, a checkbox in words
fn remember_row(app: Option<&Verified>) -> Rectangle {
    let on = with(|state| state.remember);
    let name = app.map_or("this app", |app| app.name.as_str());

    let text = format!(
        "{} Remember the choice for {name}",
        if on { "☑" } else { "☐" }
    );

    interactive(
        Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(theme::menu_item_height()))
            .radius(px(6.0))
            .fill(hover("fill.remember"))
            .padding(pad(px(10.0), 0.0))
            .align_child(amane::Start, Center)
            .child(
                Text::new(text)
                    .style(theme::subtitle_font())
                    .color(theme::label_secondary())
                    .elide(),
            ),
        String::from("fill.remember"),
        false,
        press(|| {
            with(|state| state.remember = !state.remember);
            FillChanged::changed();
        }),
    )
}

fn hover(key: &str) -> Color {
    if hovered(key) {
        theme::hover_of(theme::label_primary())
    } else {
        Color::TRANSPARENT
    }
}

fn press(run: impl Fn() + 'static) -> Actions {
    Actions {
        press: Some(std::rc::Rc::new(run)),
        ..Default::default()
    }
}

fn search() {
    crate::surfaces::launcher::ipc(&[String::from("open"), String::from("vault")]);
}

fn open_settings() {
    crate::services::run(
        "amane-settings",
        vec![String::from("--page"), String::from("passwords")],
    );
}

/*
 * fetches the secret off the UI thread and hands it to the field of `generation`; niri puts
 * it in only if that field still has the keyboard. A verified app the user lets remember gets
 * the entry linked to it
 */
fn fill(entry: &rbw::Entry, what: What, generation: u64, remember_for: Option<Verified>) {
    if with(|state| std::mem::replace(&mut state.filling, true)) {
        return;
    }

    let remember = with(|state| state.remember);
    let entry = entry.clone();

    std::thread::spawn(move || {
        // locked since the last look: say so, rather than let rbw ask in a dialog not the shell's
        if !rbw::unlocked() {
            Vault::look_now();
            return done(
                generation,
                Err(String::from("The vault locked; unlock it first")),
            );
        }

        let secret = match what {
            What::Password => rbw::password(&entry.id),
            What::Code => rbw::totp(&entry.id),
            What::Username => entry
                .user
                .as_deref()
                .and_then(|user| Secret::from_output(user.as_bytes()))
                .ok_or(rbw::Error::NotFound),
        };

        let secret = match secret {
            Ok(secret) if !secret.is_empty() => secret,
            Ok(_) => {
                return done(
                    generation,
                    Err(String::from("That entry has nothing to fill")),
                );
            }
            Err(error) => return done(generation, Err(error.describe().to_string())),
        };

        FocusedField::fill(secret.into_text(), generation, move |result| match result {
            Ok(()) => {
                if remember && let Some(app) = remember_for {
                    settings::change_passwords(|passwords: &mut Passwords| {
                        passwords.remember(AppLink {
                            desktop_id: app.desktop_id.clone(),
                            package: app.package.clone(),
                            entry_id: entry.id.clone(),
                            entry_name: entry.name.clone(),
                        });
                    });
                }

                done(generation, Ok(()));
            }
            Err(error) => done(generation, Err(describe(error).to_string())),
        });
    });
}

fn done(generation: u64, result: Result<(), String>) {
    with(|state| {
        state.filling = false;
        state.confirming = None;

        match result {
            // filled: the popup goes away for this field
            Ok(()) => {
                state.note = None;
                state.dismissed = Some(generation);
            }
            Err(note) => state.note = Some((generation, note)),
        }
    });

    FillChanged::changed();
}

fn describe(error: FillError) -> &'static str {
    match error {
        FillError::Changed => "The field changed, so nothing was filled",
        FillError::NoTextInput => "This field takes no text from the shell",
        FillError::Locked => "The screen is locked",
        FillError::Expired => "Too long since the field was focused; click again",
        FillError::Used => "Already filled; click again to fill once more",
        FillError::TooSoon => "Too quick; click again",
        FillError::InvalidText => "That entry can't be typed into a field",
        FillError::Unsupported => "This compositor can't fill fields safely",
    }
}

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
