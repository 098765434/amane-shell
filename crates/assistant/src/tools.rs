//! What Needle 3 is offered, and what its answer is read as. Needle picks
//! one of a few small tools (it's best with five or fewer, plainly named),
//! and the tools' arguments are what's on this machine, as `enum`s, so an
//! answer names a real switch or app. An answer is read back only when each
//! argument matches something here; anything else is dropped.

use serde_json::{Value, json};

use crate::command::{Command, Level, Media, State, Target};
use crate::snapshot::Snapshot;
use crate::{Reading, Source};

/// How many apps the open tool names at most, the most used first: past
/// this the prompt grows over 512 tokens, which costs Needle (and training
/// it) about twice the time. The rules still know every app.
pub const MAX_APPS: usize = 24;

/// How sure a call is taken to be when the model gives no score (one
/// fine-tuned here): offered, but as a suggestion, and never run unasked.
pub const UNSCORED: f32 = 0.6;

/// The session facts Needle is told. None: no command here needs the day
/// or the place, and the tokens they'd take slow every answer (and
/// training, past 512 a prompt). The fine-tune is trained without them.
pub const SYSTEM: &str = "";

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
            }
        }
    })
}

/// The tools for `snapshot`, as the JSON array Cactus takes.
pub fn tools(snapshot: &Snapshot) -> String {
    let mut tools = Vec::new();

    if !snapshot.switches.is_empty() {
        let names: Vec<String> = snapshot.switches.iter().map(|s| s.name()).collect();
        tools.push(tool(
            "set_switch",
            "Turn a setting on or off. Night light: warmer screen colors. Dark style: dark theme. \
             Focus: do not disturb.",
            json!({
                "setting": {"type": "string", "enum": names},
                "state": {"type": "string", "enum": ["on", "off", "toggle"]},
            }),
            &["setting", "state"],
        ));
    }

    let mut levels = vec!["volume"];
    if snapshot.brightness {
        levels.push("brightness");
    }
    // two narrow tools, every argument required: one optional argument
    // each, Cactus drops whichever the request doesn't name ("crank up the
    // volume" lost its direction); increase and decrease are words its
    // repair knows the synonyms of
    tools.push(tool(
        "change_level",
        "Make the volume louder or quieter, or the screen brighter or dimmer",
        json!({
            "what": {"type": "string", "enum": levels},
            "direction": {"type": "string", "enum": ["increase", "decrease"]},
        }),
        &["what", "direction"],
    ));
    tools.push(tool(
        "set_level",
        "Set the volume or the screen brightness to a percent",
        json!({
            "what": {"type": "string", "enum": levels},
            "percent": {"type": "integer", "minimum": 0, "maximum": 100},
        }),
        &["what", "percent"],
    ));

    if !snapshot.apps.is_empty() {
        let names: Vec<&str> = snapshot
            .apps
            .iter()
            .take(MAX_APPS)
            .map(|app| app.name.as_str())
            .collect();
        tools.push(tool(
            "open_app",
            "Open, launch or start an application",
            json!({"name": {"type": "string", "enum": names}}),
            &["name"],
        ));
    }

    tools.push(tool(
        "media",
        "Play, pause, or skip to the next or previous track",
        json!({"command": {"type": "string", "enum": Media::ALL.map(Media::name)}}),
        &["command"],
    ));

    // no session tool: the session is the rules' alone (a model's guess at
    // it, "I can't hear anything" as lock, costs too much when it's wrong),
    // and Needle is best with five tools or fewer

    Value::Array(tools).to_string()
}

/// Needle's answer for `snapshot`'s tools, read: the call it made, as a
/// command, if each argument names something here. Calls it held back as
/// unsure aren't read.
pub fn read(answer: &Value, snapshot: &Snapshot) -> Option<Reading> {
    let call = answer.get("function_calls")?.as_array()?.first()?;
    let name = call.get("name")?.as_str()?;
    let arguments = call.get("arguments")?;
    let command = command(name, arguments, snapshot)?;
    // a model fine-tuned here has no confidence head and says null: its
    // calls are offered, as suggestions, on the strength of its eval
    let mut confidence = answer
        .get("confidence")
        .and_then(Value::as_f64)
        .map_or(UNSCORED, |confidence| confidence as f32);
    let ungrounded = answer
        .pointer("/validation/ungrounded")
        .and_then(Value::as_array)
        .is_some_and(|list| !list.is_empty());
    if ungrounded {
        confidence *= 0.5;
    }
    Some(Reading {
        command,
        confidence: confidence.clamp(0.0, 1.0),
        source: Source::Model,
    })
}

fn text<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key)?.as_str()
}

fn command(name: &str, arguments: &Value, snapshot: &Snapshot) -> Option<Command> {
    match name {
        "set_switch" => {
            let switch = snapshot.switch(text(arguments, "setting")?)?;
            let state = text(arguments, "state")
                .and_then(State::named)
                .unwrap_or(State::Toggle);
            let state = if switch.on.is_none() {
                State::Toggle
            } else {
                state
            };
            Some(Command::Switch {
                control: switch.control.clone(),
                state,
            })
        }
        "set_level" | "change_level" => {
            let level = match text(arguments, "what")? {
                "volume" => Level::Volume,
                "brightness" if snapshot.brightness => Level::Brightness,
                _ => return None,
            };
            let percent = arguments
                .get("percent")
                .and_then(Value::as_u64)
                .filter(|percent| *percent <= 100);
            let direction = text(arguments, "direction").or_else(|| text(arguments, "change"));
            let target = match (percent, direction) {
                (Some(percent), _) => Target::Percent(percent as u8),
                (None, Some("increase" | "up")) => Target::Up,
                (None, Some("decrease" | "down")) => Target::Down,
                _ => return None,
            };
            Some(Command::Level { level, target })
        }
        "open_app" => {
            let app = snapshot.app(text(arguments, "name")?)?;
            Some(Command::Open {
                app: app.id.clone(),
            })
        }
        "media" => Some(Command::Media {
            media: Media::named(text(arguments, "command")?)?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::sample;

    #[test]
    fn the_tools_name_whats_here() {
        let tools: Value = serde_json::from_str(&tools(&sample())).unwrap();
        let tools = tools.as_array().unwrap();
        assert!(tools.len() <= 5, "Needle is best with five or fewer");
        let switch = &tools[0]["function"];
        assert_eq!(switch["name"], "set_switch");
        let settings = switch["parameters"]["properties"]["setting"]["enum"]
            .as_array()
            .unwrap();
        assert!(settings.contains(&json!("wifi")));
        assert!(settings.contains(&json!("night light")));
    }

    #[test]
    fn a_call_is_read_when_its_arguments_are_real() {
        let snapshot = sample();
        let answer = json!({
            "function_calls": [{"name": "set_switch", "arguments": {"setting": "night light", "state": "on"}}],
            "confidence": 0.8,
            "validation": {"ungrounded": []},
        });
        let reading = read(&answer, &snapshot).unwrap();
        assert_eq!(
            reading.command,
            Command::Switch {
                control: "night-light".into(),
                state: State::On
            }
        );
        assert!((reading.confidence - 0.8).abs() < 1e-6);

        let made_up = json!({
            "function_calls": [{"name": "open_app", "arguments": {"name": "Photoshop"}}],
            "confidence": 0.9,
        });
        assert_eq!(read(&made_up, &snapshot), None);
    }

    #[test]
    fn held_back_calls_arent_read() {
        let answer = json!({
            "function_calls": [],
            "suppressed_calls": [{"name": "media", "arguments": {"command": "play"}}],
            "confidence": 0.1,
        });
        assert_eq!(read(&answer, &sample()), None);
    }

    #[test]
    fn a_call_without_a_score_is_a_suggestion() {
        let answer = json!({
            "function_calls": [{"name": "media", "arguments": {"command": "next"}}],
            "confidence": null,
        });
        let reading = read(&answer, &sample()).unwrap();
        assert!((reading.confidence - UNSCORED).abs() < 1e-6);
    }

    #[test]
    fn ungrounded_arguments_halve_the_confidence() {
        let answer = json!({
            "function_calls": [{"name": "set_level", "arguments": {"what": "volume", "percent": 40}}],
            "confidence": 0.8,
            "validation": {"ungrounded": ["percent"]},
        });
        let reading = read(&answer, &sample()).unwrap();
        assert!((reading.confidence - 0.4).abs() < 1e-6);
    }
}
