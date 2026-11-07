//! One mirroring session: the server on the phone, and a thread for each
//! of its sockets (video, sound, control both ways). Its state is a
//! Service, so the window redraws as the session starts, changes size or
//! ends.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{LazyLock, Mutex};
use std::thread;

use amane::{Clipboard, Service, VideoFeed};

use crate::audio::{self, Sound};
use crate::decode;
use crate::protocol::{self, Control, DeviceMessage};
use crate::server::{self, Options};
use amane_kit::sync::Locked as _;

/// Where a session stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Not started: no phone adb can use yet.
    Idle,
    Starting,
    Live,
    /// Stopped, and why when it failed.
    Ended(Option<String>),
}

pub struct Mirror {
    pub phase: Phase,
    /// The phone's name, as the server tells it.
    pub name: String,
    /// The frame's size, from the server's session packets.
    pub size: Option<(u32, u32)>,
    /// Decoded on the GPU.
    pub hardware: bool,
    pub sound: Option<Sound>,
    /// The phone's screen is off while it's mirrored.
    pub screen_off: bool,
}

impl Service for Mirror {
    fn new() -> Self {
        Self {
            phase: Phase::Idle,
            name: String::new(),
            size: None,
            hardware: false,
            sound: None,
            screen_off: false,
        }
    }

    // the session's threads write it
    fn listen() {}
}

/// The frames shown, shared by every session the app runs.
pub static FEED: LazyLock<VideoFeed> = LazyLock::new(VideoFeed::new);

static CONTROL: Mutex<Option<Sender<Control>>> = Mutex::new(None);

static RUNNING: AtomicBool = AtomicBool::new(false);

static CLIPBOARD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Sends a message to the phone, dropped when no session is live.
pub fn send(message: Control) {
    if let Some(sender) = CONTROL.locked().as_ref() {
        // the screen has closed: nothing waits for it
        let _ = sender.send(message);
    }
}

/// Pastes this computer's clipboard into the phone's focused field.
pub fn paste() {
    Clipboard::paste(|text| {
        send(Control::SetClipboard {
            sequence: CLIPBOARD_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            text,
            paste: true,
        });
    });
}

pub fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

/// Starts a session with `options`, unless one is running.
pub fn start(options: Options) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        {
            let mut mirror = Mirror::write();
            mirror.phase = Phase::Starting;
            mirror.sound = None;
        }

        let why = run(&options).err();

        *CONTROL.locked() = None;
        FEED.clear();

        let mut mirror = Mirror::write();
        mirror.phase = Phase::Ended(why);
        mirror.size = None;
        drop(mirror);

        RUNNING.store(false, Ordering::SeqCst);
    });
}

fn run(options: &Options) -> Result<(), String> {
    let mut connection = server::start(options)?;

    Mirror::write().name = connection.name.clone();

    if let Some(control) = connection.control.take() {
        let reader = control.try_clone().map_err(|error| error.to_string())?;
        let (sender, messages) = mpsc::channel();

        *CONTROL.locked() = Some(sender);

        thread::spawn(move || write_control(control, messages));
        thread::spawn(move || read_device(reader));

        // the server waits a moment for its virtual display before starting it there
        if let Some(app) = &options.start_app {
            send(Control::StartApp(app.clone()));
        }
    }

    if let Some(audio) = connection.audio.take() {
        thread::spawn(move || {
            let result = audio::run(audio, |sound| Mirror::write().sound = Some(sound));
            if let Err(why) = result {
                log::warn!("sound: {why}");
                Mirror::write().sound = Some(Sound::Unavailable);
            }
        });
    }

    let Some(video) = connection.video.take() else {
        return Ok(());
    };

    let hardware = std::env::var_os("AMANE_MIRROR_SOFTWARE").is_none();

    let result = decode::run(video, &FEED, hardware, |width, height, on_gpu| {
        let mut mirror = Mirror::write();
        mirror.hardware = on_gpu;
        if width > 0 && height > 0 {
            mirror.size = Some((width, height));
            mirror.phase = Phase::Live;
        }
    });

    // ended by the phone: say what its server said, if it complained
    match result {
        Ok(()) => match connection.last_error() {
            Some(why) => Err(why),
            None => Ok(()),
        },
        Err(why) => Err(why),
    }
}

fn write_control(mut socket: TcpStream, messages: mpsc::Receiver<Control>) {
    for message in messages {
        if socket.write_all(&message.serialize()).is_err() {
            break;
        }
    }
}

fn read_device(mut socket: TcpStream) {
    let mut pending: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 4096];

    while let Ok(read) = socket.read(&mut buffer) {
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&buffer[..read]);

        loop {
            match protocol::device_message(&pending) {
                Ok(Some((message, used))) => {
                    pending.drain(..used);
                    if let DeviceMessage::Clipboard(text) = message {
                        Clipboard::copy(text);
                    }
                }
                Ok(None) => break,
                // a message this doesn't know: the rest can't be read in step
                Err(_) => return,
            }
        }
    }
}
