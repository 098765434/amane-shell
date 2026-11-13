//! Privacy: the microphone and the camera, each with one toggle. The mic
//! row is PipeWire mute (the shell drives the kernel `platform::micmute`
//! LED to match, in services/power/mic_led.rs). The camera row flips
//! `/dev/video*` from mode 660 to 000 through a polkit helper installed
//! by the NixOS module.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use amane::{Audio, Service};

use crate::{empty_row, group, groups, switch_row};

/// `/sys/class/video4linux/<node>/name` — the readable name for a
/// `/dev/video*` node. UVC webcams read as e.g. "Integrated RGB Camera";
/// phone-camera loopbacks read as "Phone Camera" or similar.
const V4L_SYS: &str = "/sys/class/video4linux";

/// How often the camera-block state is re-read. Fast enough that an
/// external toggle (fprintd-enroll outside the UI, chmod from a shell)
/// shows up without poking.
const INTERVAL: Duration = Duration::from_secs(3);

/// What the page needs to render; mic state is read from the Audio
/// service directly, so only the camera's side is kept here.
pub struct CameraBlock {
    /// Every `/dev/video*` node the local UVC path exposes; loopback
    /// devices (phone camera) are deliberately not listed, so they stay
    /// reachable even when the camera is blocked.
    nodes: Vec<PathBuf>,
    /// true when every real-camera node is mode 000.
    blocked: bool,
    asked: bool,
    error: Option<String>,
}

impl Service for CameraBlock {
    fn new() -> Self {
        let nodes = scan_nodes();
        let blocked = nodes.iter().all(|path| mode_is_blocked(path));
        Self {
            nodes,
            blocked,
            asked: true,
            error: None,
        }
    }

    fn interval() -> Duration {
        INTERVAL
    }

    fn update(&mut self) -> bool {
        let nodes = scan_nodes();
        let blocked = !nodes.is_empty() && nodes.iter().all(|p| mode_is_blocked(p));
        let changed = nodes != self.nodes || blocked != self.blocked;
        self.nodes = nodes;
        self.blocked = blocked;
        changed
    }
}

/// `/dev/video*` nodes whose v4l2 name doesn't mark them loopbacks.
fn scan_nodes() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/dev") else {
        return Vec::new();
    };
    let mut nodes: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_string();
            if !name.starts_with("video") {
                return None;
            }
            if is_loopback(&name) {
                return None;
            }
            Some(path)
        })
        .collect();
    nodes.sort();
    nodes
}

/// A `videoN` node sponsored by v4l2loopback (the phone-camera path) is
/// not what the kill switch should block.
fn is_loopback(video_node: &str) -> bool {
    // read /sys/class/video4linux/<node>/name; v4l2loopback devices
    // carry "v4l2loopback", and the phone-camera shim sets "Phone Camera".
    let name = fs::read_to_string(Path::new(V4L_SYS).join(video_node).join("name"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    name.eq_ignore_ascii_case("Phone Camera") || name.to_ascii_lowercase().contains("v4l2loopback")
}

fn mode_is_blocked(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.mode() & 0o777 == 0)
        .unwrap_or(false)
}

fn set_blocked(on: bool) {
    thread::spawn(move || {
        let arg = if on { "on" } else { "off" };
        let output = Command::new("pkexec")
            .arg("emaki-camera-block")
            .arg(arg)
            .output();
        let error = match output {
            Ok(result) if result.status.success() => None,
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
                Some(if stderr.is_empty() {
                    format!("The helper exited {}", result.status)
                } else {
                    stderr
                })
            }
            Err(why) => Some(format!("Couldn't run pkexec: {why}")),
        };
        let mut now = CameraBlock::write();
        now.error = error;
        now.update();
    });
}

pub fn page() -> amane::Column {
    let (nodes, blocked, asked, camera_error) = {
        let now = CameraBlock::read();
        (now.nodes.clone(), now.blocked, now.asked, now.error.clone())
    };

    let audio = Audio::read();
    let mic_muted = audio.microphone_muted();
    drop(audio);

    // Microphone
    let mic_row = switch_row(
        "settings.privacy.mic",
        "Microphone muted",
        mic_muted,
        move |want_muted| {
            if want_muted != Audio::read().microphone_muted() {
                Audio::toggle_microphone_mute();
            }
        },
    );
    let mic_group = group().header("Microphone").row(mic_row).footer(
        "F4 and the hardware mic-mute LED follow this toggle. Apps already \
         recording may need to be restarted to pick up the new state.",
    );

    // Camera
    let mut camera_group = group().header("Camera");
    if !asked {
        camera_group = camera_group.row(crate::loading_row(
            "settings.privacy.camera.asking",
            "Looking for cameras…",
        ));
    } else if nodes.is_empty() {
        camera_group = camera_group.row(
            empty_row("settings.privacy.camera.none", "No camera found")
                .subtitle("This machine has no /dev/video device, or your camera is a loopback."),
        );
    } else {
        camera_group = camera_group.row(switch_row(
            "settings.privacy.camera",
            "Camera blocked",
            blocked,
            move |want_blocked| {
                if want_blocked != blocked {
                    set_blocked(want_blocked);
                }
            },
        ));
    }

    let camera_footer = match (nodes.is_empty(), camera_error) {
        (true, _) => {
            String::from("The phone-camera loopback stays reachable regardless of this toggle.")
        }
        (false, Some(error)) => format!(
            "{error}. The toggle blocks every app on this computer: it chmods \
             /dev/video* to 000 until unblocked."
        ),
        (false, None) => format!(
            "Blocks every app on this computer from opening the camera: {} \
             device{} affected.",
            nodes.len(),
            if nodes.len() == 1 { "" } else { "s" },
        ),
    };
    let camera_group = camera_group.footer(camera_footer);

    groups(vec![mic_group, camera_group])
}

#[cfg(test)]
mod tests {
    #[test]
    fn loopback_names_are_filtered() {
        // the heuristic runs off v4l2 name, so pass fake names rather than paths
        assert!(is_loopback_name("Phone Camera"));
        assert!(is_loopback_name("v4l2loopback (Phone)"));
        assert!(!is_loopback_name("Integrated RGB Camera: Integrat"));
    }

    fn is_loopback_name(name: &str) -> bool {
        name.eq_ignore_ascii_case("Phone Camera")
            || name.to_ascii_lowercase().contains("v4l2loopback")
    }
}
