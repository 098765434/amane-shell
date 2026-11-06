//! The microphone, while listening: a PulseAudio recording stream named
//! "Assistant" (so the bar's privacy indicator shows it, and the mixer
//! names it), 16 kHz mono floats as Whistle takes them, read a tenth of a
//! second at a time. Nothing is kept: each piece goes to Whistle and is
//! dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

pub const RATE: u32 = 16_000;
/// A tenth of a second.
const PIECE: usize = RATE as usize / 10;

/// A recording running on its own thread.
pub struct Recording {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Recording {
    /// Starts recording, handing each piece to `heard` with how loud it
    /// was (0 … 1). `failed` hears why it couldn't, if it can't.
    pub fn start(
        mut heard: impl FnMut(Vec<f32>, f32) + Send + 'static,
        failed: impl FnOnce(String) + Send + 'static,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        let thread = std::thread::spawn(move || {
            // for tests: a recording played as the microphone
            if let Some(path) = std::env::var_os("AMANE_ASSISTANT_FAKE_MIC") {
                return fake(&path, &flag, &mut heard, failed);
            }
            let simple = match open() {
                Ok(simple) => simple,
                Err(error) => return failed(error),
            };
            let mut bytes = vec![0u8; PIECE * 4];
            while flag.load(Ordering::SeqCst) {
                if let Err(error) = simple.read(&mut bytes) {
                    return failed(format!("The microphone stopped: {error}"));
                }
                let pcm: Vec<f32> = bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect();
                let loudness = rms(&pcm);
                heard(pcm, loudness);
            }
        });
        Self {
            running,
            thread: Some(thread),
        }
    }

    /// Stops, once the piece being read is handed on.
    pub fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// `path` (raw 16 kHz mono f32, little-endian) as if heard, a piece every
/// tenth of a second, then the quiet of a room until stopped.
fn fake(
    path: &std::ffi::OsStr,
    running: &AtomicBool,
    heard: &mut impl FnMut(Vec<f32>, f32),
    failed: impl FnOnce(String),
) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return failed(format!("{}: {error}", path.to_string_lossy())),
    };
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let mut pieces = samples.chunks(PIECE);
    while running.load(Ordering::SeqCst) {
        let pcm = pieces
            .next()
            .map_or_else(|| vec![0.0; PIECE], <[f32]>::to_vec);
        let loudness = rms(&pcm);
        heard(pcm, loudness);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn open() -> Result<Simple, String> {
    let spec = Spec {
        format: Format::F32le,
        channels: 1,
        rate: RATE,
    };
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: (PIECE * 4) as u32,
    };
    Simple::new(
        None,
        "Assistant",
        Direction::Record,
        None,
        "Listening",
        &spec,
        None,
        Some(&attr),
    )
    .map_err(|error| format!("The microphone didn't open: {error}"))
}

/// How loud `pcm` is, 0 … 1, scaled so speech reads well up the meter.
fn rms(pcm: &[f32]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    let mean = pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len() as f32;
    (mean.sqrt() * 4.0).min(1.0)
}
