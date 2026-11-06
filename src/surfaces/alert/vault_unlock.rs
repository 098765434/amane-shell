//! rbw asking for the master password (or a two-factor code, or an API key) through
//! amane-pinentry, for an unlock the shell started: the shell's alert, with a title the shell
//! sets and rbw's words under it as plain text. The answer is taken out of the field, which is
//! left empty, and goes back over the private socket, wiped once sent.
//!
//! Synced from the alert surface's view, which reads `PinEntry` for it and so runs again when a
//! question comes or goes.

use std::cell::Cell;

use amane::{Service, TextInput};

use crate::components::dialog::{Dialog, Field, Icon};
use crate::components::icons::ICON_LOCK;
use crate::services::vault::pinentry::{self, PinEntry, PinQuestion};
use crate::surfaces::alert;

const NAME: &str = "amane.alert.vault";

/// The field's id; its text is taken out when the dialog answers.
const FIELD: &str = "amane.vault.pin";

thread_local! {
    // the question shown
    static SHOWN: Cell<Option<u64>> = const { Cell::new(None) };
}

pub fn sync() {
    let question = PinEntry::read().question().cloned();

    let Some(question) = question else {
        if SHOWN.take().is_some() {
            if alert::showing(NAME) {
                alert::close();
            }
            drop(TextInput::take_secret(FIELD));
        }
        return;
    };

    if SHOWN.get() == Some(question.id) {
        return;
    }

    drop(TextInput::take_secret(FIELD));
    SHOWN.set(Some(question.id));
    alert::show(dialog(&question));
}

fn dialog(question: &PinQuestion) -> Dialog {
    let id = question.id;

    let cancel = move || {
        drop(TextInput::take_secret(FIELD));
        pinentry::answer(id, None);
    };
    let answer = move || {
        let pin = TextInput::take_secret(FIELD);
        pinentry::answer(id, Some(pin));
    };

    // hidden as typed unless rbw plainly asks for a two-factor code or the API key's client id
    let lower = question.prompt.to_ascii_lowercase();
    let plain = (lower.contains("code") || lower.contains("client_id"))
        && !lower.contains("password")
        && !lower.contains("secret");
    let secret = !plain;

    let placeholder = match question.prompt.trim().trim_end_matches(':') {
        "" => String::from("Master Password"),
        prompt => prompt.to_string(),
    };

    let mut shown = Dialog::new(NAME, "Unlock Bitwarden")
        .icon(Icon::Symbol(ICON_LOCK))
        .message(if question.description.is_empty() {
            "Enter your master password to unlock the vault."
        } else {
            question.description.as_str()
        });

    if !question.error.is_empty() {
        shown = shown.error(question.error.as_str());
    }

    shown = shown.note("Asked for by rbw, which the shell started");

    shown
        .field(Field {
            id: FIELD,
            placeholder,
            secret,
        })
        .button("Cancel", cancel)
        .main_button("Unlock", answer)
}
