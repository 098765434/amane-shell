//! Keeps the `platform::micmute` LED in step with PipeWire's default
//! source. The F4 hardware key already flips both at once through the
//! kernel driver, but a mute flipped from the Sound tile, the Privacy
//! page or an OSD key change only touches PipeWire — the LED would lie.
//! This service subscribes to [`amane::Audio`] and writes the LED
//! through logind each time mute changes.

use std::path::Path;

use amane::{Audio, Service};

use super::logind;

const LED: &str = "platform::micmute";

/// Starts the sync; a no-op on machines without the LED.
pub fn start() {
    if !has_led() {
        return;
    }

    // Reconcile once at startup: trust the PipeWire state and push to the LED.
    write_led(Audio::peek().microphone_muted());

    Audio::subscribe(|audio| write_led(audio.microphone_muted())).forever();
}

fn has_led() -> bool {
    Path::new("/sys/class/leds")
        .join(LED)
        .join("brightness")
        .exists()
}

fn write_led(muted: bool) {
    logind::set_brightness("leds", LED, u32::from(muted));
}
