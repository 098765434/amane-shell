//! A phone's screen: whether it can be shown (adb has the phone, or it was
//! set up before), and opening it in `amane-mirror`. A phone set up once is
//! remembered in the file Messages reads too (`amane_phone::shared::screen`).

use amane::Service;
use amane_phone::shared::screen as shared;
use amane_phone::{Adb, PhoneDevice};

use super::MIRROR_APP;

/// Remembers that `device`'s screen was set up, for the Phone page and
/// Messages to offer it again.
pub fn remember(device: &str) {
    if let Err(error) = shared::remember(device) {
        log::warn!("couldn't remember the phone's screen: {error}");
    }
}

pub use shared::set_up;

/// Whether adb has `device` now, at one of the addresses KDE Connect knows.
pub fn ready(device: &PhoneDevice) -> bool {
    serial(device).is_some()
}

/// Opens the phone's screen.
pub fn open_screen(device: &str) {
    amane::Launch::argv(&[MIRROR_APP, "--device", device]).activated();
}

/// The serial adb has `device` by, at one of the addresses KDE Connect knows.
pub fn serial(device: &PhoneDevice) -> Option<String> {
    let adb = Adb::read();
    device
        .addresses()
        .iter()
        .find_map(|address| adb.ready_at(*address))
        .map(|found| found.serial.clone())
}
