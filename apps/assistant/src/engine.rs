//! Cactus, held: Needle 3 and Whistle loaded once, and every call to them
//! from the one thread that owns this. Cactus keeps one model set and one
//! conversation for the whole process, so nothing else touches it.

use std::ffi::{CStr, CString, c_char, c_int};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;

/// What a call gives back at most, to start with; one that doesn't fit is
/// asked again with room for it.
const OUT: usize = 16 * 1024;
/// How many tokens an answer takes at most.
const MAX_TOKENS: c_int = 256;

#[cfg(cactus)]
mod ffi {
    use std::ffi::{c_char, c_int};

    unsafe extern "C" {
        pub fn cactus_load(cact: *const u8, n: u64) -> c_int;
        pub fn cactus_models() -> c_int;
        pub fn cactus_last_error() -> *const c_char;
        pub fn cactus_init(
            system: *const c_char,
            tools: *const c_char,
            tool_index: *const c_char,
        ) -> c_int;
        pub fn cactus_complete(
            input: *const c_char,
            pcm: *const f32,
            samples: c_int,
            max_new_tokens: c_int,
            out: *mut c_char,
            out_capacity: c_int,
        ) -> c_int;
        pub fn cactus_reset();
        pub fn cactus_stream_transcribe_process(
            pcm: *const f32,
            samples: c_int,
            language: *const c_char,
            keywords: *const c_char,
            out: *mut c_char,
            out_capacity: c_int,
        ) -> c_int;
        pub fn cactus_stream_transcribe_stop(out: *mut c_char, out_capacity: c_int) -> c_int;
    }
}

pub const ENGINE: &str = "cactus 4.0.0";

pub struct Engine {
    models: PathBuf,
    cache: PathBuf,
    loaded: bool,
    // the system facts and tools Needle was last set up with
    session: Option<(String, String)>,
    listening: Option<Listening>,
}

struct Listening {
    language: CString,
    keywords: CString,
    // what's been committed so far
    heard: String,
    // what Whistle still weighs
    pending: String,
}

impl Engine {
    /// An engine loading its models from `models` (`needle3.cact`,
    /// `whistle.cact`), keeping its tool index in `cache`.
    pub fn new(models: PathBuf, cache: PathBuf) -> Self {
        Self {
            models,
            cache,
            loaded: false,
            session: None,
            listening: None,
        }
    }

    /// Cactus's loaded kinds (1 text, 2 speech), loading them if they
    /// aren't.
    pub fn models(&mut self) -> Result<i32, String> {
        self.load()?;
        Ok(models())
    }

    fn load(&mut self) -> Result<(), String> {
        if self.loaded {
            return Ok(());
        }
        let started = Instant::now();
        for name in ["needle3.cact", "whistle.cact"] {
            load(&self.models.join(name))?;
        }
        self.loaded = true;
        log::info!(
            "loaded {} in {} ms",
            self.models.display(),
            started.elapsed().as_millis()
        );
        Ok(())
    }

    /// Needle's answer for `text`, offered `tools` with `system`'s facts.
    pub fn complete(&mut self, system: &str, tools: &str, text: &str) -> Result<Value, String> {
        self.load()?;
        if self
            .session
            .as_ref()
            .is_none_or(|(s, t)| s != system || t != tools)
        {
            let index = self.cache.join(format!("tools-{:016x}.idx", hash(tools)));
            init(system, tools, &index)?;
            self.session = Some((system.to_string(), tools.to_string()));
        }
        let answer = complete(text)?;
        reset();
        serde_json::from_str(&answer).map_err(|error| format!("Cactus answered oddly: {error}"))
    }

    pub fn listen_start(
        &mut self,
        language: Option<&str>,
        keywords: &[String],
    ) -> Result<(), String> {
        self.load()?;
        // what was left of one before
        if self.listening.is_some() {
            // a stream that already ended stops as well
            let _ = stream_stop();
        }
        self.listening = Some(Listening {
            language: cstring(language.unwrap_or(""))?,
            keywords: cstring(&keywords.join("\n"))?,
            heard: String::new(),
            pending: String::new(),
        });
        Ok(())
    }

    /// Hears `pcm` (16 kHz mono), giving what's been heard so far when
    /// that changed.
    pub fn listen(&mut self, pcm: &[f32]) -> Result<Option<String>, String> {
        let Some(listening) = &mut self.listening else {
            return Ok(None);
        };
        let answer = stream_process(pcm, &listening.language, &listening.keywords)?;
        let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
        let committed = answer.get("text").and_then(Value::as_str).unwrap_or("");
        let pending = answer.get("pending").and_then(Value::as_str).unwrap_or("");
        let before = (listening.heard.len(), listening.pending.clone());
        join(&mut listening.heard, committed);
        listening.pending = pending.to_string();
        if (listening.heard.len(), listening.pending.clone()) == before {
            return Ok(None);
        }
        let mut sofar = listening.heard.clone();
        join(&mut sofar, &listening.pending);
        Ok(Some(sofar))
    }

    /// Stops listening, giving all that was heard.
    pub fn listen_stop(&mut self) -> Result<String, String> {
        let Some(mut listening) = self.listening.take() else {
            return Ok(String::new());
        };
        let answer = stream_stop()?;
        let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
        let rest = answer.get("text").and_then(Value::as_str).unwrap_or("");
        if rest.is_empty() {
            let pending = std::mem::take(&mut listening.pending);
            join(&mut listening.heard, &pending);
        } else {
            join(&mut listening.heard, rest);
        }
        Ok(listening.heard)
    }

    pub fn listen_cancel(&mut self) {
        if self.listening.take().is_some() {
            // a stream that already ended stops as well
            let _ = stream_stop();
        }
    }
}

fn join(text: &mut String, more: &str) {
    let more = more.trim();
    if more.is_empty() {
        return;
    }
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(more);
}

fn hash(text: &str) -> u64 {
    // FNV-1a: stable across runs, unlike std's
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn cstring(text: &str) -> Result<CString, String> {
    CString::new(text).map_err(|_| String::from("text with a NUL in it"))
}

#[cfg(cactus)]
fn last_error(what: &str) -> String {
    let error = unsafe { ffi::cactus_last_error() };
    let error = if error.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    };
    if error.is_empty() {
        format!("{what} failed")
    } else {
        format!("{what}: {error}")
    }
}

/// Runs `call` with an output buffer, again with a bigger one if it
/// didn't fit, giving what it wrote.
#[cfg(cactus)]
fn with_out(
    what: &str,
    mut call: impl FnMut(*mut c_char, c_int) -> c_int,
) -> Result<String, String> {
    let mut size = OUT;
    for _ in 0..2 {
        let mut out = vec![0 as c_char; size];
        let written = call(out.as_mut_ptr(), size as c_int);
        if written < 0 {
            return Err(last_error(what));
        }
        if (written as usize) < size - 1 {
            let text = unsafe { CStr::from_ptr(out.as_ptr()) };
            return Ok(text.to_string_lossy().into_owned());
        }
        size *= 8;
    }
    Err(format!("{what}: the answer didn't fit"))
}

#[cfg(cactus)]
fn load(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    // Cactus may keep pointing into it: it lives as long as the process
    let bytes: &'static [u8] = Vec::leak(bytes);
    let status = unsafe { ffi::cactus_load(bytes.as_ptr(), bytes.len() as u64) };
    if status < 0 {
        return Err(last_error(&format!("loading {}", path.display())));
    }
    Ok(())
}

#[cfg(cactus)]
fn models() -> i32 {
    unsafe { ffi::cactus_models() }
}

#[cfg(cactus)]
fn init(system: &str, tools: &str, index: &Path) -> Result<(), String> {
    let system = cstring(system)?;
    let tools = cstring(tools)?;
    let index = cstring(&index.to_string_lossy())?;
    let status = unsafe { ffi::cactus_init(system.as_ptr(), tools.as_ptr(), index.as_ptr()) };
    if status < 0 {
        return Err(last_error("setting up the tools"));
    }
    Ok(())
}

#[cfg(cactus)]
fn complete(text: &str) -> Result<String, String> {
    let input = cstring(text)?;
    with_out("answering", |out, capacity| unsafe {
        ffi::cactus_complete(
            input.as_ptr(),
            std::ptr::null(),
            0,
            MAX_TOKENS,
            out,
            capacity,
        )
    })
}

#[cfg(cactus)]
fn reset() {
    unsafe { ffi::cactus_reset() }
}

#[cfg(cactus)]
fn stream_process(pcm: &[f32], language: &CString, keywords: &CString) -> Result<String, String> {
    with_out("listening", |out, capacity| unsafe {
        ffi::cactus_stream_transcribe_process(
            pcm.as_ptr(),
            pcm.len() as c_int,
            language.as_ptr(),
            keywords.as_ptr(),
            out,
            capacity,
        )
    })
}

#[cfg(cactus)]
fn stream_stop() -> Result<String, String> {
    with_out("listening", |out, capacity| unsafe {
        ffi::cactus_stream_transcribe_stop(out, capacity)
    })
}

// Without Cactus in the build, every call says so.

#[cfg(not(cactus))]
const NO_ENGINE: &str = "this build of emaki-assistant has no Cactus";

#[cfg(not(cactus))]
fn load(_: &Path) -> Result<(), String> {
    let _ = (OUT, MAX_TOKENS);
    Err(NO_ENGINE.into())
}

#[cfg(not(cactus))]
fn models() -> i32 {
    0
}

#[cfg(not(cactus))]
fn init(_: &str, _: &str, _: &Path) -> Result<(), String> {
    Err(NO_ENGINE.into())
}

#[cfg(not(cactus))]
fn complete(_: &str) -> Result<String, String> {
    let _: Option<(&CStr, *mut c_char)> = None;
    Err(NO_ENGINE.into())
}

#[cfg(not(cactus))]
fn reset() {}

#[cfg(not(cactus))]
fn stream_process(_: &[f32], _: &CString, _: &CString) -> Result<String, String> {
    Err(NO_ENGINE.into())
}

#[cfg(not(cactus))]
fn stream_stop() -> Result<String, String> {
    Err(NO_ENGINE.into())
}
