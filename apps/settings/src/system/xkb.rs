//! The keyboard layouts and their variants, as `localectl` lists them
//! from xkeyboard-config, for checking what's typed on the General page.
//! The layout list is read once; variants once per layout.

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock, PoisonError};

static LAYOUTS: OnceLock<Vec<String>> = OnceLock::new();
static VARIANTS: Mutex<BTreeMap<String, Vec<String>>> = Mutex::new(BTreeMap::new());

fn localectl(args: &[&str]) -> String {
    Command::new("localectl")
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// One name per line, as localectl prints them.
pub fn parse_list(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("Failed"))
        .map(String::from)
        .collect()
}

/// Every layout; empty when localectl isn't around (then nothing is checked).
pub fn layouts() -> &'static [String] {
    LAYOUTS.get_or_init(|| parse_list(&localectl(&["list-x11-keymap-layouts"])))
}

/// The variants of `layout`.
pub fn variants(layout: &str) -> Vec<String> {
    let mut known = VARIANTS.lock().unwrap_or_else(PoisonError::into_inner);
    known
        .entry(layout.to_string())
        .or_insert_with(|| parse_list(&localectl(&["list-x11-keymap-variants", layout])))
        .clone()
}

/// A few of the names, for an error: "smi, dvorak, colemak and 3 more".
pub fn some(names: &[String]) -> String {
    const SHOWN: usize = 4;
    if names.is_empty() {
        return String::from("no variants");
    }
    let first = names
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > SHOWN {
        format!("{first} and {} more", names.len() - SHOWN)
    } else {
        first
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_one_name_per_line() {
        assert_eq!(parse_list("no\nus\n\n gb \n"), ["no", "us", "gb"]);
        assert_eq!(
            some(&parse_list("a\nb\nc\nd\ne\nf")),
            "a, b, c, d and 2 more"
        );
        assert_eq!(some(&[]), "no variants");
    }
}
