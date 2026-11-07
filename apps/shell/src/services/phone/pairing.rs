//! A phone asking to pair through KDE Connect. A subscription keeps the
//! latest request, so what shows it (the shell's alert, or Quick Settings'
//! Phone page) draws again only when one comes or goes.

use std::sync::{Mutex, Once, PoisonError};

use amane::Service;
use amane_phone::KdeConnect;

amane_kit::marker! {
    /// A phone started or stopped asking to pair.
    pub Asked
}

/// A phone asking now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
    pub id: String,
    pub name: String,
    /// The code both screens show; empty when the phone sent none.
    pub key: String,
}

static LATEST: Mutex<Option<Asking>> = Mutex::new(None);

fn of(kdeconnect: &KdeConnect) -> Option<Asking> {
    kdeconnect.pairing_request().map(|device| Asking {
        id: device.id().to_string(),
        name: device.name().to_string(),
        key: device.verification_key().to_string(),
    })
}

/// The phone asking to pair now, if one is. Called while drawing: what asked
/// draws again when that changes.
pub fn asking() -> Option<Asking> {
    static WATCH: Once = Once::new();
    WATCH.call_once(|| {
        KdeConnect::subscribe(|kdeconnect| {
            let fresh = of(kdeconnect);
            let mut latest = LATEST.lock().unwrap_or_else(PoisonError::into_inner);
            if *latest != fresh {
                *latest = fresh;
                Asked::changed();
            }
        })
        .forever();
        *LATEST.lock().unwrap_or_else(PoisonError::into_inner) = of(&KdeConnect::read());
    });
    Asked::watch();
    LATEST
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}
