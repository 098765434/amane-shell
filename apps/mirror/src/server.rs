//! Starting scrcpy's server on the phone and taking its sockets, the way
//! scrcpy's own client does (`app/src/server.c`): push the jar, open a
//! tunnel, run it with `app_process`, accept video, audio and control in
//! that order.

use amane_kit::sync::Locked as _;
use std::io::{BufRead, BufReader, Read};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::protocol::{self, App};

/// Where the jar goes on the phone; `/data/local/tmp` is the shell user's.
const ON_PHONE: &str = "/data/local/tmp/scrcpy-server.jar";

/// How long the server gets to connect back.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The server's jar on this computer, from the build, or `AMANE_SCRCPY_SERVER`.
fn server_jar() -> Option<String> {
    std::env::var("AMANE_SCRCPY_SERVER")
        .ok()
        .or_else(|| option_env!("SCRCPY_SERVER").map(String::from))
}

/// The server's version, which must be exactly the jar's.
fn version() -> &'static str {
    option_env!("SCRCPY_VERSION").unwrap_or("4.1")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Front,
    Back,
}

/// What to ask the server for.
#[derive(Debug, Clone)]
pub struct Options {
    pub serial: String,
    pub video: bool,
    pub audio: bool,
    pub control: bool,
    /// The frame's longer side, none for the phone's own size.
    pub max_size: Option<u32>,
    pub bit_rate: u32,
    pub max_fps: Option<u32>,
    /// Whether the phone's clipboard comes here by itself; off while KDE
    /// Connect already shares it.
    pub clipboard_autosync: bool,
    /// A virtual display of `WxH/dpi` instead of the phone's screen, which
    /// resizes with the window.
    pub new_display: Option<String>,
    pub camera: Option<(Facing, String)>,
    /// An app to start once connected (on the virtual display, if there's
    /// one), by package; not a server argument but a control message.
    pub start_app: Option<String>,
}

impl Options {
    pub fn screen(serial: &str) -> Self {
        Self {
            serial: serial.to_string(),
            video: true,
            audio: true,
            control: true,
            max_size: None,
            bit_rate: 8_000_000,
            max_fps: None,
            clipboard_autosync: true,
            new_display: None,
            camera: None,
            start_app: None,
        }
    }

    fn arguments(&self, scid: u32, forward: bool) -> Vec<String> {
        let mut arguments = vec![
            format!("scid={scid:08x}"),
            "log_level=info".into(),
            format!("video_bit_rate={}", self.bit_rate),
            "video_codec=h264".into(),
            "audio_codec=opus".into(),
        ];
        let mut add = |argument: String| arguments.push(argument);

        if !self.video {
            add("video=false".into());
        }
        if !self.audio {
            add("audio=false".into());
        }
        if !self.control {
            add("control=false".into());
        }
        if forward {
            add("tunnel_forward=true".into());
        }
        if let Some(max_size) = self.max_size {
            add(format!("max_size={max_size}"));
        }
        if let Some(max_fps) = self.max_fps {
            add(format!("max_fps={max_fps}"));
        }
        if !self.clipboard_autosync {
            add("clipboard_autosync=false".into());
        }
        if let Some(display) = &self.new_display {
            add(format!("new_display={display}"));
            add("flex_display=true".into());
        }
        if let Some((facing, size)) = &self.camera {
            add("video_source=camera".into());
            add(format!(
                "camera_facing={}",
                match facing {
                    Facing::Front => "front",
                    Facing::Back => "back",
                }
            ));
            add(format!("camera_size={size}"));
        }

        arguments
    }
}

/// A running server: its sockets, and the process that keeps it alive.
pub struct Connection {
    pub name: String,
    pub video: Option<TcpStream>,
    pub audio: Option<TcpStream>,
    pub control: Option<TcpStream>,
    process: Child,
    log: Log,
}

impl Connection {
    /// The server's last complaint, for when the stream ends.
    pub fn last_error(&self) -> Option<String> {
        self.log.last_error()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // the server may have ended already
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// What the server printed, kept to say why it stopped.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn follow(&self, from: impl Read + Send + 'static) {
        let log = self.clone();
        thread::spawn(move || {
            for line in BufReader::new(from).lines().map_while(Result::ok) {
                log::info!(target: "scrcpy-server", "{line}");
                log.0.locked().push(line);
            }
        });
    }

    fn last_error(&self) -> Option<String> {
        let lines = self.0.locked();
        lines
            .iter()
            .rev()
            .find_map(|line| line.split_once("ERROR: ").map(|(_, why)| why.to_string()))
    }

    fn text(&self) -> String {
        self.0.locked().join("\n")
    }
}

fn adb(serial: &str, arguments: &[&str]) -> Result<String, String> {
    amane_phone::adb(Some(serial), arguments).map_err(|error| error.to_string())
}

fn push(serial: &str) -> Result<(), String> {
    let jar = server_jar().ok_or("this build has no scrcpy server")?;
    adb(serial, &["push", &jar, ON_PHONE]).map(|_| ())
}

fn run_server(serial: &str, arguments: &[String], log: &Log) -> Result<Child, String> {
    let mut command = Command::new("adb");
    command.args([
        "-s",
        serial,
        "shell",
        &format!("CLASSPATH={ON_PHONE}"),
        "app_process",
        "/",
        "com.genymobile.scrcpy.Server",
        version(),
    ]);
    command.args(arguments);

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("adb didn't start: {error}"))?;

    if let Some(out) = child.stdout.take() {
        log.follow(out);
    }
    if let Some(err) = child.stderr.take() {
        log.follow(err);
    }

    Ok(child)
}

fn random_scid() -> u32 {
    // 31 bits, as scrcpy's are; the time is random enough to keep two sessions apart
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.subsec_nanos() ^ time.as_secs() as u32);
    (nanos ^ std::process::id().rotate_left(16)) & 0x7fff_ffff
}

/// Starts the server and takes its sockets.
pub fn start(options: &Options) -> Result<Connection, String> {
    push(&options.serial)?;

    let scid = random_scid();
    let socket = format!("localabstract:scrcpy_{scid:08x}");

    // the phone connects to us; adb reverse works over USB and Wireless debugging alike
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    match adb(
        &options.serial,
        &["reverse", &socket, &format!("tcp:{port}")],
    ) {
        Ok(_) => {
            let result = start_reverse(options, scid, &listener);
            // tidying after the start, whose result is the one that matters
            let _ = adb(&options.serial, &["reverse", "--remove", &socket]);
            result
        }
        // some phones refuse reverse tunnels; then we connect to it
        Err(_) => start_forward(options, scid, &socket),
    }
}

fn wanted(options: &Options) -> usize {
    [options.video, options.audio, options.control]
        .iter()
        .filter(|on| **on)
        .count()
}

fn start_reverse(
    options: &Options,
    scid: u32,
    listener: &TcpListener,
) -> Result<Connection, String> {
    let log = Log::default();
    let mut process = run_server(&options.serial, &options.arguments(scid, false), &log)?;

    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;

    let mut sockets = Vec::new();
    let deadline = Instant::now() + CONNECT_TIMEOUT;

    while sockets.len() < wanted(options) {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|error| error.to_string())?;
                sockets.push(stream);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if let Ok(Some(_)) = process.try_wait() {
                    return Err(log
                        .last_error()
                        .unwrap_or_else(|| "The phone stopped the server.".into()));
                }
                if Instant::now() > deadline {
                    // past the deadline: it may have ended already
                    let _ = process.kill();
                    return Err("The phone didn't connect back in time.".into());
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error.to_string()),
        }
    }

    connection(options, sockets, process, log)
}

fn start_forward(options: &Options, scid: u32, socket: &str) -> Result<Connection, String> {
    let port: u16 = adb(&options.serial, &["forward", "tcp:0", socket])?
        .trim()
        .parse()
        .map_err(|_| "adb forward gave no port".to_string())?;

    let log = Log::default();
    let process = run_server(&options.serial, &options.arguments(scid, true), &log);

    let result = process.and_then(|process| {
        let mut sockets = Vec::new();

        // the server listens once it's up; the first socket gets a byte to say it's really there
        for index in 0..wanted(options) {
            let mut stream = None;
            for _ in 0..100 {
                if let Ok(mut attempt) = TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
                    if index > 0 {
                        stream = Some(attempt);
                        break;
                    }
                    let mut dummy = [0u8; 1];
                    if attempt.read_exact(&mut dummy).is_ok() {
                        stream = Some(attempt);
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
            sockets.push(stream.ok_or("The phone's server didn't answer.")?);
        }

        connection(options, sockets, process, log)
    });

    // tidying after the session, whose result is the one that matters
    let _ = adb(
        &options.serial,
        &["forward", "--remove", &format!("tcp:{port}")],
    );

    result
}

fn connection(
    options: &Options,
    sockets: Vec<TcpStream>,
    process: Child,
    log: Log,
) -> Result<Connection, String> {
    let mut sockets = sockets.into_iter();

    let mut video = options.video.then(|| sockets.next()).flatten();
    let mut audio = options.audio.then(|| sockets.next()).flatten();
    let control = options.control.then(|| sockets.next()).flatten();

    // the first socket opens with the phone's name
    let first = video.as_mut().or(audio.as_mut());
    let mut name = [0u8; 64];
    let name = match first {
        Some(stream) => {
            stream
                .read_exact(&mut name)
                .map_err(|_| "The phone closed the connection.".to_string())?;
            protocol::device_name(&name)
        }
        None => String::new(),
    };

    if let Some(control) = &control {
        // only latency rides on it
        let _ = control.set_nodelay(true);
    }

    Ok(Connection {
        name,
        video,
        audio,
        control,
        process,
        log,
    })
}

/// The phone's apps, as the server lists them.
pub fn list_apps(serial: &str) -> Result<Vec<App>, String> {
    push(serial)?;

    let log = Log::default();
    let arguments = [
        format!("scid={:08x}", random_scid()),
        "log_level=info".into(),
        "video=false".into(),
        "audio=false".into(),
        "control=false".into(),
        "list_apps=true".into(),
    ];

    let mut process = run_server(serial, &arguments, &log)?;
    // its end is read from the log it leaves
    let _ = process.wait();

    // the readers may still be taking the last lines
    thread::sleep(Duration::from_millis(100));

    let apps = protocol::apps(&log.text());

    if apps.is_empty() {
        return Err(log
            .last_error()
            .unwrap_or_else(|| "No apps came back.".into()));
    }

    Ok(apps)
}
