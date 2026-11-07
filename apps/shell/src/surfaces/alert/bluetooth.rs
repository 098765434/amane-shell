//! A device asking to pair while Quick Settings' Bluetooth page isn't
//! showing — a phone pairing from its side, a keyboard waking up — asks
//! through the shell's alert: confirm the code, allow it, or see the code
//! to type on it. A code to type here opens the page, which has the field.
//! The page itself asks under its list while it shows.
//!
//! A subscription to `Bluetooth` keeps the latest request (and its device's
//! name), and wakes the alert surface's view only when it comes or goes; the
//! view syncs from that.

use amane_kit::sync::Locked as _;
use std::cell::Cell;
use std::sync::{Mutex, Once};

use amane::{Bluetooth, PairingAnswer, PairingKind, PairingRequest, Service};

use crate::surfaces::alert;
use crate::surfaces::quick_settings::{self, Page};
use amane_kit::dialog::Dialog;

/// The alert's name, so only this alert is closed when BlueZ stops asking.
const NAME: &str = "amane.alert.bluetooth.pair";

thread_local! {
    // the request already asked about, by id
    static ASKED: Cell<Option<u64>> = const { Cell::new(None) };
}

// what BlueZ asks now, and the device's name
static LATEST: Mutex<Option<(PairingRequest, String)>> = Mutex::new(None);

fn asking(bluetooth: &Bluetooth) -> Option<(PairingRequest, String)> {
    let request = bluetooth.pairing_request()?.clone();
    let device = bluetooth
        .device(request.device())
        .map_or_else(|| String::from("A device"), |d| d.name().to_string());
    Some((request, device))
}

/// Shows the alert for a new request, or puts it away once BlueZ stopped
/// asking.
pub fn sync() {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        Bluetooth::subscribe(|bluetooth| {
            let fresh = asking(bluetooth);
            let mut latest = LATEST.locked();
            if *latest != fresh {
                *latest = fresh;
                alert::Requests::changed();
            }
        })
        .forever();
        *LATEST.locked() = asking(&Bluetooth::read());
    });
    alert::Requests::watch();
    let latest = LATEST.locked().clone();

    let Some((request, device)) = latest else {
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
                .main_button("Enter Code…", || {
                    quick_settings::open_alone(Page::Bluetooth)
                })
        }
    }
}

/// A code to type on the device, which only it can answer.
fn shown(device: &str, code: &str, refuse: impl Fn() + 'static) -> Dialog {
    Dialog::new(NAME, format!("Pair with “{device}”"))
        .message(format!("Type {code} on “{device}”, then press Return."))
        .button("Cancel", refuse)
}
