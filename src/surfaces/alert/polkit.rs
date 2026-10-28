//! polkitd asking the user to prove themselves — pkexec, `systemctl
//! start`, a Flatpak install — asks through the shell's alert: the action's
//! own sentence, a password field, and what went wrong with the last try.
//! The shell is the session's polkit agent (`Polkit::serve_agent()`), so
//! this stands in for hyprpolkitagent and the like.
//!
//! Synced from the alert surface's view, which reads `Polkit` for it and
//! so runs again whenever a request comes, changes or goes.

use std::cell::Cell;

use amane::{AuthRequest, Polkit, Service, TextInput};

use crate::surfaces::alert;
use crate::components::dialog::{Dialog, Field, Icon};
use crate::components::icons::ICON_LOCK;

/// The alert's name, so only this alert is closed when polkitd stops asking.
const NAME: &str = "amane.alert.polkit";

/// The password field's id; its text is read when the dialog answers.
const FIELD: &str = "amane.polkit.password";

thread_local! {
    // the request shown, and the state of it the dialog was built from
    static SHOWN: Cell<Option<(u64, u64)>> = const { Cell::new(None) };
}

/// Shows the alert for a new request, rebuilds it when the request moved
/// on (a prompt, a wrong password), or puts it away once polkitd stopped
/// asking.
pub fn sync() {
    let polkit = Polkit::read();
    let request = polkit.auth_request().cloned();
    drop(polkit);

    let Some(request) = request else {
        if SHOWN.take().is_some() {
            if alert::showing(NAME) {
                alert::close();
            }
            TextInput::set_text(FIELD, "");
        }
        return;
    };
    let state = (request.id(), version(&request));
    if SHOWN.get() == Some(state) {
        return;
    }
    if SHOWN.get().is_none_or(|(id, _)| id != request.id()) {
        TextInput::set_text(FIELD, "");
    }
    SHOWN.set(Some(state));
    alert::show(dialog(&request));
}

/// What of the request the dialog shows, so it's rebuilt only when that
/// changed.
fn version(request: &AuthRequest) -> u64 {
    let mut version = u64::from(request.tries()) << 8;
    version |= request.info().len() as u64 & 0x3f;
    version |= u64::from(request.prompt().is_some()) << 6;
    version |= u64::from(request.busy()) << 7;
    version | (u64::from(request.error().is_some()) << 16)
}

fn dialog(request: &AuthRequest) -> Dialog {
    let id = request.id();
    let cancel = move || {
        TextInput::set_text(FIELD, "");
        Polkit::cancel(id);
    };
    let answer = move || {
        let typed = TextInput::text(FIELD);
        TextInput::set_text(FIELD, "");
        Polkit::answer(id, typed);
    };

    let (placeholder, secret) = match request.prompt() {
        Some(prompt) => (
            prompt.text().trim().trim_end_matches(':').to_string(),
            !prompt.echo(),
        ),
        None => (String::from("Password"), true),
    };
    let placeholder = if placeholder.is_empty() {
        String::from("Password")
    } else {
        placeholder
    };

    let mut shown = Dialog::new(NAME, "Authentication Required")
        .icon(Icon::Symbol(ICON_LOCK))
        .message(request.message().trim());
    for line in request.info() {
        let line = line.trim();
        if !line.is_empty() {
            shown = shown.note(line);
        }
    }
    if let Some(error) = request.error() {
        shown = shown.error(error.trim());
    } else if request.busy() {
        shown = shown.note("Checking…");
    }
    shown = shown.note(format!("{} · as {}", request.action_id(), request.user()));
    shown
        .field(Field {
            id: FIELD,
            placeholder,
            secret,
        })
        .button("Cancel", cancel)
        .main_button("Authenticate", answer)
}
