//! How the shell and the daemon talk: JSON, a line each, over the daemon's
//! Unix socket ([`crate::socket_path`]). The shell sends [`Request`]s, each
//! with an id; the daemon answers each with a [`Message`] carrying it back,
//! and while it listens sends what it hears as it hears it.
//!
//! ```text
//! → {"id":1,"op":"hello"}
//! ← {"kind":"hello","id":1,"engine":"cactus 4.0.0","models":3}
//! → {"id":2,"op":"complete","system":"date: …","tools":"[…]","text":"make it warmer"}
//! ← {"kind":"answer","id":2,"answer":{"function_calls":[…],"confidence":0.8,…},"ms":64}
//! → {"id":3,"op":"listen_start","language":"en","keywords":["Night Light","Firefox"]}
//! ← {"kind":"level","rms":0.12}
//! ← {"kind":"partial","text":"turn on night"}
//! → {"id":4,"op":"listen_stop"}
//! ← {"kind":"heard","id":4,"text":"Turn on Night Light.","ms":120}
//! ← {"kind":"error","id":2,"error":"…"}
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub id: u64,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// What the daemon runs, and has loaded.
    Hello,
    /// Needle's answer for `text`, offered `tools` (a JSON array) with
    /// `system`'s facts.
    Complete {
        system: String,
        tools: String,
        text: String,
    },
    /// Starts listening to the microphone, Whistle told to favor `keywords`.
    ListenStart {
        #[serde(default)]
        language: Option<String>,
        #[serde(default)]
        keywords: Vec<String>,
    },
    /// Stops listening, and answers with all that was heard.
    ListenStop,
    /// Stops listening, and forgets it.
    ListenCancel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    Hello {
        id: u64,
        engine: String,
        /// Cactus's loaded kinds: 1 text, 2 speech, 4 vision.
        models: i32,
    },
    /// Needle's answer, as Cactus gives it.
    Answer { id: u64, answer: Value, ms: u64 },
    /// How loud the microphone is now, 0 … 1, a few times a second.
    Level { rms: f32 },
    /// What's been heard so far, while listening.
    Partial { text: String },
    /// All that was heard, once listening stopped.
    Heard { id: u64, text: String, ms: u64 },
    Error {
        #[serde(default)]
        id: Option<u64>,
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_a_line_of_json() {
        let envelope = Envelope {
            id: 2,
            request: Request::Complete {
                system: "s".into(),
                tools: "[]".into(),
                text: "louder".into(),
            },
        };
        let line = serde_json::to_string(&envelope).unwrap();
        assert_eq!(
            line,
            r#"{"id":2,"op":"complete","system":"s","tools":"[]","text":"louder"}"#
        );
        assert_eq!(serde_json::from_str::<Envelope>(&line).unwrap(), envelope);
        let hello: Envelope = serde_json::from_str(r#"{"id":1,"op":"hello"}"#).unwrap();
        assert_eq!(hello.request, Request::Hello);
    }

    #[test]
    fn messages_are_tagged_by_kind() {
        let message = Message::Heard {
            id: 4,
            text: "louder".into(),
            ms: 80,
        };
        let line = serde_json::to_string(&message).unwrap();
        assert_eq!(line, r#"{"kind":"heard","id":4,"text":"louder","ms":80}"#);
        assert_eq!(serde_json::from_str::<Message>(&line).unwrap(), message);
    }
}
