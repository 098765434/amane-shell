//! The socket the shell talks to, and the thread Cactus runs on. One shell
//! at a time: a new connection takes over from the last (the shell
//! restarted). Requests go to the engine's thread in order; of several
//! answers asked for while it was busy, only the newest is worked out (the
//! others were for what's since been typed over).

use amane_base::sync::Locked as _;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use amane_assistant::protocol::{Envelope, Message, Request};

use crate::audio::{self, Recording};
use crate::engine::{ENGINE, Engine};

/// How much is heard before Whistle hears it: half a second.
const PASS: usize = audio::RATE as usize / 2;
/// How loud a piece is to be a voice rather than the room (on the
/// recording's 0 … 1 scale; a quiet room reads under 0.01).
const VOICE: f32 = 0.05;
/// How many pieces before the voice starts are kept for it: 0.3 s.
const LEAD_IN: usize = 3;

enum Job {
    Hello(u64),
    Complete {
        id: u64,
        system: String,
        tools: String,
        text: String,
    },
    ListenStart {
        language: Option<String>,
        keywords: Vec<String>,
    },
    /// A tenth of a second heard, and how loud.
    Audio(Vec<f32>, f32),
    ListenStop(u64),
    ListenCancel,
}

/// Where messages go: the shell connected now.
#[derive(Default)]
struct Out(Mutex<Option<UnixStream>>);

impl Out {
    fn send(&self, message: &Message) {
        let Ok(mut line) = serde_json::to_string(message) else {
            return;
        };
        line.push('\n');
        let mut client = self.0.locked();
        if let Some(stream) = client.as_mut()
            && stream.write_all(line.as_bytes()).is_err()
        {
            *client = None;
        }
    }
}

pub fn serve(engine: Engine) -> Result<(), String> {
    let path = amane_assistant::socket_path();
    if UnixStream::connect(&path).is_ok() {
        return Err(format!("another one already serves {}", path.display()));
    }
    // a socket an earlier run left, if any
    let _ = std::fs::remove_file(&path);
    let listener =
        UnixListener::bind(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
        log::warn!("couldn't make the socket private: {error}");
    }
    log::info!("serving {}", path.display());

    let out = Arc::new(Out::default());
    let (jobs, queue) = mpsc::channel();
    {
        let out = out.clone();
        std::thread::spawn(move || work(engine, queue, &out));
    }
    let recording: Arc<Mutex<Option<Recording>>> = Arc::default();

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let Ok(writer) = stream.try_clone() else {
            continue;
        };
        // the last shell's listening ends with it
        if let Some(recording) = recording.locked().take() {
            recording.stop();
            // the worker has gone: the server is ending
            let _ = jobs.send(Job::ListenCancel);
        }
        *out.0.locked() = Some(writer);
        let (jobs, out, recording) = (jobs.clone(), out.clone(), recording.clone());
        std::thread::spawn(move || read(stream, &jobs, &out, &recording));
    }
    Ok(())
}

/// Reads one shell's requests until it goes.
fn read(
    stream: UnixStream,
    jobs: &Sender<Job>,
    out: &Arc<Out>,
    recording: &Arc<Mutex<Option<Recording>>>,
) {
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        let envelope: Envelope = match serde_json::from_str(&line) {
            Ok(envelope) => envelope,
            Err(error) => {
                out.send(&Message::Error {
                    id: None,
                    error: format!("not a request: {error}"),
                });
                continue;
            }
        };
        let id = envelope.id;
        let job = match envelope.request {
            Request::Hello => Job::Hello(id),
            Request::Complete {
                system,
                tools,
                text,
            } => Job::Complete {
                id,
                system,
                tools,
                text,
            },
            Request::ListenStart { language, keywords } => {
                let mut slot = recording.locked();
                if let Some(before) = slot.take() {
                    before.stop();
                }
                let (pieces, levels, failures) = (jobs.clone(), out.clone(), out.clone());
                *slot = Some(Recording::start(
                    move |pcm, rms| {
                        levels.send(&Message::Level { rms });
                        // the worker has gone: the server is ending
                        let _ = pieces.send(Job::Audio(pcm, rms));
                    },
                    move |error| {
                        failures.send(&Message::Error {
                            id: Some(id),
                            error,
                        })
                    },
                ));
                Job::ListenStart { language, keywords }
            }
            Request::ListenStop => {
                stop(recording);
                Job::ListenStop(id)
            }
            Request::ListenCancel => {
                stop(recording);
                Job::ListenCancel
            }
        };
        if jobs.send(job).is_err() {
            break;
        }
    }
}

/// Stops the recording, once its last piece is handed on.
fn stop(recording: &Mutex<Option<Recording>>) {
    if let Some(recording) = recording.locked().take() {
        recording.stop();
    }
}

/// The engine's thread: every Cactus call, in order.
fn work(mut engine: Engine, queue: Receiver<Job>, out: &Out) {
    let mut heard: Vec<f32> = Vec::new();
    // Whistle makes words up from silence ("Bye-bye."), so it hears
    // nothing until a voice starts: till then, the last few pieces
    let mut voiced = false;
    let mut before: std::collections::VecDeque<Vec<f32>> = std::collections::VecDeque::new();
    while let Ok(first) = queue.recv() {
        let mut batch = vec![first];
        batch.extend(queue.try_iter());
        // of the answers asked for meanwhile, the newest
        let newest = batch
            .iter()
            .rposition(|job| matches!(job, Job::Complete { .. }));
        for (index, job) in batch.into_iter().enumerate() {
            match job {
                Job::Hello(id) => match engine.models() {
                    Ok(models) => out.send(&Message::Hello {
                        id,
                        engine: ENGINE.into(),
                        models,
                    }),
                    Err(error) => out.send(&Message::Error {
                        id: Some(id),
                        error,
                    }),
                },
                Job::Complete { id, .. } if Some(index) != newest => out.send(&Message::Error {
                    id: Some(id),
                    error: "superseded".into(),
                }),
                Job::Complete {
                    id,
                    system,
                    tools,
                    text,
                } => {
                    let started = Instant::now();
                    match engine.complete(&system, &tools, &text) {
                        Ok(answer) => out.send(&Message::Answer {
                            id,
                            answer,
                            ms: started.elapsed().as_millis() as u64,
                        }),
                        Err(error) => out.send(&Message::Error {
                            id: Some(id),
                            error,
                        }),
                    }
                }
                Job::ListenStart { language, keywords } => {
                    heard.clear();
                    before.clear();
                    voiced = false;
                    if let Err(error) = engine.listen_start(language.as_deref(), &keywords) {
                        out.send(&Message::Error { id: None, error });
                    }
                }
                Job::Audio(pcm, rms) => {
                    if !voiced {
                        before.push_back(pcm);
                        if before.len() > LEAD_IN + 1 {
                            before.pop_front();
                        }
                        if rms < VOICE {
                            continue;
                        }
                        voiced = true;
                        heard.extend(before.drain(..).flatten());
                    } else {
                        heard.extend(pcm);
                    }
                    if heard.len() >= PASS {
                        pass(&mut engine, &mut heard, out);
                    }
                }
                Job::ListenStop(id) if !voiced => {
                    // nothing but the room: nothing heard
                    engine.listen_cancel();
                    before.clear();
                    out.send(&Message::Heard {
                        id,
                        text: String::new(),
                        ms: 0,
                    });
                }
                Job::ListenStop(id) => {
                    voiced = false;
                    let started = Instant::now();
                    if !heard.is_empty() {
                        pass(&mut engine, &mut heard, out);
                    }
                    match engine.listen_stop() {
                        Ok(text) => out.send(&Message::Heard {
                            id,
                            text,
                            ms: started.elapsed().as_millis() as u64,
                        }),
                        Err(error) => out.send(&Message::Error {
                            id: Some(id),
                            error,
                        }),
                    }
                }
                Job::ListenCancel => {
                    heard.clear();
                    before.clear();
                    voiced = false;
                    engine.listen_cancel();
                }
            }
        }
    }
}

/// Whistle hears what's gathered; what it's heard so far goes out.
fn pass(engine: &mut Engine, heard: &mut Vec<f32>, out: &Out) {
    match engine.listen(heard) {
        Ok(Some(text)) => out.send(&Message::Partial { text }),
        Ok(None) => {}
        Err(error) => out.send(&Message::Error { id: None, error }),
    }
    heard.clear();
}
