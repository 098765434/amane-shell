//! Text, made comparable: what was typed or transcribed, and what things are
//! called, the same way.

/// `text` lowercased, hyphens joined ("Wi‑Fi" is "wifi"), other
/// punctuation as spaces, spaces single. Digits, `%` and `:` are kept.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '-' | '‐' | '‑' | '‒' | '–' | '\'' | '’') {
            continue;
        }
        if c.is_alphanumeric() || c == '%' || c == ':' {
            out.extend(c.to_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_compared_plainly() {
        assert_eq!(normalize("Wi‑Fi"), "wifi");
        assert_eq!(normalize("  Turn on Night Light, please! "), "turn on night light please");
        assert_eq!(normalize("Volume to 40%."), "volume to 40%");
        assert_eq!(normalize("don't"), "dont");
        assert_eq!(normalize("1:30"), "1:30");
    }
}
