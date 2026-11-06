//! Storage: how full /nix and /boot are, what a clean-up would free, and
//! the settings the registry puts here (the boot entries kept).

use std::ffi::CString;
use std::thread;

use amane::{Column, Service};
use amane_kit::buttons::PushButton;
use amane_kit::form::FormRow;
use amane_kit::spinner::ProgressBar;

use super::controls;
use super::nixos::run;
use crate::{group, groups};

/// Used and total bytes of the file system at `path`.
fn disk(path: &str) -> Option<(u64, u64)> {
    let path = CString::new(path).ok()?;
    // SAFETY: a zeroed statvfs is a valid one for the call to fill, and
    // `path` is a NUL-terminated string that outlives it.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    let block = stat.f_frsize as u64;
    let total = stat.f_blocks as u64 * block;
    let free = stat.f_bavail as u64 * block;
    (total > 0).then(|| (total.saturating_sub(free), total))
}

/// "12.3 GB".
pub fn human(bytes: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1000.0 && unit < units.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", units[unit])
    }
}

/// What `nix-collect-garbage --dry-run` would free.
pub struct Garbage {
    pub result: Option<String>,
    pub running: bool,
}

impl Service for Garbage {
    fn new() -> Self {
        Self {
            result: None,
            running: false,
        }
    }

    fn listen() {}
}

impl Garbage {
    fn check() {
        {
            let mut now = Self::write();
            if now.running {
                now.quiet();
                return;
            }
            now.running = true;
        }
        thread::spawn(|| {
            // it reports on stderr, and says nothing on stdout
            let output = std::process::Command::new("nix-collect-garbage")
                .arg("--dry-run")
                .stdin(std::process::Stdio::null())
                .output();
            let said = match output {
                Ok(out) => {
                    let text = String::from_utf8_lossy(&out.stderr).into_owned()
                        + &String::from_utf8_lossy(&out.stdout);
                    text.lines()
                        .rev()
                        .find(|line| line.contains("freed") || line.contains("would"))
                        .map(|line| line.trim().to_string())
                        .unwrap_or_else(|| String::from("Nothing to free."))
                }
                Err(error) => format!("nix-collect-garbage: {error}"),
            };
            let mut now = Self::write();
            now.result = Some(said);
            now.running = false;
        });
    }
}

/// How full the file system at `path` is, as a row with a meter.
fn meter(id: &str, title: &str, path: &str) -> FormRow {
    let row = FormRow::new(format!("settings.storage.{id}"), title);
    match disk(path) {
        Some((used, total)) => {
            let percent = (used * 100 / total.max(1)) as u8;
            row.subtitle(format!(
                "{} of {} used · {} free",
                human(used),
                human(total),
                human(total.saturating_sub(used))
            ))
            .trailing(ProgressBar::new(f32::from(percent) / 100.0, 140.0).build())
        }
        None => row.subtitle("Not mounted"),
    }
}

pub fn page() -> Column {
    let garbage = Garbage::read();
    let _ = run; // the dry run goes through Command for its stderr

    let found = if garbage.running {
        String::from("Looking…")
    } else {
        garbage
            .result
            .clone()
            .unwrap_or_else(|| String::from("Old generations and unreferenced store paths"))
    };

    groups(vec![
        group()
            .row(meter("nix", "Nix store (/nix)", "/nix"))
            .row(meter("boot", "Boot partition (/boot)", "/boot")),
        group()
            .row(
                {
                    let row = FormRow::new("settings.storage.clean", "Clean up").subtitle(found);
                    if garbage.running { row.loading() } else { row }
                }
                .trailing(
                    PushButton::new("settings.storage.check", "Check…")
                        .disabled(garbage.running)
                        .on_press(Garbage::check)
                        .build(),
                ),
            )
            .footer(
                "The weekly clean-up keeps the last 14 days; run `sudo nix-collect-garbage -d` \
                 to free it all now.",
            ),
        group().rows(controls::rows("storage")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_in_round_units() {
        assert_eq!(human(999), "999 B");
        assert_eq!(human(12_345), "12.3 kB");
        assert_eq!(human(123_456_789_012), "123.5 GB");
    }
}
