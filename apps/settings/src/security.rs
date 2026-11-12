//! Security: fingerprints enrolled with `fprintd`. Each row is one finger,
//! with Remove; a dropdown and button at the end enrol a new one through an
//! EnrollStart / EnrollStatus flow, with progress shown inline.
//!
//! fprintd is on the system bus (`net.reactivated.Fprint`) and its polkit
//! actions let the active session user enrol and delete their own fingers,
//! so Settings talks to it directly. Nothing here reaches a shell service;
//! the shell doesn't need to know about fingerprints. Unlock (swaylock,
//! greetd) is a PAM wire-up in the NixOS configuration, not an emaki Rust
//! change — see `nix/nixos-module.nix`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use amane::{Argument, Bus, Service, Value};
use emaki_kit::buttons::{IconButton, PushButton, Style};
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::FormRow;

use crate::{empty_row, group, groups};

const FPRINT: &str = "net.reactivated.Fprint";
const MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
const MANAGER: &str = "net.reactivated.Fprint.Manager";
const DEVICE: &str = "net.reactivated.Fprint.Device";

/// The ten finger names fprintd accepts, in the order the dropdown lists
/// them. `any` is not offered: users pick which finger they're enrolling.
const FINGERS: [(&str, &str); 10] = [
    ("right-index-finger", "Right index"),
    ("right-middle-finger", "Right middle"),
    ("right-thumb", "Right thumb"),
    ("right-ring-finger", "Right ring"),
    ("right-little-finger", "Right little"),
    ("left-index-finger", "Left index"),
    ("left-middle-finger", "Left middle"),
    ("left-thumb", "Left thumb"),
    ("left-ring-finger", "Left ring"),
    ("left-little-finger", "Left little"),
];

fn finger_label(name: &str) -> &'static str {
    FINGERS
        .iter()
        .find_map(|(key, label)| (*key == name).then_some(*label))
        .unwrap_or("Finger")
}

/// What the current enrolment is up to.
#[derive(Clone, Debug, Default, PartialEq)]
enum EnrollStage {
    #[default]
    Idle,
    Active {
        finger: String,
        status: String,
    },
    Failed(String),
}

/// What fprintd tells us about this machine and the current enrolment, if any.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fprint {
    /// The default device's object path, or `None` if no reader is on the bus.
    device: Option<String>,
    /// Finger names currently enrolled for this user.
    enrolled: Vec<String>,
    /// Whether the first read has run.
    asked: bool,
    /// What a call to fprintd said, when it failed outside enrolment.
    error: Option<String>,
    /// The enrolment flow, when one is running or just ended.
    stage: EnrollStage,
}

/// Drops back to `Idle` after an `enroll-completed` or `enroll-failed` shown
/// as feedback.
const CLEAR_AFTER: Duration = Duration::from_secs(3);

impl Service for Fprint {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {
        loop {
            let next = probe();
            {
                let mut now = Self::write();
                let stage = now.stage.clone();
                if now.device == next.device
                    && now.enrolled == next.enrolled
                    && now.error == next.error
                    && now.asked
                {
                    now.quiet();
                } else {
                    *now = Fprint {
                        asked: true,
                        stage,
                        ..next
                    };
                }
            }
            // fprintd changes only through our own enrol/delete, so a slow poll is enough
            // to pick up enrolments made with fprintd-enroll outside the UI
            thread::sleep(Duration::from_secs(3));
        }
    }
}

/// Asks fprintd what the default device is and what's enrolled on it.
fn probe() -> Fprint {
    let bus = Bus::system();
    let device = match bus.try_call(FPRINT, MANAGER_PATH, MANAGER, "GetDefaultDevice", &[]) {
        Ok(Value::Text(path)) => Some(path),
        Ok(_) => None,
        Err(error) => {
            // NoSuchDevice is the ordinary "no reader" answer, not an error for the user
            if error.name().ends_with(".NoSuchDevice") {
                return Fprint::default();
            }
            return Fprint {
                error: Some(error.message().to_string()),
                ..Fprint::default()
            };
        }
    };

    let Some(device_path) = device.clone() else {
        return Fprint::default();
    };

    let enrolled = enrolled_fingers(&device_path, &username());
    Fprint {
        device,
        enrolled,
        asked: false,
        error: None,
        stage: EnrollStage::Idle,
    }
}

fn username() -> String {
    std::env::var("USER").unwrap_or_default()
}

/// `ListEnrolledFingers` as a `Vec<String>`; an empty list on error or no enrolments.
fn enrolled_fingers(device_path: &str, user: &str) -> Vec<String> {
    let value = Bus::system().call(
        FPRINT,
        device_path,
        DEVICE,
        "ListEnrolledFingers",
        &[Argument::from(user)],
    );
    match value {
        Value::List(list) => list
            .into_iter()
            .map(|v| v.text().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Removes `finger` for the current user, off the UI thread.
fn delete(device_path: String, finger: String) {
    thread::spawn(move || {
        let bus = Bus::system();
        let user = username();
        // Claim is needed before DeleteEnrolledFinger
        let _ = bus.try_call(
            FPRINT,
            &device_path,
            DEVICE,
            "Claim",
            &[Argument::from(user.as_str())],
        );
        let result = bus.try_call(
            FPRINT,
            &device_path,
            DEVICE,
            "DeleteEnrolledFinger",
            &[Argument::from(finger.as_str())],
        );
        let _ = bus.try_call(FPRINT, &device_path, DEVICE, "Release", &[]);
        let error = result.err().map(|e| e.message().to_string());
        let enrolled = enrolled_fingers(&device_path, &user);
        let mut now = Fprint::write();
        now.enrolled = enrolled;
        now.error = error;
    });
}

/// Where the inline dropdown's selection is kept between redraws. The
/// Dropdown's `on_select` doesn't tie into a Service, so this one atomic
/// index follows the picker through each `page()` build.
static PICKED: AtomicUsize = AtomicUsize::new(0);

/// Humanises an `EnrollStatus` result for the user.
fn enroll_status_text(result: &str) -> String {
    match result {
        "enroll-stage-passed" => String::from("Good, lift and place your finger again."),
        "enroll-retry-scan" => String::from("Try again, holding your finger still."),
        "enroll-swipe-too-short" => String::from("Swipe again, more slowly."),
        "enroll-finger-not-centered" => String::from("Centre your finger on the reader."),
        "enroll-remove-and-retry" => String::from("Lift your finger and place it again."),
        "enroll-data-full" => String::from("The reader is full. Remove a finger first."),
        "enroll-completed" => String::from("Enrolled."),
        "enroll-failed" => String::from("Enrolment failed; try again."),
        "" => String::from("Place your finger on the reader."),
        other => format!("Place your finger on the reader ({other})."),
    }
}

/// Starts an enrolment on a worker thread.
fn start_enroll(device_path: String, finger: String) {
    {
        let mut now = Fprint::write();
        now.stage = EnrollStage::Active {
            finger: finger.clone(),
            status: String::from("Place your finger on the reader."),
        };
        now.error = None;
    }
    thread::spawn(move || {
        let bus = Bus::system();
        let user = username();
        if let Err(error) = bus.try_call(
            FPRINT,
            &device_path,
            DEVICE,
            "Claim",
            &[Argument::from(user.as_str())],
        ) {
            let mut now = Fprint::write();
            now.stage = EnrollStage::Failed(format!("Couldn't claim the reader: {}", error.message()));
            schedule_clear();
            return;
        }
        if let Err(error) = bus.try_call(
            FPRINT,
            &device_path,
            DEVICE,
            "EnrollStart",
            &[Argument::from(finger.as_str())],
        ) {
            let _ = bus.try_call(FPRINT, &device_path, DEVICE, "Release", &[]);
            let mut now = Fprint::write();
            now.stage = EnrollStage::Failed(format!("Couldn't start enrolment: {}", error.message()));
            schedule_clear();
            return;
        }

        // wait for EnrollStatus { result, done } until the terminal signal
        for signal in bus.signals_at(&device_path, DEVICE, "EnrollStatus") {
            let args = signal.arguments();
            let result = args.first().map(Value::text).unwrap_or_default().to_string();
            let done = args.get(1).map(Value::bool).unwrap_or_default();
            {
                let mut now = Fprint::write();
                now.stage = EnrollStage::Active {
                    finger: finger.clone(),
                    status: enroll_status_text(&result),
                };
            }
            if done || matches!(result.as_str(), "enroll-completed" | "enroll-failed") {
                let _ = bus.try_call(FPRINT, &device_path, DEVICE, "EnrollStop", &[]);
                let _ = bus.try_call(FPRINT, &device_path, DEVICE, "Release", &[]);
                let enrolled = enrolled_fingers(&device_path, &user);
                let mut now = Fprint::write();
                now.enrolled = enrolled;
                if result == "enroll-failed" {
                    now.stage = EnrollStage::Failed(String::from("Enrolment failed; try again."));
                }
                schedule_clear();
                return;
            }
        }
    });
}

/// Returns the stage to Idle after a short delay, so the user sees the
/// final message.
fn schedule_clear() {
    thread::spawn(|| {
        thread::sleep(CLEAR_AFTER);
        let mut now = Fprint::write();
        if !matches!(now.stage, EnrollStage::Idle) {
            now.stage = EnrollStage::Idle;
        } else {
            now.quiet();
        }
    });
}

pub fn page() -> amane::Column {
    let (device, enrolled, asked, error, stage) = {
        let now = Fprint::read();
        (
            now.device.clone(),
            now.enrolled.clone(),
            now.asked,
            now.error.clone(),
            now.stage.clone(),
        )
    };

    if !asked {
        return groups(vec![group().header("Fingerprint").row(crate::loading_row(
            "settings.security.asking",
            "Looking for a reader…",
        ))]);
    }

    let Some(device_path) = device else {
        return groups(vec![
            group()
                .header("Fingerprint")
                .row(
                    empty_row("settings.security.no-reader", "No fingerprint reader")
                        .subtitle("This machine's fingerprint reader isn't on the system bus."),
                )
                .footer(
                    "Enable fprintd in your NixOS configuration: \
                     `services.fprintd.enable = true;`. If your laptop has no reader, \
                     this page stays empty.",
                ),
        ]);
    };

    // one row per enrolled finger, with Remove
    let busy = !matches!(stage, EnrollStage::Idle);
    let rows: Vec<FormRow> = enrolled
        .iter()
        .map(|finger| {
            let id = format!("settings.security.finger.{finger}");
            let device_for_delete = device_path.clone();
            let name = finger.clone();
            FormRow::new(id.clone(), finger_label(finger))
                .trailing(
                    IconButton::new(format!("{id}.remove"), emaki_kit::icons::ICON_DELETE)
                        .tooltip(crate::WINDOW, "Remove")
                        .style(Style::Destructive)
                        .on_press(move || delete(device_for_delete.clone(), name.clone()))
                        .build(),
                )
                .disabled(busy)
        })
        .collect();

    let mut fingers = group().header("Fingerprints");
    if rows.is_empty() {
        fingers = fingers.row(
            empty_row("settings.security.no-fingers", "No fingers enrolled yet")
                .subtitle("Add one below to unlock with a touch."),
        );
    } else {
        fingers = fingers.rows(rows);
    }
    let fingers = fingers.footer(
        "Unlocking the screen with a finger needs `services.fprintd.enable = true;` in \
         NixOS, and PAM wired up in the lock screen \
         (`security.pam.services.swaylock.fprintAuth = true;`).",
    );

    // "Add" group: dropdown of free fingers + Add button; status row underneath while enrolling
    let free: Vec<(&'static str, &'static str)> = FINGERS
        .iter()
        .copied()
        .filter(|(name, _)| !enrolled.iter().any(|f| f == name))
        .collect();

    let picked = PICKED.load(Ordering::Relaxed).min(free.len().saturating_sub(1));
    let device_for_add = device_path.clone();
    let free_for_add = free.clone();

    let dropdown = if free.is_empty() {
        None
    } else {
        Some(
            Dropdown::new(
                "settings.security.pick-finger",
                free.iter().map(|(_, label)| (*label).to_string()).collect(),
                picked,
            )
            .width(crate::MENU_W_WIDE)
            .on_select(|index| PICKED.store(index, Ordering::Relaxed))
            .build(),
        )
    };

    let add_button = PushButton::new("settings.security.add.button", "Add Finger")
        .style(Style::Filled)
        .on_press(move || {
            let index = PICKED.load(Ordering::Relaxed).min(free_for_add.len().saturating_sub(1));
            let Some((name, _)) = free_for_add.get(index) else {
                return;
            };
            start_enroll(device_for_add.clone(), (*name).to_string());
        })
        .build();

    let mut add_group = group().header("Add a finger");
    if let Some(dropdown) = dropdown {
        add_group = add_group.row(
            FormRow::new("settings.security.add", "Finger to add")
                .leading(dropdown, crate::MENU_W_WIDE)
                .trailing(add_button)
                .disabled(busy),
        );
    } else {
        add_group = add_group.row(
            empty_row("settings.security.full", "All fingers are enrolled")
                .subtitle("Remove one above to add another."),
        );
    }

    // status row under the dropdown while enrolling, or an error from a delete
    let status_row = match (&stage, &error) {
        (EnrollStage::Active { finger, status }, _) => Some(
            FormRow::new("settings.security.status", finger_label(finger))
                .subtitle(status.clone())
                .icon(
                    emaki_kit::icons::ICON_FINGERPRINT,
                    emaki_kit::color::Accent::Teal,
                ),
        ),
        (EnrollStage::Failed(why), _) => Some(crate::problem_row(
            "settings.security.failed",
            "Enrolment didn't finish",
            why,
            "",
        )),
        (EnrollStage::Idle, Some(why)) => Some(crate::problem_row(
            "settings.security.error",
            "Couldn't change fingerprints",
            why,
            "",
        )),
        _ => None,
    };
    if let Some(row) = status_row {
        add_group = add_group.row(row);
    }

    groups(vec![fingers, add_group])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_finger_has_a_label() {
        assert_eq!(finger_label("right-index-finger"), "Right index");
        assert_eq!(finger_label("left-thumb"), "Left thumb");
        assert_eq!(finger_label("unknown"), "Finger");
    }

    #[test]
    fn fingers_are_unique() {
        let mut names: Vec<&str> = FINGERS.iter().map(|(name, _)| *name).collect();
        names.sort();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn enroll_status_text_is_specific_where_it_can_be() {
        assert_eq!(enroll_status_text("enroll-completed"), "Enrolled.");
        assert_eq!(
            enroll_status_text("enroll-finger-not-centered"),
            "Centre your finger on the reader.",
        );
        assert!(enroll_status_text("unknown-code").contains("unknown-code"));
    }
}
