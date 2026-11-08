//! A phone asking to pair through KDE Connect while Quick Settings' Phone
//! page isn't showing asks through the shell's alert, with the code both
//! screens show; the page itself asks under its list while it shows.
//!
//! The request itself is `services::phone::pairing`'s; the alert's view
//! syncs from it.

use std::cell::RefCell;

use emaki_phone::KdeConnect;

use crate::services::phone::pairing::{self, Asking};

use crate::surfaces::alert;
use crate::surfaces::quick_settings::{self, Page};
use emaki_kit::dialog::Dialog;

/// The alert's name, so only this alert is closed when the phone stops asking.
const NAME: &str = "emaki.alert.phone.pair";

thread_local! {
    // the phone already asked about, by its id
    static ASKED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Shows the alert for a new request, or puts it away once the phone
/// stopped asking.
pub fn sync() {
    let Some(Asking { id, name, key }) = pairing::asking() else {
        if ASKED.take().is_some() && alert::showing(NAME) {
            alert::close();
        }
        return;
    };
    if ASKED.with_borrow(|asked| asked.as_deref() == Some(id.as_str())) {
        return;
    }
    // the page asks under its list
    if quick_settings::showing(Page::Phone) {
        return;
    }
    ASKED.set(Some(id.clone()));
    alert::show(dialog(id, &name, &key));
}

fn dialog(id: String, name: &str, key: &str) -> Dialog {
    let declined = id.clone();
    let message = if key.is_empty() {
        String::from("Pair only with a phone you know.")
    } else {
        format!("Make sure “{name}” shows the code {key}.")
    };
    Dialog::new(NAME, format!("Pair with “{name}”?"))
        .message(message)
        .button("Decline", move || KdeConnect::cancel_pairing(&declined))
        .main_button("Pair", move || KdeConnect::accept_pairing(&id))
}
