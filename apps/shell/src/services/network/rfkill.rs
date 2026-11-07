//! Airplane mode — every radio (Wi-Fi, Bluetooth, mobile, NFC) blocked.
//!
//! amane's `Rfkill` follows the kernel's switches from `/dev/rfkill`, so nothing polls; this
//! adds the one thing the shell does with them: turning every radio off or on at once, as
//! `rfkill block all` does. It's on when every radio is blocked, by software or by a hardware
//! switch.

pub use amane::Rfkill;

/// Blocks every radio, or lifts every software block.
pub fn set_airplane(on: bool) {
    if let Err(err) = Rfkill::set_blocked(None, on) {
        eprintln!("amane-shell: couldn't switch airplane mode: {err}");
    }
}
