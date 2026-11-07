//! The shell's end of amane-pinentry: a socket only this user can reach, and a dialog only for
//! the rbw the shell itself asked to unlock.
//!
//! A caller is answered only if all of these hold, or it's told no and runs pinentry-gnome3:
//!
//! - it's this user (SO_PEERCRED), and runs `amane-pinentry` from the Nix store;
//! - its parent runs `rbw-agent` from the store, with the shell's own HOME, XDG_CONFIG_HOME,
//!   XDG_CACHE_HOME, XDG_RUNTIME_DIR and RBW_PROFILE, so its server and account are the ones
//!   the shell shows;
//! - an rbw command the shell started that can ask (unlock, login, register, sync) is still
//!   running, for at most two minutes;
//! - its pidfd still points at a live process once all that was read.
//!
//! So no other program can put up the shell's trusted dialog to collect the master password.

use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use amane::Service;
use amane_pinentry::assuan::{self, Prompt};
use amane_pinentry::peer::{self, Peer};
use amane_pinentry::wire::{self, Answer};
use zeroize::Zeroizing;

// how long after the shell runs an rbw command that can ask the dialog may come up
const ASKING_TIME: Duration = Duration::from_secs(120);

// how long one question waits for the user
const ANSWER_TIME: Duration = Duration::from_secs(180);

// how long the description and error may be, as shown
const SHOWN_LIMIT: usize = 400;

// until when the shell expects rbw to ask, set as it runs unlock or login
static EXPECTED: Mutex<Option<Instant>> = Mutex::new(None);

// one question at a time: the one showing, and where its answer goes
static ASKING: Mutex<Option<(u64, Sender<Answer>)>> = Mutex::new(None);

// one caller at a time is let in; the next waits its turn
static TURN: Mutex<()> = Mutex::new(());

/// rbw asking the user, as the unlock dialog shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinQuestion {
    pub id: u64,
    pub description: String,
    pub prompt: String,
    pub error: String,
}

/// The question showing, if any; the dialog reads it.
#[derive(Default)]
pub struct PinEntry {
    question: Option<PinQuestion>,
    next_id: u64,
}

impl Service for PinEntry {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

impl PinEntry {
    pub fn question(&self) -> Option<&PinQuestion> {
        self.question.as_ref()
    }
}

/// The shell is about to run an rbw command that may ask for the master password.
pub fn expect() {
    *EXPECTED.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now() + ASKING_TIME);
}

/// The rbw command the shell ran is over; a later caller is told no.
pub fn done_expecting() {
    *EXPECTED.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

fn expected() -> bool {
    EXPECTED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|until| Instant::now() < until)
}

/// The dialog's answer: the pin, which is wiped once sent, or none for Cancel.
pub fn answer(id: u64, pin: Option<Zeroizing<String>>) {
    let asking = ASKING.lock().unwrap_or_else(PoisonError::into_inner).take();

    match asking {
        Some((asked, reply)) if asked == id => {
            // a caller that hung up no longer wants the answer
            let _ = reply.send(match pin {
                Some(pin) => Answer::Pin(pin),
                None => Answer::Cancelled,
            });
        }

        // not the question showing: put it back
        other => *ASKING.lock().unwrap_or_else(PoisonError::into_inner) = other,
    }
}

/// Starts answering on the socket, once, on threads of its own.
pub fn serve() {
    thread::spawn(|| {
        let listener = match listen() {
            Ok(listener) => listener,
            Err(error) => {
                log::warn!("passwords: no pinentry socket: {error}");
                return;
            }
        };

        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                if let Err(error) = answer_caller(stream) {
                    log::warn!("passwords: pinentry caller: {error}");
                }
            });
        }
    });
}

// the socket in a folder of the runtime directory that only this user can enter
fn listen() -> io::Result<UnixListener> {
    let path = peer::socket_path().ok_or_else(|| io::Error::other("no XDG_RUNTIME_DIR"))?;
    let folder = path.parent().ok_or_else(|| io::Error::other("no folder"))?;

    match fs::create_dir(folder) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }

    // a folder that is ours and no link to somewhere else
    let meta = fs::symlink_metadata(folder)?;
    if !meta.is_dir() || meta.uid() != peer::current_uid() {
        return Err(io::Error::other("the socket's folder isn't ours"));
    }
    fs::set_permissions(folder, fs::Permissions::from_mode(0o700))?;

    // a socket left by an earlier run; bind below fails if it stays
    let _ = fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;

    Ok(listener)
}

fn answer_caller(mut stream: UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    wire::read_hello(&mut stream)?;

    let welcome = trusted(&stream);

    wire::write_welcome(&mut stream, welcome)?;

    if !welcome {
        return Ok(());
    }

    stream.set_read_timeout(Some(ANSWER_TIME))?;

    // a caller may ask again, after a wrong password
    loop {
        let prompt = match wire::read_request(&mut stream) {
            Ok(prompt) => prompt,
            Err(_) => return Ok(()),
        };

        // one question showing at a time; the next caller waits for this one's answer
        let answer = {
            let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);

            if !expected() {
                Answer::Refused
            } else {
                ask(prompt)
            }
        };

        wire::write_answer(&mut stream, &answer)?;
    }
}

// the checks above
fn trusted(stream: &UnixStream) -> bool {
    if !expected() {
        return false;
    }

    let Ok(caller) = Peer::of(stream) else {
        return false;
    };

    if caller.uid != peer::current_uid() {
        return false;
    }

    if !peer::exe(caller.pid).is_some_and(|exe| peer::is_program(&exe, &["amane-pinentry"])) {
        return false;
    }

    let Some(agent) = peer::parent(caller.pid) else {
        return false;
    };

    if !peer::exe(agent).is_some_and(|exe| peer::is_program(&exe, &["rbw-agent"])) {
        return false;
    }

    // the agent reads the same config, so its server and account are the ones shown
    for variable in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
        "RBW_PROFILE",
    ] {
        let ours = std::env::var(variable)
            .ok()
            .filter(|value| !value.is_empty());

        let Some(theirs) = peer::environment(agent, variable) else {
            return false;
        };

        if theirs.filter(|value| !value.is_empty()) != ours {
            return false;
        }
    }

    caller.still_there()
}

// shows the question and waits for the dialog
fn ask(prompt: Prompt) -> Answer {
    let (reply, answered): (Sender<Answer>, Receiver<Answer>) = mpsc::channel();

    let id = {
        let mut entry = PinEntry::write();
        entry.next_id += 1;

        let id = entry.next_id;

        let shown = |text: &str| {
            let mut text = text.trim().replace(['\r', '\t'], " ");
            assuan::clip(&mut text, SHOWN_LIMIT);
            text
        };

        entry.question = Some(PinQuestion {
            id,
            description: shown(&prompt.description),
            prompt: shown(&prompt.prompt),
            error: shown(&prompt.error),
        });

        id
    };

    *ASKING.lock().unwrap_or_else(PoisonError::into_inner) = Some((id, reply));

    let answer = answered
        .recv_timeout(ANSWER_TIME)
        .unwrap_or(Answer::Cancelled);

    ASKING.lock().unwrap_or_else(PoisonError::into_inner).take();

    let mut entry = PinEntry::write();
    if entry
        .question
        .as_ref()
        .is_some_and(|question| question.id == id)
    {
        entry.question = None;
    }

    answer
}
