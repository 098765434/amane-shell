//! Privacy — what's using the microphone, the camera or the screen right
//! now, for the bar's indicators (and the Quick Settings microphone slider,
//! which GNOME shows only while something records).
//!
//! The microphone and the camera are read from PipeWire, by following
//! `pw-dump --monitor`: each batch is a JSON array of the objects that
//! changed (an object with `"info": null` is gone). An app records from a
//! `Stream/Input/Audio` node; a camera in use is a v4l2 or libcamera
//! `Video/Source` node that's running. Streams that only watch an output
//! (visualisers, `stream.capture.sink`) and the shell's own aren't counted.
//!
//! Screen sharing is niri's to know (`niri msg --json casts`), polled.

use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

use amane::Service;
use serde_json::Value;

use super::run_now;

const CASTS_EVERY: Duration = Duration::from_secs(2);

#[derive(Default, PartialEq)]
pub struct Privacy {
    microphone: Vec<String>,
    camera: bool,
    screen: bool,
}

impl Service for Privacy {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        std::thread::spawn(watch_casts);
        watch_pipewire();
    }
}

impl Privacy {
    /// The apps recording sound, by name.
    pub fn microphone(&self) -> &[String] {
        &self.microphone
    }

    pub fn recording(&self) -> bool {
        !self.microphone.is_empty()
    }

    pub fn camera(&self) -> bool {
        self.camera
    }

    pub fn screen(&self) -> bool {
        self.screen
    }
}

fn watch_pipewire() {
    let child = Command::new("pw-dump")
        .args(["--monitor", "--no-colors"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        eprintln!("amane-shell: pw-dump isn't available; no microphone or camera indicator");
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        return;
    };

    let own = std::process::id();
    let mut nodes = HashMap::new();
    let mut batch = String::new();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let end = line == "]";
        batch.push_str(&line);
        batch.push('\n');
        if !end {
            continue;
        }

        if let Ok(objects) = serde_json::from_str::<Value>(&batch) {
            apply(&mut nodes, &objects);
            let (microphone, camera) = summary(&nodes, own);
            let privacy = Privacy::read();
            let changed = privacy.microphone != microphone || privacy.camera != camera;
            drop(privacy);
            if changed {
                let mut privacy = Privacy::write();
                privacy.microphone = microphone;
                privacy.camera = camera;
            }
        }
        batch.clear();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn watch_casts() {
    let args = ["msg".to_string(), "--json".into(), "casts".into()];
    loop {
        let screen = run_now("niri", &args).is_some_and(|json| sharing(&json));
        if Privacy::read().screen != screen {
            Privacy::write().screen = screen;
        }
        std::thread::sleep(CASTS_EVERY);
    }
}

/// Whether niri's cast list has one streaming frames.
fn sharing(json: &str) -> bool {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|casts| {
            casts.as_array().map(|casts| {
                casts
                    .iter()
                    .any(|cast| cast["is_active"].as_bool().unwrap_or(true))
            })
        })
        .unwrap_or(false)
}

/// What the indicators need of one PipeWire node.
#[derive(Clone, Debug, Default, PartialEq)]
struct Node {
    class: String,
    app: String,
    pid: Option<u64>,
    api: String,
    state: String,
    // a stream that records what's playing, not a microphone
    monitor: bool,
}

/// Folds one `pw-dump` batch into `nodes`.
fn apply(nodes: &mut HashMap<u64, Node>, objects: &Value) {
    let Some(objects) = objects.as_array() else {
        return;
    };
    for object in objects {
        let Some(id) = object["id"].as_u64() else {
            continue;
        };
        let info = &object["info"];
        if info.is_null() {
            nodes.remove(&id);
            continue;
        }
        if object["type"].as_str() != Some("PipeWire:Interface:Node") {
            continue;
        }

        let node = nodes.entry(id).or_default();
        if let Some(state) = info["state"].as_str() {
            node.state = state.to_string();
        }
        let props = &info["props"];
        if props.is_object() {
            let text = |key: &str| props[key].as_str().unwrap_or_default().to_string();
            node.class = text("media.class");
            node.app = [
                text("application.name"),
                text("node.description"),
                text("node.name"),
            ]
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or_default();
            node.pid = props["application.process.id"].as_u64();
            node.api = text("device.api");
            node.monitor = props["stream.monitor"].as_bool().unwrap_or(false)
                || props["stream.capture.sink"].as_bool().unwrap_or(false);
        }
    }
}

/// The apps recording sound (sorted, once each), and whether a camera runs.
fn summary(nodes: &HashMap<u64, Node>, own_pid: u32) -> (Vec<String>, bool) {
    let microphone: BTreeSet<String> = nodes
        .values()
        .filter(|node| {
            node.class == "Stream/Input/Audio"
                && !node.monitor
                && node.pid != Some(u64::from(own_pid))
        })
        .map(|node| node.app.clone())
        .collect();
    let camera = nodes.values().any(|node| {
        node.class == "Video/Source"
            && matches!(node.api.as_str(), "v4l2" | "libcamera")
            && node.state == "running"
    });
    (microphone.into_iter().collect(), camera)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../tests/fixtures/pipewire/monitor.json");

    /// The fixture's batches, split as `pw-dump --monitor` prints them.
    fn batches() -> Vec<Value> {
        let mut out = Vec::new();
        let mut batch = String::new();
        for line in FIXTURE.lines() {
            batch.push_str(line);
            batch.push('\n');
            if line == "]" {
                out.push(serde_json::from_str(&batch).unwrap());
                batch.clear();
            }
        }
        out
    }

    #[test]
    fn a_recording_app_shows_until_its_stream_goes() {
        let batches = batches();
        let mut nodes = HashMap::new();

        // the start: a microphone, a camera at rest, a visualiser
        apply(&mut nodes, &batches[0]);
        assert_eq!(summary(&nodes, 1), (Vec::new(), false));

        // pw-record starts recording; the shell's own stream doesn't count
        apply(&mut nodes, &batches[1]);
        assert_eq!(
            summary(&nodes, 4242),
            (vec!["pw-record".to_string()], false)
        );

        // the camera runs, and the recording ends
        apply(&mut nodes, &batches[2]);
        assert_eq!(summary(&nodes, 4242), (Vec::new(), true));
    }

    #[test]
    fn a_paused_cast_isnt_sharing() {
        assert!(!sharing("[]"));
        assert!(sharing(r#"[{"stream_id": 1, "is_active": true}]"#));
        assert!(!sharing(r#"[{"stream_id": 1, "is_active": false}]"#));
        assert!(!sharing("not json"));
    }
}
