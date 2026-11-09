//! The shell's and emaki-pinentry's socket, read from either end: whatever arrives, a reader
//! gives an answer or an error, never a panic, and never a pin longer than the limit.
#![no_main]

use emaki_pinentry::wire::{self, Answer, PIN_LIMIT};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = wire::read_hello(&mut &bytes[..]);
    let _ = wire::read_welcome(&mut &bytes[..]);
    let _ = wire::read_request(&mut &bytes[..]);
    if let Ok(Answer::Pin(pin)) = wire::read_answer(&mut &bytes[..]) {
        assert!(pin.len() <= PIN_LIMIT);
    }
});
