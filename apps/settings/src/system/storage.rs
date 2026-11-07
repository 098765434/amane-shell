//! Storage: how full /nix and /boot are, what a clean-up would free, and
//! the settings the registry puts here (the boot entries kept).

use std::ffi::CString;
use std::thread;

use amane::{Column, Service};
use amane_kit::app::Dialog;
use amane_kit::buttons::PushButton;
use amane_kit::form::FormRow;
use amane_kit::spinner::ProgressBar;

use super::controls;
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

/// What a clean-up would remove, and what the last one freed.
pub struct Garbage {
    /// Store items nothing uses, from the last check.
    pub dead: Option<usize>,
    /// What the last check or clean-up said, in words.
    pub result: Option<String>,
    /// A check or a clean-up under way.
    pub running: bool,
}

impl Service for Garbage {
    fn new() -> Self {
        Self {
            dead: None,
            result: None,
            running: false,
        }
    }

    fn listen() {}
}

/// "5139 store paths would be deleted" → 5139.
fn dead_count(text: &str) -> Option<usize> {
    text.lines().rev().find_map(|line| {
        let line = line.trim();
        let count = line.strip_suffix("store paths would be deleted")?;
        count.trim().parse().ok()
    })
}

/// "5139 store paths deleted, 3210.45 MiB freed" → its size in words.
fn freed(text: &str) -> Option<String> {
    let line = text.lines().rev().find(|line| line.contains("freed"))?;
    let (amount, _) = line.split(',').nth(1)?.trim().split_once(" freed")?;
    let (number, unit) = amount.trim().split_once(' ')?;
    let number: f64 = number.parse().ok()?;
    let bytes = number
        * match unit {
            "KiB" => 1024.0,
            "MiB" => 1024.0 * 1024.0,
            "GiB" => 1024.0 * 1024.0 * 1024.0,
            _ => 1.0,
        };
    Some(human(bytes as u64))
}

/// `n` with thousands apart: 5,139.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Runs `args` through nix, which reports on stderr.
fn nix_said(program: &str, args: &[&str]) -> Result<String, String> {
    match amane_process::Cmd::new(program).args(args).output() {
        Ok(output) => Ok(output.stderr + &output.stdout),
        Err(failure) => {
            let text = failure.output.stderr + &failure.output.stdout;
            match text.lines().last() {
                Some(line) => Err(line.trim().to_string()),
                None => Err(failure.message),
            }
        }
    }
}

impl Garbage {
    /// Starts `work` unless something's running, and keeps what it says.
    fn run(work: impl FnOnce() -> (Option<usize>, String) + Send + 'static) {
        {
            let mut now = Self::write();
            if now.running {
                now.quiet();
                return;
            }
            now.running = true;
        }
        thread::spawn(move || {
            let (dead, said) = work();
            let mut now = Self::write();
            now.dead = dead;
            now.result = Some(said);
            now.running = false;
        });
    }

    fn check() {
        Self::run(|| match nix_said("nix-collect-garbage", &["--dry-run"]) {
            Ok(text) => match dead_count(&text) {
                Some(0) | None => (Some(0), String::from("Nothing to clean up")),
                Some(n) => (
                    Some(n),
                    format!("{} unused items can be removed", grouped(n)),
                ),
            },
            Err(why) => (None, format!("Couldn't check: {why}")),
        });
    }

    /// Removes what nothing uses; the system's old versions stay, so
    /// History can still go back to them.
    fn clean() {
        Self::run(|| match nix_said("nix-store", &["--gc"]) {
            Ok(text) => (
                Some(0),
                match freed(&text) {
                    Some(size) => format!("Freed {size}"),
                    None => String::from("Cleaned up"),
                },
            ),
            Err(why) => (None, format!("Couldn't clean up: {why}")),
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

    let found = if garbage.running {
        String::from("Looking…")
    } else {
        garbage
            .result
            .clone()
            .unwrap_or_else(|| String::from("Store items no system version uses any more"))
    };
    let dead = garbage.dead.unwrap_or(0);
    let button = if dead > 0 {
        PushButton::new("settings.storage.clean-up", "Clean Up…")
            .disabled(garbage.running)
            .on_press(move || {
                Dialog::new(
                    "settings.storage.clean-dialog",
                    format!("Remove {} unused items?", grouped(dead)),
                )
                .message(
                    "Old versions of the system stay, so History can still go back to them. \
                     It can take a few minutes.",
                )
                .button("Cancel", || {})
                .destructive("Clean Up", Garbage::clean)
                .open("main");
            })
            .build()
    } else {
        PushButton::new("settings.storage.check", "Check")
            .disabled(garbage.running)
            .on_press(Garbage::check)
            .build()
    };
    let clean = FormRow::new("settings.storage.clean", "Clean up")
        .subtitle(found)
        .trailing(button);
    let clean = if garbage.running {
        clean.loading()
    } else {
        clean
    };

    groups(vec![
        group()
            .row(meter("nix", "System and apps", "/nix"))
            .row(meter("boot", "Startup partition", "/boot")),
        group().row(clean).footer(
            "Every week, versions of the system older than 14 days are removed along with what \
             only they used.",
        ),
        group().rows(controls::rows("storage")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nix_says_what_it_would_free_and_freed() {
        assert_eq!(
            dead_count("determining…\n5139 store paths would be deleted\n"),
            Some(5139)
        );
        assert_eq!(dead_count("nothing"), None);
        assert_eq!(
            freed("5139 store paths deleted, 1000.00 MiB freed").as_deref(),
            Some("1.0 GB")
        );
        assert_eq!(grouped(5139), "5,139");
        assert_eq!(grouped(12), "12");
        assert_eq!(grouped(1234567), "1,234,567");
    }

    #[test]
    fn sizes_read_in_round_units() {
        assert_eq!(human(999), "999 B");
        assert_eq!(human(12_345), "12.3 kB");
        assert_eq!(human(123_456_789_012), "123.5 GB");
    }
}
