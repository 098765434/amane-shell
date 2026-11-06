//! The shell's end of the daemon's socket: a connection split into a side
//! that sends and a side that reads, each for its own thread.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::protocol::{Envelope, Message, Request};

/// Sends requests.
pub struct Sender {
    stream: UnixStream,
    next: u64,
}

/// Reads what the daemon sends.
pub struct Receiver {
    reader: BufReader<UnixStream>,
    line: String,
}

/// Connects to the daemon at `path`.
pub fn connect(path: &Path) -> io::Result<(Sender, Receiver)> {
    let stream = UnixStream::connect(path)?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let reader = BufReader::new(stream.try_clone()?);
    Ok((
        Sender { stream, next: 1 },
        Receiver {
            reader,
            line: String::new(),
        },
    ))
}

impl Sender {
    /// Sends `request`, giving the id its answer will carry.
    pub fn send(&mut self, request: Request) -> io::Result<u64> {
        let id = self.next;
        self.next += 1;
        let mut line = serde_json::to_string(&Envelope { id, request }).map_err(io::Error::other)?;
        line.push('\n');
        self.stream.write_all(line.as_bytes())?;
        Ok(id)
    }

    pub fn shutdown(&self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

impl Receiver {
    /// The next message; an error once the daemon's gone. A line that
    /// doesn't read is skipped.
    pub fn recv(&mut self) -> io::Result<Message> {
        loop {
            self.line.clear();
            if self.reader.read_line(&mut self.line)? == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            if let Ok(message) = serde_json::from_str(self.line.trim_end()) {
                return Ok(message);
            }
        }
    }
}
