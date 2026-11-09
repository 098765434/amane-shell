//! rbw's lines to emaki-pinentry: parsed, unescaped and clipped, never a panic.
#![no_main]

use emaki_pinentry::assuan;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|line: &str| {
    let _ = assuan::parse(line);
    if let Some(mut text) = assuan::unescape(line) {
        // escaping what was unescaped gives text that unescapes to it again
        let escaped = assuan::escape(&text);
        assert_eq!(assuan::unescape(&escaped).as_deref(), Some(text.as_str()));
        assuan::clip(&mut text, 16);
    }
});
