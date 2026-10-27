//! A device asking to pair while Quick Settings' Bluetooth page isn't
//! showing — a phone pairing from its side, a keyboard waking up — asks
//! through the shell's alert: confirm the code, allow it, or see the code
//! to type on it. A code to type here opens the page, which has the field.
//! The page itself asks under its list while it shows.
//!
//! Synced from the alert surface's view, which reads `Bluetooth` for it
//! and so runs again whenever a request comes or goes.

use std::cell::Cell;

use amane::{Bluetooth, PairingAnswer, PairingKind, PairingRequest, Service};

use crate::components::alert;
use crate::components::dialog::Dialog;
use crate::components::quick_settings::{self, Page};

/// The alert's name, so only this alert is closed when BlueZ stops asking.
const NAME: &str = "amane.alert.bluetooth.pair";

thread_local! {
    // the request already asked about, by id
    static ASKED: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Shows the alert for a new request, or puts it away once BlueZ stopped
/// asking.
pub fn sync() {
    let bluetooth = Bluetooth::read();
    let request = bluetooth.pairing_request().cloned();
    let device = request.as_ref().map(|request| {
        bluetooth
            .device(request.device())
            .map_or_else(|| String::from("A device"), |d| d.name().to_string())
    });
    drop(bluetooth);

    let Some((request, device)) = request.zip(device) else {
        if ASKED.take().is_some() && alert::showing(NAME) {
            alert::close();
        }
        return;
    };
    if ASKED.get() == Some(request.id()) {
        return;
    }
    // the page asks under its list
    if quick_settings::showing(Page::Bluetooth) {
        return;
    }
    ASKED.set(Some(request.id()));
    alert::show(dialog(&request, &device));
}

fn dialog(request: &PairingRequest, device: &str) -> Dialog {
    let id = request.id();
    let path = request.device().to_string();
    let refuse = move || {
        Bluetooth::answer(id, PairingAnswer::Reject);
        Bluetooth::cancel_pairing(&path);
    };
    let accept = move || Bluetooth::answer(id, PairingAnswer::Accept);

    match request.kind() {
        PairingKind::Confirm(passkey) => Dialog::new(NAME, format!("Pair with “{device}”?"))
            .message(format!("Make sure “{device}” shows the code {passkey:06}."))
            .button("Cancel", refuse)
            .main_button("Pair", accept),
        PairingKind::Authorize => Dialog::new(NAME, format!("“{device}” wants to pair"))
            .message("Pair only with a device you know.")
            .button("Cancel", refuse)
            .main_button("Pair", accept),
        PairingKind::AuthorizeService(_) => {
            Dialog::new(NAME, format!("Allow “{device}” to connect?"))
                .message("It isn't paired with this computer.")
                .button("Don't Allow", refuse)
                .main_button("Allow", accept)
        }
        PairingKind::DisplayPin(pin) => shown(device, pin, refuse),
        PairingKind::DisplayPasskey { passkey, .. } => {
            shown(device, &format!("{passkey:06}"), refuse)
        }
        PairingKind::EnterPin | PairingKind::EnterPasskey => {
            Dialog::new(NAME, format!("“{device}” wants to pair"))
                .message(format!("Enter the code for “{device}” to pair with it."))
                .button("Cancel", refuse)
                .main_button("Enter Code…", || quick_settings::open(Page::Bluetooth))
        }
    }
}

/// A code to type on the device, which only it can answer.
fn shown(device: &str, code: &str, refuse: impl Fn() + 'static) -> Dialog {
    Dialog::new(NAME, format!("Pair with “{device}”"))
        .message(format!("Type {code} on “{device}”, then press Return."))
        .button("Cancel", refuse)
}
