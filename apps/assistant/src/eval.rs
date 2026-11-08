//! Scoring how well commands are read: each case's text through the rules,
//! then Needle, against the command it should give (or none, for what
//! isn't one). The cases are JSON lines, `{"text": …, "expect": Command |
//! null}`; the snapshot is what the machine is taken to have.
//!
//! `EMAKI_ASSISTANT_NO_RULES=1` leaves the rules out, scoring Needle alone.
//!
//! ```text
//! emaki-assistant eval apps/assistant/eval/cases.jsonl apps/assistant/eval/laptop.json
//! ```

use std::time::Instant;

use emaki_assistant::command::Risk;
use emaki_assistant::{Command, Snapshot, Source, rules, tools};
use serde::Deserialize;

use crate::engine::Engine;

/// Below this, a model's reading isn't offered.
pub const OFFERED: f32 = 0.3;

#[derive(Deserialize)]
struct Case {
    text: String,
    expect: Option<Command>,
}

/// Prints the tools Needle is offered for the snapshot at `path`, so
/// training data is made with exactly what runs.
pub fn tools(path: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let snapshot: Snapshot = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    println!("{}", tools::tools(&snapshot));
    Ok(())
}

/// What a run scored, as `--json` prints it.
#[derive(serde::Serialize)]
pub struct Score {
    pub right: u32,
    pub commands: u32,
    pub by_rules: u32,
    pub missed: u32,
    pub wrong: u32,
    pub negatives: u32,
    pub clean: u32,
    /// Commands the shell asks about before running: wrong ones of these
    /// cost the most.
    pub destructive_wrong: u32,
    pub p50_ms: f64,
    pub p95_ms: f64,
}

pub fn run(mut engine: Engine, cases: &str, snapshot: &str, json: bool) -> Result<(), String> {
    let read = |path: &str| std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"));
    let snapshot: Snapshot =
        serde_json::from_str(&read(snapshot)?).map_err(|e| format!("{snapshot}: {e}"))?;
    let cases: Vec<Case> = read(cases)?
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("//"))
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{line}: {e}")))
        .collect::<Result<_, _>>()?;

    let system = tools::SYSTEM;
    let offered = tools::tools(&snapshot);
    let (mut right, mut wrong, mut missed, mut by_rules) = (0, 0, 0, 0);
    let (mut commands, mut clean, mut negatives, mut destructive_wrong) = (0, 0, 0, 0);
    let mut times = Vec::new();
    // `EMAKI_ASSISTANT_NO_RULES=1` scores Needle alone
    let rules_on = std::env::var_os("EMAKI_ASSISTANT_NO_RULES").is_none();
    for case in &cases {
        let started = Instant::now();
        let ruled = rules_on
            .then(|| rules::read(&case.text, &snapshot))
            .flatten();
        let reading = match ruled {
            Some(command) => Some((command, 1.0, Source::Rules)),
            // as the shell does: some queries aren't worth the model's guess
            None if !rules::worth_asking(&case.text, &snapshot) => None,
            None => {
                let answer = engine.complete(system, &offered, &case.text)?;
                if std::env::var_os("EMAKI_ASSISTANT_DEBUG").is_some() {
                    eprintln!("   {answer}");
                }
                tools::read(&answer, &snapshot)
                    .filter(|reading| reading.confidence >= OFFERED)
                    .map(|reading| (reading.command, reading.confidence, reading.source))
            }
        };
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        let mark = match (&case.expect, &reading) {
            (Some(expect), Some((got, _, source))) if got == expect => {
                commands += 1;
                right += 1;
                if *source == Source::Rules {
                    by_rules += 1;
                }
                "ok "
            }
            (Some(_), Some((got, _, _))) => {
                commands += 1;
                wrong += 1;
                if got.risk() == Risk::Destructive {
                    destructive_wrong += 1;
                }
                "BAD"
            }
            (Some(_), None) => {
                commands += 1;
                missed += 1;
                "-- "
            }
            (None, None) => {
                negatives += 1;
                clean += 1;
                "ok "
            }
            (None, Some((got, _, _))) => {
                negatives += 1;
                wrong += 1;
                if got.risk() == Risk::Destructive {
                    destructive_wrong += 1;
                }
                "BAD"
            }
        };
        let got = reading
            .as_ref()
            .map(|(command, confidence, source)| {
                format!(
                    "{source:?} {confidence:.2} {}",
                    serde_json::to_string(command).unwrap_or_default()
                )
            })
            .unwrap_or_else(|| String::from("nothing"));
        if !json {
            println!("{mark} {:44} {got}", format!("{:?}", case.text));
        }
    }
    times.sort_by(f64::total_cmp);
    let p = |q: f64| {
        times
            .get(((times.len() as f64 * q) as usize).min(times.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0)
    };
    if json {
        let score = Score {
            right,
            commands,
            by_rules,
            missed,
            wrong,
            negatives,
            clean,
            destructive_wrong,
            p50_ms: p(0.5).round(),
            p95_ms: p(0.95).round(),
        };
        println!(
            "{}",
            serde_json::to_string(&score).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    println!(
        "\ncommands right {right}/{commands} ({by_rules} by rules), missed {missed}, wrong {wrong} \
         (counting negatives, {destructive_wrong} destructive); negatives left alone {clean}/{negatives}; p50 {:.0} ms, p95 {:.0} ms",
        p(0.5),
        p(0.95)
    );
    Ok(())
}
