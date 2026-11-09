//! Storage: how full /nix and /boot are, and what a clean-up would free.

use std::thread;

use amane::{Column, Service};
use emaki_kit::app::Dialog;
use emaki_kit::buttons::PushButton;
use emaki_kit::form::FormRow;
use emaki_kit::spinner::ProgressBar;

use crate::{group, groups};

/// Used and total bytes of the file system at `path`.
fn disk(path: &str) -> Option<(u64, u64)> {
    let space = emaki_kit::fs::space(std::path::Path::new(path))?;
    (space.total > 0).then(|| (space.total.saturating_sub(space.available), space.total))
}

/// What a clean-up would remove, and what the last one freed.
pub struct Garbage {
    /// Store items nothing uses, from the last check.
    pub dead: Option<usize>,
    /// What the last check or clean-up said, in words.
    pub result: Option<String>,
    /// A check or a clean-up under way.
    pub running: bool,
    /// What the one under way is doing, in words.
    pub doing: &'static str,
}

impl Service for Garbage {
    fn new() -> Self {
        Self {
            dead: None,
            result: None,
            running: false,
            doing: "",
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
    Some(emaki_kit::format::bytes(bytes as u64))
}

/// Runs `args` through nix, which reports on stderr.
fn nix_said(program: &str, args: &[&str]) -> Result<String, String> {
    match emaki_process::Cmd::new(program).args(args).output() {
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
    fn run(doing: &'static str, work: impl FnOnce() -> (Option<usize>, String) + Send + 'static) {
        {
            let mut now = Self::write();
            if now.running {
                now.quiet();
                return;
            }
            now.running = true;
            now.doing = doing;
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
        Self::run("Looking…", || {
            match nix_said("nix-collect-garbage", &["--dry-run"]) {
                Ok(text) => match dead_count(&text) {
                    Some(0) | None => (Some(0), String::from("Nothing to clean up")),
                    Some(n) => (
                        Some(n),
                        format!(
                            "{} unused items can be removed",
                            emaki_kit::format::thousands(n)
                        ),
                    ),
                },
                Err(why) => (None, format!("Couldn't check: {why}")),
            }
        });
    }

    /// Removes what nothing uses; the system's old versions stay, so
    /// History can still go back to them.
    fn clean() {
        Self::run("Removing unused items…", || {
            match nix_said("nix-store", &["--gc"]) {
                Ok(text) => (
                    Some(0),
                    match freed(&text) {
                        Some(size) => format!("Freed {size}"),
                        None => String::from("Cleaned up"),
                    },
                ),
                Err(why) => (None, format!("Couldn't clean up: {why}")),
            }
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
                emaki_kit::format::bytes(used),
                emaki_kit::format::bytes(total),
                emaki_kit::format::bytes(total.saturating_sub(used))
            ))
            .trailing(ProgressBar::new(f32::from(percent) / 100.0, crate::MENU_W_NARROW).build())
        }
        None => row.subtitle("Not mounted"),
    }
}

pub fn page() -> Column {
    let garbage = Garbage::read();

    let found = if garbage.running {
        String::from(garbage.doing)
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
                    format!(
                        "Remove {} unused items?",
                        emaki_kit::format::thousands(dead)
                    ),
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
    }
}
