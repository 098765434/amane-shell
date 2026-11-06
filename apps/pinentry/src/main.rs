//! amane-pinentry: rbw's `pinentry`. It asks for the master password in the shell's own
//! dialog, or runs pinentry-gnome3 in its place when the shell isn't there to ask.
//!
//! rbw config set pinentry /run/current-system/sw/bin/amane-pinentry

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};
use std::time::Duration;

use amane_pinentry::assuan::{self, Command as Request, ParseError, Prompt};
use amane_pinentry::peer::{self, Peer};
use amane_pinentry::wire::{self, Answer};
use zeroize::Zeroizing;

// set by the nix build to the store's pinentry-gnome3, so no PATH can swap it
const FALLBACK: &str = match option_env!("AMANE_PINENTRY_FALLBACK") {
    Some(path) => path,
    None => "/run/current-system/sw/bin/pinentry-gnome3",
};

// how long the shell may take to answer: the user typing a password
const ANSWER_TIME: Duration = Duration::from_secs(180);

fn main() -> ExitCode {
    let Some(shell) = shell() else {
        return fallback();
    };

    match serve(shell) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

// the shell's socket, if the shell is the one at the other end and agrees to ask
fn shell() -> Option<UnixStream> {
    let mut stream = UnixStream::connect(peer::socket_path()?).ok()?;

    let peer = Peer::of(&stream).ok()?;

    if peer.uid != peer::current_uid() {
        return None;
    }

    let exe = peer::exe(peer.pid)?;

    if !peer::is_program(&exe, &["amane-shell"]) || !peer.still_there() {
        return None;
    }

    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;

    wire::write_hello(&mut stream).ok()?;

    if !wire::read_welcome(&mut stream).ok()? {
        return None;
    }

    stream.set_read_timeout(Some(ANSWER_TIME)).ok()?;

    Some(stream)
}

// pinentry-gnome3 in this process's place, with the same arguments, stdin and stdout
fn fallback() -> ExitCode {
    let error = Command::new(FALLBACK)
        .args(std::env::args_os().skip(1))
        .exec();

    eprintln!("amane-pinentry: no shell to ask, and {FALLBACK} didn't run: {error}");

    ExitCode::FAILURE
}

fn serve(mut shell: UnixStream) -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin.lock());
    // straight to fd 1, with no buffer of std's to keep the pin after it's written
    // SAFETY: fd 1 is this process's stdout, open for its whole life; ManuallyDrop keeps it open
    let mut out = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(1) });

    out.write_all(format!("{}\n", assuan::GREETING).as_bytes())?;

    let mut prompt = Prompt::default();

    loop {
        let Some(line) = read_line(&mut lines)? else {
            return Ok(());
        };

        let reply = match assuan::parse(&line) {
            Ok(Request::Set(field, text)) => {
                prompt.set(field, text);

                Zeroizing::new(String::from("OK"))
            }

            Ok(Request::GetPin) => ask(&mut shell, &prompt),

            Ok(Request::GetInfo(what)) => Zeroizing::new(match what.as_str() {
                "pid" => format!("D {}\nOK", std::process::id()),
                "flavor" => String::from("D amane\nOK"),
                "version" => format!("D {}\nOK", env!("CARGO_PKG_VERSION")),
                _ => String::from("OK"),
            }),

            Ok(Request::Reset) => {
                prompt = Prompt::default();

                Zeroizing::new(String::from("OK"))
            }

            Ok(Request::Bye) => {
                out.write_all(b"OK closing connection\n")?;

                return Ok(());
            }

            Ok(Request::Unsupported) => Zeroizing::new(String::from(assuan::NOT_IMPLEMENTED)),
            Ok(Request::Ignored) => Zeroizing::new(String::from("OK")),

            Err(ParseError::TooLong) => Zeroizing::new(String::from(assuan::TOO_LONG)),
            Err(ParseError::BadEscape) => Zeroizing::new(String::from(assuan::BAD_ESCAPE)),
        };

        let mut line = Zeroizing::new(String::with_capacity(reply.len() + 1));
        line.push_str(&reply);
        line.push('\n');

        out.write_all(line.as_bytes())?;
    }
}

// the pin from the shell's dialog, as the lines Assuan answers GETPIN with
fn ask(shell: &mut UnixStream, prompt: &Prompt) -> Zeroizing<String> {
    let answer = wire::write_request(shell, prompt).and_then(|()| wire::read_answer(shell));

    match answer {
        Ok(Answer::Pin(pin)) => {
            let escaped = assuan::escape(&pin);

            let mut reply = Zeroizing::new(String::with_capacity(escaped.len() + 6));
            reply.push_str("D ");
            reply.push_str(&escaped);
            reply.push_str("\nOK");

            reply
        }

        Ok(Answer::Cancelled | Answer::Refused) | Err(_) => {
            Zeroizing::new(String::from(assuan::CANCELLED))
        }
    }
}

// one line from rbw, without its line break; a line too long is kept too long, for the error
fn read_line(from: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();

    let read = from
        .by_ref()
        .take(assuan::LINE_LIMIT as u64 + 1)
        .read_until(b'\n', &mut bytes)?;

    if read == 0 {
        return Ok(None);
    }

    // a line that went past the limit is read to its end and refused
    if !bytes.ends_with(b"\n") && bytes.len() > assuan::LINE_LIMIT {
        let mut rest = Vec::new();
        from.read_until(b'\n', &mut rest)?;

        return Ok(Some("X".repeat(assuan::LINE_LIMIT)));
    }

    while bytes
        .last()
        .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
    {
        bytes.pop();
    }

    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}
