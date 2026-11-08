//! The phone's camera as this computer's webcam: `emaki-mirror --camera`
//! feeding the v4l2loopback device nixos-config loads, run as a user unit
//! of its own so it outlives the shell.

use amane::Service;

use super::MIRROR_APP;

/// The user unit the webcam runs as, apart from the shell's own.
pub const CAMERA_UNIT: &str = "emaki-phone-camera";

/// The loopback device's name, as nixos-config loads v4l2loopback.
pub const CAMERA_LABEL: &str = "Phone Camera";

/// Which camera the webcam is, while it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Front,
    Back,
}

/// The phone's webcam as the tile shows it: whether its loopback device is there, and which
/// camera runs. Read on the service's thread once a second (sysfs and every process's command
/// line), only while the tile shows it, so a view never does that file IO itself.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Webcam {
    /// Whether the loopback device is there.
    pub loopback: bool,
    /// Which camera runs, if one does.
    pub running: Option<Facing>,
}

impl Service for Webcam {
    fn new() -> Self {
        Self::default()
    }

    fn idle_stop() -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(2))
    }

    fn update(&mut self) -> bool {
        let fresh = Self {
            loopback: loopback(),
            running: camera(),
        };
        let changed = fresh != *self;
        *self = fresh;
        changed
    }
}

fn loopback() -> bool {
    std::fs::read_dir("/sys/class/video4linux").is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            std::fs::read_to_string(entry.path().join("name"))
                .is_ok_and(|name| name.trim() == CAMERA_LABEL)
        })
    })
}

/// The running webcam's camera, from its process.
fn camera() -> Option<Facing> {
    std::fs::read_dir("/proc").ok().and_then(|entries| {
        entries.flatten().find_map(|entry| {
            let line = std::fs::read(entry.path().join("cmdline")).ok()?;
            let words: Vec<&[u8]> = line.split(|b| *b == 0).collect();
            let at = words.iter().position(|word| *word == b"--camera")?;
            if !words.iter().any(|word| {
                word.ends_with(MIRROR_APP.as_bytes()) || word.ends_with(b".emaki-mirror-wrapped")
            }) {
                return None;
            }
            match words.get(at + 1) {
                Some(&b"back") => Some(Facing::Back),
                _ => Some(Facing::Front),
            }
        })
    })
}

/// Runs the webcam with the `facing` camera of `device`, or stops it.
pub fn set_camera(device: &str, facing: Option<Facing>) {
    let device = device.to_string();
    std::thread::spawn(move || {
        // the one running stops first, so the new one can have the camera
        let unit = format!("{CAMERA_UNIT}.service");
        // a failure is logged, and one that was never running is stopped already
        let _ = crate::services::run_now("systemctl", &["--user".into(), "stop".into(), unit]);
        if let Some(facing) = facing {
            let which = if facing == Facing::Back {
                "back"
            } else {
                "front"
            };
            crate::services::launch(&[
                "systemd-run",
                "--user",
                &format!("--unit={CAMERA_UNIT}"),
                "--collect",
                "--quiet",
                MIRROR_APP,
                "--camera",
                which,
                "--device",
                &device,
            ]);
        }
    });
    // shown at once; the next reading confirms it
    Webcam::write().running = facing;
}
