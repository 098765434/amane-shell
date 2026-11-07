//! `amane-assistant`: the launcher's assistant's engine, a user service beside
//! the shell. It holds Cactus (Needle 3 for commands, Whistle for speech),
//! listens to the microphone while asked to, and answers the shell over a
//! Unix socket ([`amane_assistant::protocol`]). It never runs a command
//! itself, and never touches the network: Cactus reads everything here.
//!
//! ```text
//! amane-assistant                  serve the shell (the default)
//! amane-assistant eval [--json] CASES SNAP
//!                                  read each case's text, and score it
//! amane-assistant tools SNAP       the tools Needle is offered for SNAP, as JSON
//! ```
//!
//! Its models come from `$AMANE_ASSISTANT_MODELS`, else
//! `$XDG_DATA_HOME/amane-assistant/models`: `needle3.cact`, `whistle.cact`.

mod audio;
mod engine;
mod eval;
mod server;

use std::path::PathBuf;

fn main() {
    amane_log::init("amane-assistant");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let engine = engine::Engine::new(models_dir(), cache_dir());
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["serve"] => server::serve(engine),
        ["eval", cases, snapshot] => eval::run(engine, cases, snapshot, false),
        ["eval", "--json", cases, snapshot] => eval::run(engine, cases, snapshot, true),
        ["tools", snapshot] => eval::tools(snapshot),
        ["--version"] => {
            println!(
                "amane-assistant {} ({})",
                env!("CARGO_PKG_VERSION"),
                engine::ENGINE
            );
            Ok(())
        }
        _ => Err(String::from(
            "usage: amane-assistant [serve] | eval [--json] CASES.jsonl SNAPSHOT.json | tools SNAPSHOT.json",
        )),
    };
    if let Err(error) = result {
        eprintln!("amane-assistant: {error}");
        std::process::exit(1);
    }
}

fn dir(var: &str, home_default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(home_default)))
        .unwrap_or_else(std::env::temp_dir)
}

fn models_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("AMANE_ASSISTANT_MODELS") {
        return PathBuf::from(dir);
    }
    dir("XDG_DATA_HOME", ".local/share").join("amane-assistant/models")
}

fn cache_dir() -> PathBuf {
    let dir = dir("XDG_CACHE_HOME", ".cache").join("amane-assistant");
    let _ = std::fs::create_dir_all(&dir);
    dir
}
