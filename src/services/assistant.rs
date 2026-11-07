//! The launcher's assistant, the shell's end: reading a query as a command,
//! by the rules at once or by Needle 3 in `amane-assistant` a moment after
//! the last key, and listening for one while the shell asks.
//!
//! The daemon is optional: without it (not installed, not running, its
//! socket gone) the rules still read what they can and nothing waits on
//! it. Its answers come back on a thread of their own and are kept for
//! the view to read, as file search's are; [`Assistant`] marks a change.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use amane_assistant::client::{self, Receiver, Sender};
use amane_assistant::protocol::{Message, Request};
use amane_assistant::{Reading, Snapshot, Source, rules, tools};

amane_kit::marker! {
    /// An answer came, or what's being heard changed.
    pub Assistant
}

/// How long after the last key Needle is asked.
const DEBOUNCE: Duration = Duration::from_millis(180);
/// Below this, what Needle reads isn't offered.
pub const OFFERED: f32 = 0.3;
/// How long the daemon is left alone after it couldn't be reached.
const RETRY: Duration = Duration::from_secs(10);

/// What's being heard, while listening.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listening {
    pub on: bool,
    /// What's been heard so far.
    pub partial: String,
    /// How loud the microphone is, 0 … 1.
    pub level: f32,
    pub error: Option<String>,
}

struct State {
    // the last answer to come, for its text
    done: Option<(String, Option<Reading>)>,
    // the newest text asked about
    asked: Option<String>,
    // the request in flight, and what it was for: only the newest
    waiting: Option<(u64, String, Snapshot)>,
    // when the daemon may be tried again
    retry_at: Option<Instant>,
    listening: Listening,
    // what was heard, once listening stopped, until it's taken
    heard: Option<String>,
    // what the daemon said it runs
    hello: Option<String>,
}

static STATE: Mutex<State> = Mutex::new(State {
    done: None,
    asked: None,
    waiting: None,
    retry_at: None,
    listening: Listening {
        on: false,
        partial: String::new(),
        level: 0.0,
        error: None,
    },
    heard: None,
    hello: None,
});
static SENDER: Mutex<Option<Sender>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// What `text` asks, as far as it's known now: a rule's reading at once,
/// Needle's once it answers (asking it if it hasn't been). `snapshot` is
/// what the machine has, made only when it's needed.
pub fn read(text: &str, snapshot: impl FnOnce() -> Snapshot) -> Option<Reading> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let snapshot = snapshot();
    if let Some(command) = rules::read(text, &snapshot) {
        return Some(Reading {
            command,
            confidence: 1.0,
            source: Source::Rules,
        });
    }
    // a word alone, a bare name or a question isn't a command: the launcher's
    if !rules::worth_asking(text, &snapshot) {
        return None;
    }
    Assistant::watch();
    let mut state = state();
    if let Some((done, reading)) = &state.done
        && done == text
    {
        return reading.clone();
    }
    if state.asked.as_deref() != Some(text) {
        state.asked = Some(text.to_string());
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        let text = text.to_string();
        std::thread::spawn(move || ask(text, snapshot, generation));
    }
    None
}

fn ask(text: String, snapshot: Snapshot, generation: u64) {
    std::thread::sleep(DEBOUNCE);
    if GENERATION.load(Ordering::SeqCst) != generation {
        return;
    }
    let request = Request::Complete {
        system: tools::SYSTEM.to_string(),
        tools: tools::tools(&snapshot),
        text: text.clone(),
    };
    let asked = text.clone();
    let sent = send_then(request, move |id| {
        // only the newest is waited for
        state().waiting = Some((id, asked, snapshot));
    });
    if sent.is_none() {
        // no daemon: nothing more is coming for it
        state().done = Some((text, None));
        Assistant::changed();
    }
}

/// Sends `request`, connecting first if need be; its id, or `None` when
/// the daemon can't be reached.
fn send(request: Request) -> Option<u64> {
    send_then(request, |_| {})
}

/// Sends `request` now that someone asked for it: trying the daemon even
/// if it couldn't be reached a moment ago.
fn send_now(request: Request) -> Option<u64> {
    state().retry_at = None;
    send(request)
}

/// Sends `request`, and hands `sent` its id before its answer can be read.
/// Locks: the sender, then the state, never the other way round.
fn send_then(request: Request, sent: impl FnOnce(u64)) -> Option<u64> {
    let mut sender = SENDER.lock().unwrap_or_else(|e| e.into_inner());
    if sender.is_none() {
        let mut state = state();
        if state.retry_at.is_some_and(|at| Instant::now() < at) {
            return None;
        }
        match client::connect(&amane_assistant::socket_path()) {
            Ok((connected, receiver)) => {
                state.retry_at = None;
                *sender = Some(connected);
                std::thread::spawn(move || listen(receiver));
            }
            Err(_) => {
                state.retry_at = Some(Instant::now() + RETRY);
                return None;
            }
        }
    }
    match sender.as_mut()?.send(request) {
        Ok(id) => {
            sent(id);
            Some(id)
        }
        Err(_) => {
            *sender = None;
            None
        }
    }
}

/// Reads what the daemon sends until it's gone.
fn listen(mut receiver: Receiver) {
    while let Ok(message) = receiver.recv() {
        let mut state = state();
        match message {
            Message::Answer { id, answer, .. } => {
                let Some((_, text, snapshot)) = take(&mut state.waiting, id) else {
                    continue;
                };
                let reading =
                    tools::read(&answer, &snapshot).filter(|reading| reading.confidence >= OFFERED);
                state.done = Some((text, reading));
            }
            Message::Error { id, error } => {
                if let Some((_, text, _)) = id.and_then(|id| take(&mut state.waiting, id))
                    && error != "superseded"
                {
                    state.done = Some((text, None));
                }
                if error != "superseded" {
                    eprintln!("amane-assistant: {error}");
                    if state.listening.on {
                        state.listening.error = Some(error);
                    }
                }
            }
            Message::Level { rms } => state.listening.level = rms,
            Message::Partial { text } => state.listening.partial = text,
            Message::Heard { text, .. } => {
                state.listening = Listening::default();
                state.heard = Some(text);
            }
            Message::Hello { engine, models, .. } => {
                state.hello = Some(format!("{engine} ({models})"));
            }
        }
        drop(state);
        Assistant::changed();
    }
    *SENDER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let mut state = state();
    state.waiting = None;
    state.listening = Listening::default();
    state.retry_at = Some(Instant::now() + Duration::from_secs(2));
    drop(state);
    Assistant::changed();
}

/// The request in flight, if it's `id`'s.
fn take(waiting: &mut Option<(u64, String, Snapshot)>, id: u64) -> Option<(u64, String, Snapshot)> {
    waiting.take_if(|(waited, _, _)| *waited == id)
}

/// Starts listening, Whistle told to listen for `keywords`. Whether the
/// daemon could be asked.
pub fn listen_start(keywords: Vec<String>) -> bool {
    let started = send_now(Request::ListenStart {
        language: None,
        keywords,
    })
    .is_some();
    let mut state = state();
    state.heard = None;
    state.listening = Listening {
        on: started,
        error: (!started).then(|| String::from("The assistant isn't running.")),
        ..Listening::default()
    };
    drop(state);
    Assistant::changed();
    started
}

/// Stops listening; what was heard comes as [`take_heard`].
pub fn listen_stop() {
    if state().listening.on {
        let _ = send(Request::ListenStop);
    }
}

/// Stops listening, forgetting what was heard.
pub fn listen_cancel() {
    let on = std::mem::take(&mut state().listening).on;
    if on {
        let _ = send(Request::ListenCancel);
        Assistant::changed();
    }
}

/// What's being heard now.
pub fn listening() -> Listening {
    Assistant::watch();
    state().listening.clone()
}

/// What was heard, once, after listening stopped.
pub fn take_heard() -> Option<String> {
    Assistant::watch();
    state().heard.take()
}

/// What the daemon runs, once it's said; asks it if it hasn't.
pub fn status() -> Option<String> {
    let hello = state().hello.clone();
    if hello.is_none() {
        std::thread::spawn(|| {
            let _ = send_now(Request::Hello);
        });
    }
    hello
}
