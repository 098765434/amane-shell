//! A password, username or code on its way from rbw into a field: wiped when dropped, and never
//! printed. It lives for the moment between rbw and the fill; the copies past it (the Wayland
//! message, the compositor's, the app's) are outside the shell's reach.

use std::fmt;

use zeroize::Zeroizing;

/// The most a secret read from rbw can hold.
pub const LIMIT: usize = 4096;

pub struct Secret {
    text: Zeroizing<String>,
}

impl Secret {
    /// Takes what rbw printed, without the line break after it; none for text that isn't UTF-8.
    pub fn from_output(bytes: &[u8]) -> Option<Self> {
        let mut end = bytes.len();
        while end > 0 && matches!(bytes[end - 1], b'\n' | b'\r') {
            end -= 1;
        }

        let text = std::str::from_utf8(&bytes[..end]).ok()?;

        // sized once, so it never grows and leaves a copy behind
        let mut kept = Zeroizing::new(String::with_capacity(text.len().max(1)));
        kept.push_str(text);

        Some(Self { text: kept })
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    #[cfg(test)]
    pub fn expose(&self) -> &str {
        &self.text
    }

    /// The text, moved out as a buffer that wipes itself, for FocusedField::fill.
    pub fn into_text(self) -> Zeroizing<String> {
        self.text
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_loses_its_line_break_and_never_prints() {
        let secret = Secret::from_output(b"hunter2\n").unwrap();

        assert_eq!(secret.expose(), "hunter2");
        assert_eq!(format!("{secret:?}"), "Secret([redacted])");
        assert_eq!(secret.into_text().as_str(), "hunter2");

        assert!(Secret::from_output(b"\xff\xfe").is_none());
        assert!(Secret::from_output(b"\n").unwrap().is_empty());
    }
}
