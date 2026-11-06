//! What Needle 3 is offered, and what its answer is read as. Needle picks
//! one of a few small tools (it's best with five or fewer, plainly named),
//! and the tools' arguments are what's on this machine, as `enum`s, so an
//! answer names a real switch or app. An answer is read back only when each
//! argument matches something here; anything else is dropped.

use serde_json::{Value, json};

use crate::command::{Command, Level, Media, Power, State, Target};
use crate::snapshot::Snapshot;
use crate::{Reading, Source};

/// How many apps the open tool names at most.
pub const MAX_APPS: usize = 40;

/// The session facts Needle is told: the day, the language, the device.
pub fn system(date: &str, locale: &str) -> String {
    format!("date: {date}; locale: {locale}; device: laptop")
}

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
        let mut description = String::from(
            "Turn a system setting on or off. Night light makes the screen warmer at night; \
             dark style is the dark color theme; focus is do not disturb, silencing notifications; \
             keep awake stops the screen sleeping.",
        );
        if names.iter().any(|name| name == "airplane mode") {
            description.push_str(" Airplane mode turns every radio off.");
        }
        tools.push(tool(
            "set_switch",
            &description,
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
    tools.push(tool(
        "set_level",
        "Set or change the speaker volume or the screen brightness. Louder or quieter is volume; \
         brighter or dimmer is brightness.",
        json!({
            "what": {"type": "string", "enum": levels},
            "percent": {"type": "integer", "description": "the level to set, 0 to 100"},
            "change": {"type": "string", "enum": ["up", "down"]},
        }),
        &["what"],
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
            "Open or launch an application",
            json!({"name": {"type": "string", "enum": names}}),
            &["name"],
        ));
    }

    tools.push(tool(
        "media",
        "Control the music or video playing: play, pause, skip to the next track or go back to \
         the previous one",
        json!({"command": {"type": "string", "enum": Media::ALL.map(Media::name)}}),
        &["command"],
    ));

    let powers: Vec<&str> = snapshot.powers.iter().map(|power| power.name()).collect();
    if !powers.is_empty() {
        tools.push(tool(
            "session",
            "Lock the screen, sleep, log out, restart or power off the computer",
            json!({"action": {"type": "string", "enum": powers}}),
            &["action"],
        ));
    }

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
    let mut confidence = answer
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
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
            let state = if switch.on.is_none() { State::Toggle } else { state };
            Some(Command::Switch {
                control: switch.control.clone(),
                state,
            })
        }
        "set_level" => {
            let level = match text(arguments, "what")? {
                "volume" => Level::Volume,
                "brightness" if snapshot.brightness => Level::Brightness,
                _ => return None,
            };
            let percent = arguments
                .get("percent")
                .and_then(Value::as_u64)
                .filter(|percent| *percent <= 100);
            let target = match (percent, text(arguments, "change")) {
                (Some(percent), _) => Target::Percent(percent as u8),
                (None, Some("up")) => Target::Up,
                (None, Some("down")) => Target::Down,
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
        "session" => {
            let power = Power::named(text(arguments, "action")?)?;
            snapshot
                .powers
                .contains(&power)
                .then_some(Command::Power { power })
        }
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
