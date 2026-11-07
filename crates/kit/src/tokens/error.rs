//! What a token problem says: the token, its set and the `mode` × `density`
//! selection it was found under. Shared with `build.rs`, which includes this
//! file and `envelope.rs` to read the export the same way the shell does.

use std::fmt;

/// A problem with a token, its set or the export, and where it was found.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenError {
    /// The token's dotted name; empty for a problem with a set or the file.
    pub token: String,
    /// The set's name; empty for a problem with the whole file.
    pub set: String,
    /// The `mode` × `density` selection, when one applies.
    pub key: String,
    /// What's wrong, in words.
    pub message: String,
}

impl TokenError {
    /// A problem with the whole export.
    pub fn file(message: impl Into<String>) -> Self {
        Self::in_set("", "", message)
    }

    /// A problem with `token` in `set`; an empty `token` is the set itself.
    pub fn in_set(set: &str, token: &str, message: impl Into<String>) -> Self {
        Self {
            token: token.to_string(),
            set: set.to_string(),
            key: String::new(),
            message: message.into(),
        }
    }
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place: Vec<String> = [
            (!self.set.is_empty()).then(|| format!("set {}", self.set)),
            (!self.key.is_empty()).then(|| self.key.clone()),
        ]
        .into_iter()
        .flatten()
        .collect();

        match (self.token.is_empty(), place.is_empty()) {
            (true, true) => write!(f, "{}", self.message),
            (true, false) => write!(f, "({}): {}", place.join(", "), self.message),
            (false, true) => write!(f, "{}: {}", self.token, self.message),
            (false, false) => write!(f, "{} ({}): {}", self.token, place.join(", "), self.message),
        }
    }
}
