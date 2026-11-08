//! `emaki-assistant`: the launcher's assistant's engine, a user service beside
//! the shell. It holds Cactus (Needle 3 for commands, Whistle for speech),
//! listens to the microphone while asked to, and answers the shell over a
//! Unix socket ([`emaki_assistant::protocol`]). It never runs a command
//! itself, and never touches the network: Cactus reads everything here.
//!
//! ```text
//! emaki-assistant                  serve the shell (the default)
//! emaki-assistant eval [--json] CASES SNAP
//!                                  read each case's text, and score it
//! emaki-assistant tools SNAP       the tools Needle is offered for SNAP, as JSON
//! ```
//!
//! Its models come from `$EMAKI_ASSISTANT_MODELS`, else
//! `$XDG_DATA_HOME/emaki-assistant/models`: `needle3.cact`, `whistle.cact`.

mod audio;
mod engine;
mod eval;
mod server;

use std::path::PathBuf;

fn main() {
    emaki_log::init(emaki_base::app_ids::ASSISTANT);
    emaki_base::migrate::from_amane();
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
                "emaki-assistant {} ({})",
                env!("CARGO_PKG_VERSION"),
                engine::ENGINE
            );
            Ok(())
        }
        _ => Err(String::from(
            "usage: emaki-assistant [serve] | eval [--json] CASES.jsonl SNAPSHOT.json | tools SNAPSHOT.json",
        )),
    };
    if let Err(error) = result {
        log::error!("{error}");
        std::process::exit(1);
    }
}

fn models_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("EMAKI_ASSISTANT_MODELS") {
        return PathBuf::from(dir);
    }
    emaki_base::paths::data()
        .join(emaki_base::app_ids::ASSISTANT)
        .join("models")
}

fn cache_dir() -> PathBuf {
    let dir = emaki_base::paths::cache().join(emaki_base::app_ids::ASSISTANT);
    // binding the socket below says so if the folder couldn't be made
    let _ = std::fs::create_dir_all(&dir);
    dir
}
