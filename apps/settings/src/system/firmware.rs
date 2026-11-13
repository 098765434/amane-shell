//! Firmware updates on the System pages: devices known to fwupd, with any
//! release available, listed one per row. The page reads the state over
//! fwupd's system-bus API (`org.freedesktop.fwupd`) and shells out to
//! `fwupdmgr` for metadata refresh and installs, which handle polkit for
//! us — Emaki is the session's polkit agent so the prompt is drawn by the
//! shell itself.
//!
//! The writes go through `fwupdmgr` and not D-Bus because fwupd's `Install`
//! and `UpdateMetadata` take Unix FDs, which `amane::Argument` does not
//! carry today. See the follow-up noted in the plan file.

use std::process::Command;
use std::thread;
use std::time::Duration;

use amane::{Argument, Bus, Service, Value};
use emaki_kit::buttons::{PushButton, Style};
use emaki_kit::color::Accent;
use emaki_kit::form::FormRow;
use emaki_kit::icons::{ICON_CHECK, ICON_UPDATE};

use crate::{empty_row, group, groups};

const FWUPD: &str = "org.freedesktop.fwupd";
const ROOT: &str = "/";
const IFACE: &str = "org.freedesktop.fwupd";

/// How long the bus is given to come up before the page shows "No daemon".
const POLL_INTERVAL: Duration = Duration::from_secs(15);

/// `FwupdDeviceFlag::NeedsReboot`, as carried in GetDevices()'s `Flags` u64.
const FLAG_NEEDS_REBOOT: u64 = 1 << 8;
/// `FwupdDeviceFlag::Updatable`, filters devices the user can act on.
const FLAG_UPDATABLE: u64 = 1 << 3;

/// A device as the page shows it.
#[derive(Clone, Debug, PartialEq)]
struct Device {
    id: String,
    name: String,
    vendor: String,
    version: String,
    updatable: bool,
    needs_reboot: bool,
    upgrade: Option<Upgrade>,
}

#[derive(Clone, Debug, PartialEq)]
struct Upgrade {
    version: String,
    summary: String,
}

/// What a run of the `fwupdmgr` helper is doing.
#[derive(Clone, Debug, Default, PartialEq)]
enum Job {
    #[default]
    Idle,
    Refreshing,
    Updating {
        device_id: String,
    },
    Failed(String),
    JustFinished {
        needs_reboot: bool,
    },
}

pub struct Firmware {
    devices: Vec<Device>,
    asked: bool,
    reached: bool,
    job: Job,
}

impl Service for Firmware {
    fn new() -> Self {
        Self {
            devices: Vec::new(),
            asked: false,
            reached: false,
            job: Job::Idle,
        }
    }

    fn listen() {
        loop {
            let (devices, reached) = probe();
            {
                let mut now = Self::write();
                now.asked = true;
                now.reached = reached;
                if now.devices != devices {
                    now.devices = devices;
                } else {
                    now.quiet();
                }
            }
            thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Reads every updatable device fwupd knows about and any upgrades it has.
/// `reached` is false when fwupd isn't on the bus.
fn probe() -> (Vec<Device>, bool) {
    let bus = Bus::system();
    let list = match bus.try_call(FWUPD, ROOT, IFACE, "GetDevices", &[]) {
        Ok(Value::List(list)) => list,
        Ok(_) => return (Vec::new(), true),
        Err(error) => {
            // ServiceUnknown / NoReply == "the daemon isn't there"
            if matches!(
                error.name(),
                "org.freedesktop.DBus.Error.ServiceUnknown"
                    | "org.freedesktop.DBus.Error.NoReply"
                    | "org.freedesktop.DBus.Error.Disconnected"
            ) {
                return (Vec::new(), false);
            }
            log::warn!("fwupd GetDevices: {}: {}", error.name(), error.message());
            return (Vec::new(), true);
        }
    };

    let mut devices = Vec::new();
    for entry in list {
        let Some(device) = read_device(&entry) else {
            continue;
        };
        // skip internal plumbing devices the user can't act on, unless an upgrade is pending
        let upgrade = if device.updatable {
            upgrade_for(&bus, &device.id)
        } else {
            None
        };
        devices.push(Device { upgrade, ..device });
    }
    (devices, true)
}

fn read_device(value: &Value) -> Option<Device> {
    let id = value.get("DeviceId").text().to_string();
    if id.is_empty() {
        return None;
    }
    let flags = value.get("Flags").number() as u64;
    Some(Device {
        id,
        name: value.get("Name").text().to_string(),
        vendor: value.get("Vendor").text().to_string(),
        version: value.get("Version").text().to_string(),
        updatable: flags & FLAG_UPDATABLE != 0,
        needs_reboot: flags & FLAG_NEEDS_REBOOT != 0,
        upgrade: None,
    })
}

fn upgrade_for(bus: &Bus, device_id: &str) -> Option<Upgrade> {
    let answer = bus.try_call(
        FWUPD,
        ROOT,
        IFACE,
        "GetUpgrades",
        &[Argument::from(device_id)],
    );
    let list = match answer {
        Ok(Value::List(list)) => list,
        // "Nothing to do" is fwupd's ordinary answer when a device is up to date
        _ => return None,
    };
    let release = list.into_iter().next()?;
    let version = release.get("Version").text().to_string();
    if version.is_empty() {
        return None;
    }
    Some(Upgrade {
        version,
        summary: release.get("Summary").text().to_string(),
    })
}

/// Runs `fwupdmgr refresh --force` off the UI thread; fwupdmgr handles its
/// own polkit prompt through our agent.
fn refresh() {
    {
        let mut now = Firmware::write();
        now.job = Job::Refreshing;
    }
    thread::spawn(|| {
        let output = Command::new("fwupdmgr")
            .args(["refresh", "--force"])
            .output();
        let error = run_error(output, "refresh");
        let mut now = Firmware::write();
        now.job = match error {
            Some(why) => Job::Failed(why),
            None => Job::JustFinished {
                needs_reboot: false,
            },
        };
    });
}

/// Updates one device via `fwupdmgr update <id> -y`.
fn update_device(device_id: String) {
    {
        let mut now = Firmware::write();
        now.job = Job::Updating {
            device_id: device_id.clone(),
        };
    }
    thread::spawn(move || {
        let output = Command::new("fwupdmgr")
            .args(["update", &device_id, "-y", "--no-reboot-check"])
            .output();
        let error = run_error(output, "update");
        // re-probe straight away so the new version and any reboot flag show
        let (devices, reached) = probe();
        let needs_reboot = devices.iter().any(|d| d.needs_reboot);
        let mut now = Firmware::write();
        now.devices = devices;
        now.reached = reached;
        now.job = match error {
            Some(why) => Job::Failed(why),
            None => Job::JustFinished { needs_reboot },
        };
    });
}

fn run_error(output: std::io::Result<std::process::Output>, what: &str) -> Option<String> {
    match output {
        Ok(result) if result.status.success() => None,
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
            Some(if stderr.is_empty() {
                format!("fwupdmgr {what} exited {}", result.status)
            } else {
                stderr
            })
        }
        Err(why) => Some(format!("Couldn't run fwupdmgr: {why}")),
    }
}

/// Reboots the machine through logind, interactive (polkit prompts).
fn reboot() {
    thread::spawn(|| {
        Bus::system().call(
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
            "Reboot",
            &[Argument::from(true)],
        );
    });
}

pub fn page() -> amane::Column {
    let (devices, asked, reached, job) = {
        let now = Firmware::read();
        (now.devices.clone(), now.asked, now.reached, now.job.clone())
    };

    if !asked {
        return groups(vec![group().header("Firmware").row(crate::loading_row(
            "settings.system.firmware.asking",
            "Looking for fwupd…",
        ))]);
    }

    if !reached {
        return groups(vec![
            group()
                .header("Firmware")
                .row(
                    empty_row("settings.system.firmware.off", "fwupd isn't running")
                        .subtitle("The firmware updater service isn't on the system bus."),
                )
                .footer(
                    "Turn it on in your NixOS configuration: \
                     `programs.emaki.firmware.enable = true;`.",
                ),
        ]);
    }

    let busy = !matches!(job, Job::Idle | Job::Failed(_) | Job::JustFinished { .. });

    // top row: Check for updates, plus any job banner
    let check = FormRow::new("settings.system.firmware.check", "Check for updates")
        .subtitle("Downloads the latest firmware index from LVFS.")
        .trailing(
            PushButton::new("settings.system.firmware.check.button", "Check Now")
                .style(Style::Filled)
                .on_press(refresh)
                .build(),
        )
        .disabled(busy);

    let mut top = group().row(check);
    if let Some(banner) = job_banner(&job) {
        top = top.row(banner);
    }

    // one row per updatable device
    let rows: Vec<FormRow> = devices
        .iter()
        .filter(|d| d.updatable || d.upgrade.is_some())
        .map(|device| device_row(device, busy))
        .collect();

    let mut devices_group = group().header("Devices");
    if rows.is_empty() {
        devices_group = devices_group.row(
            empty_row(
                "settings.system.firmware.none",
                "No updatable devices found",
            )
            .subtitle("fwupd sees nothing on this machine that takes firmware updates."),
        );
    } else {
        devices_group = devices_group.rows(rows);
    }

    let needs_reboot = devices.iter().any(|d| d.needs_reboot)
        || matches!(job, Job::JustFinished { needs_reboot: true });

    let mut out = vec![top, devices_group];
    if needs_reboot {
        out.push(
            group()
                .row(
                    FormRow::new(
                        "settings.system.firmware.reboot",
                        "A restart will apply firmware updates",
                    )
                    .icon(ICON_UPDATE, Accent::Orange)
                    .trailing(
                        PushButton::new("settings.system.firmware.reboot.button", "Restart Now")
                            .style(Style::Filled)
                            .on_press(reboot)
                            .build(),
                    ),
                )
                .footer(
                    "Firmware installs finish on the next boot. Save your work before \
                     restarting.",
                ),
        );
    }

    groups(out)
}

fn device_row(device: &Device, busy: bool) -> FormRow {
    let id = format!(
        "settings.system.firmware.device.{}",
        sanitise_id(&device.id)
    );
    let title = if device.name.is_empty() {
        device.id.clone()
    } else {
        device.name.clone()
    };
    let subtitle = match (&device.vendor, &device.version) {
        (v, ver) if !v.is_empty() && !ver.is_empty() => format!("{v} · {ver}"),
        (_, ver) if !ver.is_empty() => ver.clone(),
        (v, _) if !v.is_empty() => v.clone(),
        _ => String::new(),
    };
    let row = FormRow::new(id.clone(), title).subtitle(subtitle);

    match &device.upgrade {
        Some(upgrade) => {
            let device_id = device.id.clone();
            let upgrade_label = format!("Update to {}", upgrade.version);
            let summary = if upgrade.summary.is_empty() {
                format!("New version: {}", upgrade.version)
            } else {
                upgrade.summary.clone()
            };
            row.subtitle(summary).trailing(
                PushButton::new(format!("{id}.update"), upgrade_label)
                    .style(Style::Filled)
                    .on_press(move || update_device(device_id.clone()))
                    .build(),
            )
        }
        None => row.icon(ICON_CHECK, Accent::Green).value("Up to date"),
    }
    .disabled(busy)
}

fn job_banner(job: &Job) -> Option<FormRow> {
    match job {
        Job::Idle => None,
        Job::Refreshing => Some(
            FormRow::new("settings.system.firmware.job", "Refreshing firmware index…")
                .subtitle("fwupdmgr refresh --force")
                .loading(),
        ),
        Job::Updating { device_id } => Some(
            FormRow::new(
                "settings.system.firmware.job",
                "Installing firmware update…",
            )
            .subtitle(format!("Device {}", short_id(device_id)))
            .loading(),
        ),
        Job::JustFinished {
            needs_reboot: false,
        } => Some(
            FormRow::new("settings.system.firmware.job", "All done")
                .icon(ICON_CHECK, Accent::Green)
                .subtitle("Nothing more to install."),
        ),
        Job::JustFinished { needs_reboot: true } => None, // the reboot card below is enough
        Job::Failed(why) => Some(crate::problem_row(
            "settings.system.firmware.failed",
            "The last firmware command failed",
            why,
            "",
        )),
    }
}

/// Last 8 chars of a fwupd device id, used in subtitles — the full id is a
/// 40-char sha-sum.
fn short_id(id: &str) -> &str {
    if id.len() > 8 {
        &id[id.len() - 8..]
    } else {
        id
    }
}

/// Keeps a device id usable as a kit widget id (alnum + dots only).
fn sanitise_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_takes_the_tail() {
        assert_eq!(
            short_id("0123456789abcdef0123456789abcdef01234567"),
            "01234567"
        );
        assert_eq!(short_id("short"), "short");
    }

    #[test]
    fn sanitise_id_strips_non_alnum() {
        assert_eq!(sanitise_id("abc/def.ghi"), "abc-def-ghi");
        assert_eq!(sanitise_id("ok1234"), "ok1234");
    }

    #[test]
    fn needs_reboot_is_set_from_flags() {
        let mut map = std::collections::BTreeMap::new();
        map.insert(String::from("DeviceId"), Value::Text(String::from("abc")));
        map.insert(String::from("Name"), Value::Text(String::from("UEFI")));
        map.insert(
            String::from("Flags"),
            Value::Number((FLAG_NEEDS_REBOOT | FLAG_UPDATABLE) as f64),
        );
        map.insert(String::from("Version"), Value::Text(String::from("1.0")));
        let value = Value::Map(map);
        let device = read_device(&value).expect("a device");
        assert!(device.needs_reboot);
        assert!(device.updatable);
    }

    #[test]
    fn a_device_without_an_id_is_dropped() {
        let value = Value::Map(std::collections::BTreeMap::new());
        assert!(read_device(&value).is_none());
    }
}
