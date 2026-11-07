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
//! Screen sharing is niri's to know: amane's `Casts` follows niri's cast events.

use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use amane::{Casts, Service, niri};
use serde_json::Value;

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
        Casts::subscribe(|casts| {
            let casts = sharing(casts.all());
            if Privacy::read().casts != casts {
                Privacy::write().casts = casts;
            }
        })
        .forever();
        // a cast going before the subscription would otherwise wait for the next change
        let casts = sharing(Casts::read().all());
        if Privacy::read().casts != casts {
            Privacy::write().casts = casts;
        }
        if !crate::services::installed("pw-dump") {
            log::warn!("pw-dump isn't available; no microphone or camera indicator");
            return;
        }
        crate::services::child_stream::supervise("pw-dump", watch_pipewire);
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
    // niri's answer can take a moment, so not on the UI thread
    std::thread::spawn(move || {
        if let Err(err) = niri::stop_cast(session) {
            log::warn!("couldn't stop the screencast: {err}");
        }
    });
}

/// Follows `pw-dump --monitor` until it ends; whether it heard anything.
fn watch_pipewire() -> bool {
    let child = Command::new("pw-dump")
        .args(["--monitor", "--no-colors"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            log::warn!("pw-dump: {error}");
            return false;
        }
    };
    let Some(stdout) = child.stdout.take() else {
        return false;
    };

    let own = std::process::id();
    let mut nodes = HashMap::new();
    let mut batch = Batch::default();
    let mut heard = false;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if let Some(objects) = batch.push(&line) {
            heard = true;
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
    }
    // the stream has ended; the child is likely gone already
    let _ = child.kill();
    let _ = child.wait();
    heard
}

/// `pw-dump --monitor`'s output, a batch at a time: each change is a JSON
/// array whose closing `]` is a line of its own.
#[derive(Default)]
struct Batch(String);

impl Batch {
    /// Adds a line. At a batch's end, its objects; a batch that doesn't
    /// parse is dropped.
    fn push(&mut self, line: &str) -> Option<Value> {
        self.0.push_str(line);
        self.0.push('\n');
        if line != "]" {
            return None;
        }
        let objects = serde_json::from_str(&self.0).ok();
        self.0.clear();
        objects
    }
}

/// The casts in niri's list that stream frames, one per session.
fn sharing(casts: &[niri::Cast]) -> Vec<Cast> {
    let mut found: Vec<Cast> = Vec::new();
    for cast in casts {
        if !cast.is_active || found.iter().any(|known| known.session == cast.session_id) {
            continue;
        }
        let target = match &cast.target {
            niri::CastTarget::Output(name) => CastTarget::Output(name.clone()),
            niri::CastTarget::Window(id) => CastTarget::Window(*id),
            niri::CastTarget::Nothing => CastTarget::Nothing,
        };
        found.push(Cast {
            session: cast.session_id,
            target,
        });
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

    /// The fixture's batches, split as the watcher splits them.
    fn batches() -> Vec<Value> {
        let mut batch = Batch::default();
        let out: Vec<Value> = FIXTURE
            .lines()
            .filter_map(|line| batch.push(line))
            .collect();
        assert_eq!(out.len(), 3, "the fixture holds three batches");
        out
    }

    #[test]
    fn a_broken_batch_is_dropped_and_the_next_one_read() {
        let mut batch = Batch::default();
        assert!(batch.push("[ { \"id\": ").is_none());
        assert!(batch.push("]").is_none());
        assert!(batch.push("[").is_none());
        assert_eq!(batch.push("]"), Some(Value::Array(Vec::new())));
    }

    /// niri's `casts` answer, as amane hands it over.
    fn parsed(json: &str) -> Vec<niri::Cast> {
        let Ok(Value::Array(list)) = serde_json::from_str::<Value>(json) else {
            return Vec::new();
        };
        list.iter()
            .filter_map(|cast| {
                let target = &cast["target"];
                Some(niri::Cast {
                    session_id: cast["session_id"].as_u64()?,
                    stream_id: cast["stream_id"].as_u64().unwrap_or(0),
                    kind: String::from("PipeWire"),
                    target: if let Some(name) = target["Output"]["name"].as_str() {
                        niri::CastTarget::Output(name.to_string())
                    } else if let Some(id) = target["Window"]["id"].as_u64() {
                        niri::CastTarget::Window(id)
                    } else {
                        niri::CastTarget::Nothing
                    },
                    is_active: cast["is_active"].as_bool().unwrap_or(true),
                    pid: None,
                })
            })
            .collect()
    }

    #[test]
    fn recorded_casts_are_one_per_active_session() {
        let casts = sharing(&parsed(include_str!(
            "../../../tests/fixtures/niri/casts.json"
        )));
        assert_eq!(
            casts,
            [
                Cast {
                    session: 10,
                    target: CastTarget::Output(String::from("eDP-1"))
                },
                Cast {
                    session: 11,
                    target: CastTarget::Window(5)
                },
                Cast {
                    session: 13,
                    target: CastTarget::Nothing
                },
            ]
        );
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
        assert!(sharing(&parsed("[]")).is_empty());
        assert!(sharing(&parsed("not json")).is_empty());
        assert!(sharing(&parsed(r#"[{"session_id": 3, "is_active": false}]"#)).is_empty());
        let casts = sharing(&parsed(
            r#"[{"stream_id": 1, "session_id": 3, "is_active": true,
                 "target": {"Output": {"name": "eDP-1"}}},
                {"stream_id": 2, "session_id": 3, "is_active": true,
                 "target": {"Window": {"id": 9}}},
                {"stream_id": 4, "session_id": 5, "is_active": true,
                 "target": {"Window": {"id": 9}}}]"#,
        ));
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
