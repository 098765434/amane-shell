//! The part of the Assuan pinentry protocol rbw speaks: it sets a title, a description, a
//! prompt and an error, then asks for the pin with GETPIN. Everything else is answered OK,
//! or "not implemented" for the dialogs this pinentry has no use for (CONFIRM, MESSAGE).
//!
//! Lines are at most 1000 bytes as the protocol says; their arguments are percent-escaped.

use zeroize::Zeroizing;

/// The longest line Assuan allows, the line break included.
pub const LINE_LIMIT: usize = 1000;

/// The longest text kept from one SET command; the shell clips what it shows further.
pub const TEXT_LIMIT: usize = 900;

pub const GREETING: &str = "OK Pleased to meet you";

// gpg-error codes pinentries answer with: GPG_ERR_CANCELED and GPG_ERR_NOT_IMPLEMENTED,
// from the pinentry source (source 5, pinentry)
pub const CANCELLED: &str = "ERR 83886179 Operation cancelled <Pinentry>";
pub const NOT_IMPLEMENTED: &str = "ERR 83886149 Not implemented <Pinentry>";
pub const TOO_LONG: &str = "ERR 83886157 Line too long <Pinentry>";
pub const BAD_ESCAPE: &str = "ERR 83886155 Invalid value <Pinentry>";

/// What the prompt says, from the SET commands so far.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub title: String,
    pub description: String,
    pub prompt: String,
    pub error: String,
}

/// One command, read off a line.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Set(Field, String),
    GetPin,
    GetInfo(String),
    Reset,
    Bye,
    // a dialog this pinentry doesn't show
    Unsupported,
    // a setting there's nothing to do for
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Description,
    Prompt,
    Error,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    TooLong,
    BadEscape,
}

/// Reads one line, without its line break.
pub fn parse(line: &str) -> Result<Command, ParseError> {
    if line.len() >= LINE_LIMIT {
        return Err(ParseError::TooLong);
    }

    let (name, argument) = match line.split_once(' ') {
        Some((name, argument)) => (name, argument),
        None => (line, ""),
    };

    // commands are case-insensitive
    let name = name.to_ascii_uppercase();

    let set = |field| -> Result<Command, ParseError> {
        let mut text = unescape(argument).ok_or(ParseError::BadEscape)?;

        clip(&mut text, TEXT_LIMIT);

        Ok(Command::Set(field, text))
    };

    match name.as_str() {
        "SETTITLE" => set(Field::Title),
        "SETDESC" => set(Field::Description),
        "SETPROMPT" => set(Field::Prompt),
        "SETERROR" => set(Field::Error),
        "GETPIN" => Ok(Command::GetPin),
        "GETINFO" => Ok(Command::GetInfo(argument.trim().to_ascii_lowercase())),
        "RESET" => Ok(Command::Reset),
        "BYE" => Ok(Command::Bye),
        "CONFIRM" | "MESSAGE" => Ok(Command::Unsupported),
        _ => Ok(Command::Ignored),
    }
}

/// Undoes Assuan's percent escapes; none for a bad escape or text that isn't UTF-8.
pub fn unescape(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());

    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let high = hex(*bytes.get(at + 1)?)?;
            let low = hex(*bytes.get(at + 2)?)?;

            out.push(high << 4 | low);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }

    String::from_utf8(out).ok()
}

/// Escapes data for a D line: percent, line breaks and carriage returns, as the protocol asks.
pub fn escape(text: &str) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(text.len() * 3));

    for letter in text.chars() {
        match letter {
            '%' => out.push_str("%25"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            letter => out.push(letter),
        }
    }

    out
}

fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

/// Cuts text to at most `limit` bytes, at a whole letter.
pub fn clip(text: &mut String, limit: usize) {
    if text.len() <= limit {
        return;
    }

    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }

    text.truncate(end);
}

impl Prompt {
    pub fn set(&mut self, field: Field, text: String) {
        match field {
            Field::Title => self.title = text,
            Field::Description => self.description = text,
            Field::Prompt => self.prompt = text,
            Field::Error => self.error = text,
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn reads_what_rbw_sends() {
        assert_eq!(
            parse("SETTITLE rbw"),
            Ok(Command::Set(Field::Title, String::from("rbw")))
        );
        assert_eq!(
            parse("SETDESC Please enter the master password for%0Aocbk@ocbk.no"),
            Ok(Command::Set(
                Field::Description,
                String::from("Please enter the master password for\nocbk@ocbk.no")
            ))
        );
        assert_eq!(
            parse("SETPROMPT Master Password"),
            Ok(Command::Set(Field::Prompt, String::from("Master Password")))
        );
        assert_eq!(parse("getpin"), Ok(Command::GetPin));
        assert_eq!(parse("BYE"), Ok(Command::Bye));
        assert_eq!(parse("OPTION ttyname=/dev/pts/1"), Ok(Command::Ignored));
        assert_eq!(parse("GETINFO pid"), Ok(Command::GetInfo(String::from("pid"))));
        assert_eq!(parse("CONFIRM"), Ok(Command::Unsupported));
    }

    #[test]
    fn refuses_bad_lines() {
        assert_eq!(parse("SETDESC 100%"), Err(ParseError::BadEscape));
        assert_eq!(parse("SETDESC %zz"), Err(ParseError::BadEscape));
        assert_eq!(parse("SETDESC %ff%fe"), Err(ParseError::BadEscape));
        assert_eq!(parse(&"A".repeat(LINE_LIMIT)), Err(ParseError::TooLong));
    }

    #[test]
    fn escapes_data() {
        assert_eq!(escape("a%b\nc\rd").as_str(), "a%25b%0Ac%0Dd");
        assert_eq!(unescape(&escape("pä%ss\nword")).as_deref(), Some("pä%ss\nword"));
    }

    #[test]
    fn clips_at_whole_letters() {
        let mut text = String::from("ééé");
        clip(&mut text, 3);
        assert_eq!(text, "é");
    }

    proptest! {
        #[test]
        fn any_line_parses_or_is_refused_without_panicking(line in ".{0,1100}") {
            let _ = parse(&line);
        }

        #[test]
        fn escape_round_trips(text in "\\PC{0,200}") {
            prop_assert_eq!(unescape(&escape(&text)), Some(text));
        }

        #[test]
        fn set_text_is_clipped(text in "[a-zé ]{0,2000}") {
            if let Ok(Command::Set(_, kept)) = parse(&format!("SETDESC {text}")) {
                prop_assert!(kept.len() <= TEXT_LIMIT);
            }
        }
    }
}
