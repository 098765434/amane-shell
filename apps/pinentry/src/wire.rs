//! What amane-pinentry and the shell say over their socket: a request with what the prompt
//! says, and an answer with the pin, cancelled, or refused. Lengths are checked against fixed
//! limits before anything is read, and the pin's buffers wipe themselves.

use std::io::{self, Read, Write};

use zeroize::Zeroizing;

use crate::assuan::Prompt;

const MAGIC: &[u8; 6] = b"AMPE1\n";

/// The longest field of a request, in bytes.
pub const FIELD_LIMIT: usize = 1024;

/// The longest pin, in bytes.
pub const PIN_LIMIT: usize = 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Pin(Zeroizing<String>),
    Cancelled,
    // the shell wouldn't ask: the caller failed its checks, or nothing of the shell's asked
    Refused,
}

const HELLO: &[u8; 6] = b"AMPH1\n";

/// The pinentry's first words, so the shell checks who it is before rbw is answered at all.
pub fn write_hello(to: &mut impl Write) -> io::Result<()> {
    to.write_all(HELLO)?;
    to.flush()
}

pub fn read_hello(from: &mut impl Read) -> io::Result<()> {
    let mut hello = [0; 6];
    from.read_exact(&mut hello)?;

    if &hello != HELLO {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a pinentry"));
    }

    Ok(())
}

/// The shell's answer to the hello: whether it will ask.
pub fn write_welcome(to: &mut impl Write, welcome: bool) -> io::Result<()> {
    to.write_all(&[u8::from(welcome)])?;
    to.flush()
}

pub fn read_welcome(from: &mut impl Read) -> io::Result<bool> {
    let mut welcome = [0; 1];
    from.read_exact(&mut welcome)?;

    Ok(welcome[0] == 1)
}

pub fn write_request(to: &mut impl Write, prompt: &Prompt) -> io::Result<()> {
    to.write_all(MAGIC)?;

    for field in [
        &prompt.title,
        &prompt.description,
        &prompt.prompt,
        &prompt.error,
    ] {
        write_field(to, field.as_bytes(), FIELD_LIMIT)?;
    }

    to.flush()
}

pub fn read_request(from: &mut impl Read) -> io::Result<Prompt> {
    let mut magic = [0; 6];
    from.read_exact(&mut magic)?;

    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a pinentry request"));
    }

    let mut text = || -> io::Result<String> {
        let bytes = read_field(from, FIELD_LIMIT)?;

        String::from_utf8(bytes.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8"))
    };

    Ok(Prompt {
        title: text()?,
        description: text()?,
        prompt: text()?,
        error: text()?,
    })
}

pub fn write_answer(to: &mut impl Write, answer: &Answer) -> io::Result<()> {
    match answer {
        Answer::Pin(pin) => {
            to.write_all(&[1])?;
            write_field(to, pin.as_bytes(), PIN_LIMIT)?;
        }
        Answer::Cancelled => to.write_all(&[0])?,
        Answer::Refused => to.write_all(&[2])?,
    }

    to.flush()
}

pub fn read_answer(from: &mut impl Read) -> io::Result<Answer> {
    let mut status = [0; 1];
    from.read_exact(&mut status)?;

    match status[0] {
        0 => Ok(Answer::Cancelled),
        2 => Ok(Answer::Refused),
        1 => {
            let bytes = read_field(from, PIN_LIMIT)?;

            let text = std::str::from_utf8(&bytes)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not UTF-8"))?;

            Ok(Answer::Pin(Zeroizing::new(String::from(text))))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "unknown answer")),
    }
}

fn write_field(to: &mut impl Write, bytes: &[u8], limit: usize) -> io::Result<()> {
    if bytes.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "field too long"));
    }

    to.write_all(&(bytes.len() as u16).to_le_bytes())?;
    to.write_all(bytes)
}

fn read_field(from: &mut impl Read, limit: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut length = [0; 2];
    from.read_exact(&mut length)?;

    let length = usize::from(u16::from_le_bytes(length));

    if length > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "field too long"));
    }

    let mut bytes = Zeroizing::new(vec![0; length]);
    from.read_exact(&mut bytes)?;

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn requests_and_answers_round_trip() {
        let prompt = Prompt {
            title: String::from("rbw"),
            description: String::from("Please enter the master password"),
            prompt: String::from("Master Password"),
            error: String::new(),
        };

        let mut wire = Vec::new();
        write_request(&mut wire, &prompt).unwrap();
        assert_eq!(read_request(&mut wire.as_slice()).unwrap(), prompt);

        for answer in [
            Answer::Pin(Zeroizing::new(String::from("hunter2 ä"))),
            Answer::Cancelled,
            Answer::Refused,
        ] {
            let mut wire = Vec::new();
            write_answer(&mut wire, &answer).unwrap();
            assert_eq!(read_answer(&mut wire.as_slice()).unwrap(), answer);
        }
    }

    #[test]
    fn hello_and_welcome() {
        let mut wire = Vec::new();
        write_hello(&mut wire).unwrap();
        assert!(read_hello(&mut wire.as_slice()).is_ok());
        assert!(read_hello(&mut &b"AMPE1\n"[..]).is_err());

        let mut wire = Vec::new();
        write_welcome(&mut wire, true).unwrap();
        assert!(read_welcome(&mut wire.as_slice()).unwrap());
    }

    #[test]
    fn refuses_oversized_and_foreign_input() {
        let mut wire = MAGIC.to_vec();
        wire.extend_from_slice(&u16::MAX.to_le_bytes());
        assert!(read_request(&mut wire.as_slice()).is_err());

        assert!(read_request(&mut &b"GET / HTTP/1.1\r\n"[..]).is_err());

        let mut wire = vec![1];
        wire.extend_from_slice(&((PIN_LIMIT + 1) as u16).to_le_bytes());
        assert!(read_answer(&mut wire.as_slice()).is_err());

        assert!(read_answer(&mut &[7u8][..]).is_err());
    }

    proptest! {
        #[test]
        fn any_bytes_are_read_or_refused_without_panicking(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
            let _ = read_request(&mut bytes.as_slice());
            let _ = read_answer(&mut bytes.as_slice());
        }
    }
}
