//! The system profile's generations: what History lists and Restore goes
//! back to.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, Datelike, Local};

use crate::Config;
use crate::repo::version_of;

/// How many of the newest generations run `nixos-version` for their commit
/// (a script each); the older ones show their version alone.
const WITH_REVISION: usize = 20;

/// One generation of the `system` profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Generation {
    /// The number in its `system-<n>-link`.
    pub number: u32,
    /// The store path the link leads to.
    pub toplevel: PathBuf,
    /// When the link was made.
    pub date: DateTime<Local>,
    /// `<link>/nixos-version`: "26.05.20261015.26f4107".
    pub version: String,
    /// The commit it was built from, when it records one.
    pub revision: Option<String>,
    /// That commit's subject, when the repo has it.
    pub subject: Option<String>,
    /// Where its `kernel` link leads, when it has one.
    pub kernel: Option<PathBuf>,
}

impl Generation {
    /// "Today 14:02", "Yesterday 09:10", "Mon 20 Oct 14:02" or "20 Oct 2025".
    pub fn when(&self) -> String {
        let now = Local::now();
        let days = now
            .date_naive()
            .signed_duration_since(self.date.date_naive())
            .num_days();
        match days {
            0 => format!("Today {}", self.date.format("%H:%M")),
            1 => format!("Yesterday {}", self.date.format("%H:%M")),
            _ if now.year() == self.date.year() => self.date.format("%a %-d %b %H:%M").to_string(),
            _ => self.date.format("%-d %b %Y").to_string(),
        }
    }

    /// What names it in a list: the commit's subject, else its version.
    pub fn title(&self) -> String {
        self.subject
            .clone()
            .unwrap_or_else(|| format!("NixOS {}", self.version))
    }
}

/// The number in "system-65-link".
pub fn number(name: &str) -> Option<u32> {
    name.strip_prefix("system-")?
        .strip_suffix("-link")?
        .parse()
        .ok()
}

/// The system profile's generations, newest first, and which ones run and booted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    /// Newest first.
    pub list: Vec<Generation>,
    /// Where `/run/current-system` leads.
    pub current: Option<PathBuf>,
    /// Where `/run/booted-system` leads.
    pub booted: Option<PathBuf>,
    /// The generation the `system` profile points at.
    pub profile: Option<u32>,
}

/// Reads the profile's generations, the newest ones' commits and their
/// subjects from the repo.
pub fn scan(config: &Config) -> Scan {
    let mut list: Vec<Generation> = fs::read_dir(&config.profiles)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let number = number(&name)?;
                    let link = entry.path();
                    let toplevel = fs::read_link(&link).ok()?;
                    let date = fs::symlink_metadata(&link)
                        .and_then(|meta| meta.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    Some(Generation {
                        number,
                        version: fs::read_to_string(link.join("nixos-version"))
                            .map(|v| v.trim().to_string())
                            .unwrap_or_default(),
                        kernel: fs::read_link(link.join("kernel")).ok(),
                        toplevel,
                        date: DateTime::from(date),
                        revision: None,
                        subject: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    list.sort_by_key(|g| std::cmp::Reverse(g.number));

    // the newest ones' commits, and their subjects from the repo (once each)
    let mut subjects: HashMap<String, Option<String>> = HashMap::new();
    for generation in list.iter_mut().take(WITH_REVISION) {
        let Some((_, _, revision)) = version_of(&generation.toplevel) else {
            continue;
        };
        if let Some(revision) = &revision {
            let sha = revision
                .strip_suffix("-dirty")
                .unwrap_or(revision)
                .to_string();
            let subject = subjects
                .entry(sha.clone())
                .or_insert_with(|| {
                    config
                        .git(&["log", "-1", "--format=%s", &sha])
                        .ok()
                        .map(|out| out.trim().to_string())
                        .filter(|s| !s.is_empty())
                })
                .clone();
            generation.subject = subject;
        }
        generation.revision = revision;
    }

    Scan {
        list,
        current: fs::read_link(&config.current_system).ok(),
        booted: fs::read_link(&config.booted_system).ok(),
        profile: fs::read_link(config.profiles.join("system"))
            .ok()
            .and_then(|link| number(&link.to_string_lossy())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_link_names_its_generation() {
        assert_eq!(number("system-65-link"), Some(65));
        assert_eq!(number("system"), None);
        assert_eq!(number("system-x-link"), None);
    }
}
