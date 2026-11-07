//! `%20` escapes, as URLs and Assuan write them.

/// `text` with every byte but RFC 3986's unreserved characters (letters,
/// digits, `-._~`) escaped.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `text` with its escapes undone; a `%` that isn't an escape stays, and
/// bytes that aren't UTF-8 become `�`.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match escape(bytes, at) {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `text` with its escapes undone; nothing for a bad escape or bytes that
/// aren't UTF-8.
pub fn decode_strict(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            out.push(escape(bytes, at)?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

// the byte an escape at `at` stands for
fn escape(bytes: &[u8], at: usize) -> Option<u8> {
    let hex = |byte: u8| char::from(byte).to_digit(16);
    match bytes.get(at..at + 3)? {
        [b'%', high, low] => Some((hex(*high)? << 4 | hex(*low)?) as u8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_round_trip() {
        assert_eq!(encode("a b/ø~"), "a%20b%2F%C3%B8~");
        assert_eq!(decode("a%20b%2F%C3%B8~"), "a b/ø~");
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz"), "%zz");
        assert_eq!(decode_strict("a%25b").as_deref(), Some("a%b"));
        assert_eq!(decode_strict("100%"), None);
        assert_eq!(decode_strict("%FF"), None);
    }
}
