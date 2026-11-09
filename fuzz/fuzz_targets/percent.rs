//! %-escapes from links (a dropped file's, a notification's): decoded without a panic, and what
//! was encoded decodes back.
#![no_main]

use emaki_base::percent;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = percent::decode(text);
    let _ = percent::decode_strict(text);
    assert_eq!(percent::decode(&percent::encode(text)), text);
});
