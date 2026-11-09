//! A name quoted for a Nix attribute path: whatever it is, the quoted form is one plain name or
//! one string, which nothing in it can end or splice code into.
#![no_main]

use emaki_base::nix::attr_part;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|name: &str| {
    let quoted = attr_part(name);
    let Some(inner) = quoted.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) else {
        // a plain name: only the letters a Nix name may have
        assert!(quoted.chars().all(|c| c.is_ascii_alphanumeric() || "_-'".contains(c)));
        return;
    };
    // inside the string, a backslash always escapes the next letter, a quote or ${ only ever
    // comes escaped
    let mut letters = inner.chars().peekable();
    while let Some(letter) = letters.next() {
        match letter {
            '\\' => {
                assert!(letters.next().is_some(), "a lone backslash at the end");
            }
            '"' => panic!("an unescaped quote in {quoted:?}"),
            '$' => assert_ne!(letters.peek(), Some(&'{'), "an unescaped ${{ in {quoted:?}"),
            _ => {}
        }
    }
});
