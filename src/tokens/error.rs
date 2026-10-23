//! What a token problem says: the token, its set and the `mode` × `density`
//! selection it was found under. Shared with `build.rs`, which includes this
//! file and `envelope.rs` to read the export the same way the shell does.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct TokenError {
    pub token: String,
    pub set: String,
    /// The `mode` × `density` selection, when one applies.
    pub key: String,
    pub message: String,
}

impl TokenError {
    pub fn file(message: impl Into<String>) -> Self {
        Self::in_set("", "", message)
    }

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
