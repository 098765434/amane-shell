//! Font stacks that actually fall back. amane takes one family name and
//! fontconfig always answers with *some* font, so the shell picks the first
//! family in a token's stack that is really installed.

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};

use fontconfig::{Fontconfig, Pattern};

/// CSS generic families: fontconfig maps these to a configured font.
const GENERIC: &[&str] = &[
    "sans-serif",
    "serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "emoji",
    "math",
];

/// The first family in `stack` that fontconfig has installed, else the
/// last entry. Each stack is looked up once.
pub fn first_installed(stack: &[String]) -> String {
    static CHOSEN: OnceLock<Mutex<HashMap<Vec<String>, String>>> = OnceLock::new();
    let mut chosen = CHOSEN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    chosen
        .entry(stack.to_vec())
        .or_insert_with(|| {
            let fontconfig = Fontconfig::new();
            let pick = pick(stack, |family| {
                fontconfig.as_ref().is_some_and(|fc| installed(fc, family))
            });
            if std::env::var_os("AMANE_DEBUG").is_some() {
                eprintln!("amane-shell: font stack {stack:?} → {pick}");
            }
            pick
        })
        .clone()
}

fn pick(stack: &[String], installed: impl Fn(&str) -> bool) -> String {
    stack
        .iter()
        .find(|family| GENERIC.contains(&family.to_ascii_lowercase().as_str()) || installed(family))
        .or(stack.last())
        .cloned()
        .unwrap_or_default()
}

/// Whether fontconfig's best match for `family` is that family.
fn installed(fc: &Fontconfig, family: &str) -> bool {
    let Ok(name) = CString::new(family) else {
        return false;
    };
    let Ok(mut pattern) = Pattern::new(fc) else {
        return false;
    };
    if pattern.add_string(c"family", &name).is_err() || pattern.config_substitute().is_err() {
        return false;
    }
    pattern.default_substitute();

    pattern
        .font_match()
        .ok()
        .and_then(|m| {
            m.get_string(c"family")
                .ok()
                .map(|f| f.eq_ignore_ascii_case(family))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn made_up_names_fall_to_the_generic() {
        let s = stack(&["No Such Font 7f3a", "Also Missing 91b2", "sans-serif"]);
        assert_eq!(first_installed(&s), "sans-serif");
    }

    #[test]
    fn nothing_installed_gives_the_last_entry() {
        let s = stack(&["Missing A", "Missing B"]);
        assert_eq!(pick(&s, |_| false), "Missing B");
        assert_eq!(pick(&[], |_| false), "");
    }

    #[test]
    fn first_installed_wins() {
        let s = stack(&["SF Pro Text", "Inter", "sans-serif"]);
        assert_eq!(pick(&s, |f| f == "Inter"), "Inter");
        assert_eq!(pick(&s, |_| true), "SF Pro Text");
    }
}
