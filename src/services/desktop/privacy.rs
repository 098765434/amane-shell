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

use crate::services::run_now;

const CASTS_EVERY: Duration = Duration::from_secs(2);

#[derive(Default, PartialEq)]
pub struct Privacy {
    microphone: Vec<User>,
    camera: bool,
    camera_users: Vec<User>,
    casts: Vec<Cast>,
}

/// An app using the microphone or the camera.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct User {
    pub name: String,
    /// Its process, to find its window by.
    pub pid: Option<u64>,
}

/// A screencast niri is making.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cast {
    /// What `niri msg action stop-cast --session-id` stops.
    pub session: u64,
    pub target: CastTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CastTarget {
    Output(String),
    Window(u64),
    Nothing,
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
    /// The apps recording sound.
    pub fn microphone(&self) -> &[User] {
        &self.microphone
    }

    pub fn recording(&self) -> bool {
        !self.microphone.is_empty()
    }

    pub fn camera(&self) -> bool {
        self.camera
    }

    /// The apps watching the camera, as far as PipeWire knows.
    pub fn camera_users(&self) -> &[User] {
        &self.camera_users
    }

    pub fn screen(&self) -> bool {
        !self.casts.is_empty()
    }

    /// The screencasts streaming now.
    pub fn casts(&self) -> &[Cast] {
        &self.casts
    }
}

/// Stops the screencast `session`, as the bar's Stop Sharing does.
pub fn stop_cast(session: u64) {
    let session = session.to_string();
    amane::spawn(&format!("niri msg action stop-cast --session-id {session}"));
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
            let (microphone, camera, camera_users) = summary(&nodes, own);
            let privacy = Privacy::read();
            let changed = privacy.microphone != microphone
                || privacy.camera != camera
                || privacy.camera_users != camera_users;
            drop(privacy);
            if changed {
                let mut privacy = Privacy::write();
                privacy.microphone = microphone;
                privacy.camera = camera;
                privacy.camera_users = camera_users;
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
        let casts = run_now("niri", &args)
            .map(|json| sharing(&json))
            .unwrap_or_default();
        if Privacy::read().casts != casts {
            Privacy::write().casts = casts;
        }
        std::thread::sleep(CASTS_EVERY);
    }
}

/// The casts in niri's list that stream frames, one per session.
fn sharing(json: &str) -> Vec<Cast> {
    let Ok(casts) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let mut found: Vec<Cast> = Vec::new();
    for cast in casts.as_array().into_iter().flatten() {
        if !cast["is_active"].as_bool().unwrap_or(true) {
            continue;
        }
        let Some(session) = cast["session_id"].as_u64() else {
            continue;
        };
        if found.iter().any(|known| known.session == session) {
            continue;
        }
        let target = &cast["target"];
        let target = if let Some(name) = target["Output"]["name"].as_str() {
            CastTarget::Output(name.to_string())
        } else if let Some(id) = target["Window"]["id"].as_u64() {
            CastTarget::Window(id)
        } else {
            CastTarget::Nothing
        };
        found.push(Cast { session, target });
    }
    found
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

/// The apps recording sound (sorted, once each), whether a camera runs, and
/// the apps reading video, which while it runs are the camera's.
fn summary(nodes: &HashMap<u64, Node>, own_pid: u32) -> (Vec<User>, bool, Vec<User>) {
    let users = |class: &str| -> Vec<User> {
        let found: BTreeSet<User> = nodes
            .values()
            .filter(|node| {
                node.class == class && !node.monitor && node.pid != Some(u64::from(own_pid))
            })
            .map(|node| User {
                name: node.app.clone(),
                pid: node.pid,
            })
            .collect();
        found.into_iter().collect()
    };
    let microphone = users("Stream/Input/Audio");
    let camera = nodes.values().any(|node| {
        node.class == "Video/Source"
            && matches!(node.api.as_str(), "v4l2" | "libcamera")
            && node.state == "running"
    });
    let camera_users = if camera {
        users("Stream/Input/Video")
    } else {
        Vec::new()
    };
    (microphone, camera, camera_users)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/pipewire/monitor.json");

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
        assert_eq!(summary(&nodes, 1), (Vec::new(), false, Vec::new()));

        // pw-record starts recording; the shell's own stream doesn't count
        apply(&mut nodes, &batches[1]);
        let (microphone, camera, _) = summary(&nodes, 4242);
        let names: Vec<&str> = microphone.iter().map(|user| user.name.as_str()).collect();
        assert_eq!((names, camera), (vec!["pw-record"], false));

        // the camera runs, and the recording ends
        apply(&mut nodes, &batches[2]);
        let (microphone, camera, _) = summary(&nodes, 4242);
        assert_eq!((microphone, camera), (Vec::new(), true));
    }

    #[test]
    fn a_paused_cast_isnt_sharing() {
        assert!(sharing("[]").is_empty());
        assert!(sharing("not json").is_empty());
        assert!(sharing(r#"[{"session_id": 3, "is_active": false}]"#).is_empty());
        let casts = sharing(
            r#"[{"stream_id": 1, "session_id": 3, "is_active": true,
                 "target": {"Output": {"name": "eDP-1"}}},
                {"stream_id": 2, "session_id": 3, "is_active": true,
                 "target": {"Window": {"id": 9}}},
                {"stream_id": 4, "session_id": 5, "is_active": true,
                 "target": {"Window": {"id": 9}}}]"#,
        );
        assert_eq!(
            casts,
            [
                Cast {
                    session: 3,
                    target: CastTarget::Output(String::from("eDP-1"))
                },
                Cast {
                    session: 5,
                    target: CastTarget::Window(9)
                },
            ]
        );
    }
}
