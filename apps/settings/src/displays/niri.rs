//! niri's outputs, over its IPC socket (`$NIRI_SOCKET`): what each one runs
//! at, and the changes niri makes at once but keeps only until its config
//! is read again. One JSON request per connection, one reply line back.
//! Only the fields the page uses are mirrored from `niri-ipc`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Output {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    /// Width and height in millimetres.
    pub physical_size: Option<(u32, u32)>,
    pub modes: Vec<Mode>,
    /// Into `modes`; none while the output is off.
    pub current_mode: Option<usize>,
    pub is_custom_mode: bool,
    pub vrr_supported: bool,
    pub vrr_enabled: bool,
    /// HDR, from a niri that has it; an older one leaves these out.
    #[serde(default)]
    pub hdr_supported: bool,
    #[serde(default)]
    pub hdr_enabled: bool,
    /// SDR white in HDR mode, in nits.
    #[serde(default)]
    pub sdr_brightness: Option<f64>,
    #[serde(default)]
    pub peak_luminance: Option<f64>,
    pub logical: Option<Logical>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Mode {
    pub width: u16,
    pub height: u16,
    /// In millihertz.
    pub refresh_rate: u32,
    pub is_preferred: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Logical {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub transform: Transform,
}

/// Counter-clockwise, as niri turns them.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Transform {
    #[default]
    Normal,
    #[serde(rename = "90")]
    _90,
    #[serde(rename = "180")]
    _180,
    #[serde(rename = "270")]
    _270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl Transform {
    pub const ALL: [Transform; 8] = [
        Transform::Normal,
        Transform::_90,
        Transform::_180,
        Transform::_270,
        Transform::Flipped,
        Transform::Flipped90,
        Transform::Flipped180,
        Transform::Flipped270,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Transform::Normal => "Standard",
            Transform::_90 => "90°",
            Transform::_180 => "180°",
            Transform::_270 => "270°",
            Transform::Flipped => "Flipped",
            Transform::Flipped90 => "Flipped, 90°",
            Transform::Flipped180 => "Flipped, 180°",
            Transform::Flipped270 => "Flipped, 270°",
        }
    }

    /// The name in IPC requests and the config (`flipped-90`).
    pub fn config(self) -> &'static str {
        match self {
            Transform::Normal => "normal",
            Transform::_90 => "90",
            Transform::_180 => "180",
            Transform::_270 => "270",
            Transform::Flipped => "flipped",
            Transform::Flipped90 => "flipped-90",
            Transform::Flipped180 => "flipped-180",
            Transform::Flipped270 => "flipped-270",
        }
    }

    fn ipc(self) -> &'static str {
        match self {
            Transform::Normal => "Normal",
            Transform::_90 => "90",
            Transform::_180 => "180",
            Transform::_270 => "270",
            Transform::Flipped => "Flipped",
            Transform::Flipped90 => "Flipped90",
            Transform::Flipped180 => "Flipped180",
            Transform::Flipped270 => "Flipped270",
        }
    }

    /// Turned a quarter, so its width and height swap.
    pub fn sideways(self) -> bool {
        matches!(
            self,
            Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
        )
    }
}

impl Output {
    pub fn mode(&self) -> Option<Mode> {
        self.current_mode
            .and_then(|index| self.modes.get(index).copied())
    }

    pub fn on(&self) -> bool {
        self.current_mode.is_some()
    }

    /// The name niri matches a config block by when the connector could
    /// change: `make model serial`.
    pub fn identity(&self) -> String {
        format!(
            "{} {} {}",
            self.make,
            self.model,
            self.serial.as_deref().unwrap_or("Unknown")
        )
    }

    /// A built-in panel: a laptop's, on an embedded connector.
    pub fn built_in(&self) -> bool {
        ["eDP", "LVDS", "DSI"]
            .iter()
            .any(|kind| self.name.starts_with(kind))
    }

    /// The diagonal in inches, when the size is known.
    pub fn inches(&self) -> Option<f32> {
        let (w, h) = self.physical_size.filter(|(w, h)| *w > 0 && *h > 0)?;
        Some((w as f32).hypot(h as f32) / 25.4)
    }
}

impl Mode {
    pub fn hz(self) -> f64 {
        f64::from(self.refresh_rate) / 1000.0
    }
}

/// A change niri makes at once.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    On,
    Off,
    Mode(Mode),
    /// A mode the monitor doesn't list: width, height, refresh in Hz.
    CustomMode(u16, u16, f64),
    Scale(f64),
    Transform(Transform),
    Position(i32, i32),
    Vrr {
        on: bool,
        on_demand: bool,
    },
    Hdr(bool),
    /// SDR white in HDR mode, in nits.
    SdrBrightness(f64),
}

impl Change {
    fn action(&self) -> Value {
        match self {
            Change::On => json!("On"),
            Change::Off => json!("Off"),
            Change::Mode(mode) => json!({"Mode": {"mode": {"Specific": {
                "width": mode.width,
                "height": mode.height,
                "refresh": mode.hz(),
            }}}}),
            Change::CustomMode(width, height, hz) => json!({"CustomMode": {"mode": {
                "width": width,
                "height": height,
                "refresh": hz,
            }}}),
            Change::Scale(scale) => json!({"Scale": {"scale": {"Specific": scale}}}),
            Change::Transform(transform) => {
                json!({"Transform": {"transform": transform.ipc()}})
            }
            Change::Position(x, y) => {
                json!({"Position": {"position": {"Specific": {"x": x, "y": y}}}})
            }
            Change::Hdr(on) => json!({"Hdr": {"hdr": on}}),
            Change::SdrBrightness(nits) => json!({"SdrBrightness": {"nits": nits}}),
            Change::Vrr { on, on_demand } => {
                json!({"Vrr": {"vrr": {"vrr": on, "on_demand": on_demand}}})
            }
        }
    }
}

/// Every output niri knows, by connector; `None` when niri isn't answering.
pub fn outputs() -> Option<BTreeMap<String, Output>> {
    let reply = request(&json!("Outputs")).ok()?;
    serde_json::from_value(reply.get("Outputs")?.clone()).ok()
}

/// Makes `change` to the output named `output` until the config is read
/// again.
pub fn apply(output: &str, change: &Change) -> Result<(), String> {
    request(&json!({"Output": {"output": output, "action": change.action()}})).map(|_| ())
}

fn request(request: &Value) -> Result<Value, String> {
    let path = std::env::var("NIRI_SOCKET").map_err(|_| String::from("niri isn't running"))?;
    let stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;

    let mut line = request.to_string();
    line.push('\n');
    (&stream)
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_str(&reply).map_err(|e| e.to_string())?;
    match reply {
        Value::Object(mut map) if map.contains_key("Ok") => {
            Ok(map.remove("Ok").unwrap_or_default())
        }
        Value::Object(map) => Err(map
            .get("Err")
            .and_then(Value::as_str)
            .unwrap_or("niri refused")
            .to_string()),
        _ => Err(String::from("niri answered oddly")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPLY: &str = r#"{"eDP-1":{"name":"eDP-1","make":"Lenovo Group Limited","model":"ATNA40HQ10-0 ","serial":null,"physical_size":[300,190],"modes":[{"width":2880,"height":1800,"refresh_rate":60000,"is_preferred":true},{"width":2880,"height":1800,"refresh_rate":120000,"is_preferred":false}],"current_mode":1,"is_custom_mode":false,"vrr_supported":true,"vrr_enabled":false,"logical":{"x":0,"y":0,"width":1645,"height":1028,"scale":1.75,"transform":"Normal"}}}"#;

    #[test]
    fn reads_niris_outputs() {
        let outputs: BTreeMap<String, Output> = serde_json::from_str(REPLY).unwrap();
        let panel = &outputs["eDP-1"];
        assert!(panel.built_in() && panel.on());
        assert_eq!(panel.mode().unwrap().hz(), 120.0);
        assert_eq!(panel.logical.unwrap().transform, Transform::Normal);
        assert_eq!(panel.inches().unwrap().round(), 14.0);
        assert_eq!(
            panel.identity(),
            "Lenovo Group Limited ATNA40HQ10-0  Unknown"
        );
    }

    #[test]
    fn changes_are_niri_ipc_actions() {
        assert_eq!(
            Change::Scale(1.5).action(),
            json!({"Scale": {"scale": {"Specific": 1.5}}})
        );
        assert_eq!(
            Change::Transform(Transform::Flipped90).action(),
            json!({"Transform": {"transform": "Flipped90"}})
        );
        assert_eq!(
            Change::Vrr {
                on: true,
                on_demand: true
            }
            .action(),
            json!({"Vrr": {"vrr": {"vrr": true, "on_demand": true}}})
        );
    }
}
