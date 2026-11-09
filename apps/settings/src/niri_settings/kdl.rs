//! `settings.kdl`: niri settings changed in Emaki Settings, beside niri's
//! `config.kdl`, which includes it as its last line. This module is the
//! file's one writer (a test holds every other file to that).
//!
//! niri merges the sections here (`layout`, `cursor`, `gestures`,
//! `overview`, `recent-windows`, `hotkey-overlay`) into the ones before
//! field by field, and the later file wins. So the file holds only what
//! was changed in Settings: [`Sparse`] keeps each setting's line by its
//! section and key, and a setting it doesn't hold is whatever
//! `config.kdl` says, or niri's default. Removing one puts that back.
//!
//! Lines it doesn't know, in a section or between them, are kept as they
//! were and written back, so an edit by hand isn't lost.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::niri_files::{
    self, Child, beside_config, children, code, commit, dirty, niri_config, node_end, write_atomic,
};

pub const FILE: &str = "settings.kdl";

const HEADER: &str = "// Written by Emaki Settings, which reads it back; lines it doesn't\n\
                      // know are kept. niri's config.kdl includes it last, so what's\n\
                      // here wins over config.kdl's own setting.\n";

/// The sections it keeps settings in, outer ones first; `""` is the top
/// level, and `layout/mat` is the `mat` block in `layout`.
pub const SECTIONS: [&str; 11] = [
    "",
    "layout",
    "layout/focus-ring",
    "layout/border",
    "layout/shadow",
    "layout/mat",
    "overview",
    "recent-windows",
    "gestures",
    "cursor",
    "hotkey-overlay",
];

/// The top-level settings it keeps; anything else at the top level is
/// kept as it was.
const TOP: [&str; 2] = ["prefer-no-csd", "screenshot-path"];

/// A setting: its section (one of [`SECTIONS`]) and its key. A section's
/// `on` or `off` line is its `state`.
pub type Key = (&'static str, &'static str);

/// The settings a file holds, by section and key, each as its line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sparse {
    lines: BTreeMap<(String, String), String>,
    /// Lines in a section that aren't settings it reads, kept in order.
    extra: BTreeMap<String, Vec<String>>,
    /// Top-level nodes that aren't its sections, as they were.
    outside: Vec<String>,
}

/// The key a line sets: its first word, and `state` for `on` or `off`.
fn key_of(line: &str) -> &str {
    let word = code(line).trim().split([' ', '{']).next().unwrap_or("");
    match word {
        "on" | "off" => "state",
        word => word,
    }
}

fn inner<'a>(lines: &'a [&'a str]) -> &'a [&'a str] {
    &lines[1..lines.len().saturating_sub(1)]
}

impl Sparse {
    /// The settings in `text`: what [`write`](Self::write) wrote, or
    /// `config.kdl` (whose other nodes then land in `outside`, unused).
    pub fn read(text: &str) -> Self {
        let mut sparse = Sparse::default();
        let lines: Vec<&str> = text.lines().collect();
        let mut at = 0;
        while at < lines.len() {
            let raw = lines[at];
            let line = raw.trim();
            if line.is_empty() || HEADER.lines().any(|header| header == line) {
                at += 1;
                continue;
            }
            let end = node_end(&lines, at);
            let node = &lines[at..=end];
            let name = code(line).trim().split([' ', '{']).next().unwrap_or("");
            let commented = line.starts_with("//") || line.starts_with("/-");
            if !commented && node.len() > 1 && SECTIONS.contains(&name) {
                sparse.read_section(name, inner(node));
            } else if !commented && node.len() == 1 && TOP.contains(&name) {
                sparse.put("", name, line.to_string());
            } else {
                sparse
                    .outside
                    .extend(node.iter().map(|raw| raw.trim_end().to_string()));
            }
            at = end + 1;
        }
        sparse
    }

    fn read_section(&mut self, section: &str, inner_lines: &[&str]) {
        let mut has_state = false;
        for child in children(inner_lines) {
            match child {
                Child::Block { name, lines } => {
                    let nested = format!("{section}/{name}");
                    if SECTIONS.contains(&nested.as_str()) {
                        self.read_section(&nested, inner(lines));
                    } else {
                        self.extra
                            .entry(section.to_string())
                            .or_default()
                            .extend(lines.iter().map(|raw| raw.trim_end().to_string()));
                    }
                }
                Child::Line(raw) => {
                    let line = raw.trim();
                    if line.starts_with("//") || line.starts_with("/-") {
                        self.extra
                            .entry(section.to_string())
                            .or_default()
                            .push(raw.trim_end().to_string());
                        continue;
                    }
                    let key = key_of(line).to_string();
                    has_state |= key == "state";
                    self.put(section, &key, line.to_string());
                }
            }
        }
        // a `mat {}` without `off` turns the mat on
        if section == "layout/mat" && !has_state {
            self.put(section, "state", String::from("on"));
        }
    }

    fn put(&mut self, section: &str, key: &str, line: String) {
        self.lines
            .insert((section.to_string(), key.to_string()), line);
    }

    /// The file's text: its header, its top-level settings, then each
    /// section with a setting, then what else it held.
    pub fn write(&self) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        for line in self.section_lines("") {
            out.push_str(line);
            out.push('\n');
        }
        for section in SECTIONS
            .iter()
            .filter(|s| !s.is_empty() && !s.contains('/'))
        {
            if self.used(section) {
                self.write_section(&mut out, section, 0);
            }
        }
        if !self.outside.is_empty() {
            out.push('\n');
            for raw in &self.outside {
                out.push_str(raw);
                out.push('\n');
            }
        }
        out
    }

    fn write_section(&self, out: &mut String, section: &str, depth: usize) {
        let pad = "    ".repeat(depth);
        let name = section.rsplit('/').next().unwrap_or(section);
        out.push_str(&format!("{pad}{name} {{\n"));
        for line in self.section_lines(section) {
            // the mat is on by its block alone: it has no `on`
            if section == "layout/mat" && line == "on" {
                continue;
            }
            out.push_str(&format!("{pad}    {line}\n"));
        }
        // kept as they were, their own indent included
        for raw in self.extra.get(section).into_iter().flatten() {
            out.push_str(raw);
            out.push('\n');
        }
        let prefix = format!("{section}/");
        for nested in SECTIONS.iter().filter(|s| {
            s.strip_prefix(&prefix)
                .is_some_and(|rest| !rest.contains('/'))
        }) {
            if self.used(nested) {
                self.write_section(out, nested, depth + 1);
            }
        }
        out.push_str(&format!("{pad}}}\n"));
    }

    fn section_lines(&self, section: &str) -> impl Iterator<Item = &String> {
        self.lines
            .iter()
            .filter(move |((s, _), _)| s == section)
            .map(|(_, line)| line)
    }

    /// Whether the section, or one inside it, holds anything.
    fn used(&self, section: &str) -> bool {
        let prefix = format!("{section}/");
        let holds = |s: &str| s == section || s.starts_with(&prefix);
        self.lines.keys().any(|(s, _)| holds(s))
            || self
                .extra
                .iter()
                .any(|(s, lines)| holds(s) && !lines.is_empty())
    }

    /// The line setting `key`, as written.
    pub fn line(&self, (section, key): Key) -> Option<&str> {
        self.lines
            .get(&(section.to_string(), key.to_string()))
            .map(String::as_str)
    }

    /// What `key` is set to: the line after its key, its comment left out;
    /// `on` or `off` for a state, and empty for a bare flag.
    pub fn value(&self, key: Key) -> Option<String> {
        let line = self.line(key)?;
        let code = code(line).trim();
        if key.1 == "state" {
            return Some(code.to_string());
        }
        Some(code.strip_prefix(key.1).unwrap_or(code).trim().to_string())
    }

    /// Sets `key` to `value`: `on`/`off` for a state, `true`/`false` or a
    /// value for anything else, written after the key.
    pub fn set(&mut self, key: Key, value: &str) {
        let line = if key.1 == "state" || value.is_empty() {
            if key.1 == "state" {
                value.to_string()
            } else {
                key.1.to_string()
            }
        } else {
            format!("{} {value}", key.1)
        };
        self.put(key.0, key.1, line);
    }

    /// Sets `key`'s line whole, a comment of its own included.
    pub fn set_line(&mut self, key: Key, line: String) {
        self.put(key.0, key.1, line);
    }

    /// Leaves `key` to `config.kdl` again.
    pub fn remove(&mut self, (section, key): Key) {
        self.lines.remove(&(section.to_string(), key.to_string()));
    }
}

/// niri's own default for `key`, when neither file sets it; none for one
/// with no value worth showing.
pub fn default(key: Key) -> Option<&'static str> {
    Some(match key {
        ("", "prefer-no-csd") => "false",
        ("layout", "gaps") => "16",
        ("layout", "center-focused-column") => "\"never\"",
        ("layout", "always-center-single-column") => "false",
        ("layout", "empty-workspace-above-first") => "false",
        ("layout", "default-column-width") => "{ proportion 0.5; }",
        ("layout/focus-ring", "state") => "on",
        ("layout/focus-ring", "width") => "4",
        ("layout/focus-ring", "active-color") => "\"#7fc8ff\"",
        ("layout/border", "state") => "off",
        ("layout/border", "width") => "4",
        ("layout/shadow", "state") => "off",
        ("layout/mat", "state") => "off",
        ("overview", "zoom") => "0.5",
        ("recent-windows", "state") => "on",
        ("gestures", "hot-corners") => "{ top-left; }",
        ("gestures", "mod-swipe") => "{ }",
        ("cursor", "xcursor-theme") => "\"default\"",
        ("cursor", "xcursor-size") => "24",
        ("cursor", "hide-when-typing") => "false",
        ("hotkey-overlay", "skip-at-startup") => "false",
        _ => return None,
    })
}

/// What's in force for `key`: ours, else `config.kdl`'s, else niri's.
pub fn effective(ours: &Sparse, theirs: &Sparse, key: Key) -> Option<String> {
    ours.value(key)
        .or_else(|| theirs.value(key))
        .or_else(|| default(key).map(String::from))
}

/// A flag's or state's value as a bool: a bare flag, `true` and `on` are on.
pub fn truthy(value: &str) -> bool {
    matches!(value, "" | "true" | "on")
}

// ───── the files ────────────────────────────────────────────────────────────

pub fn settings_file() -> PathBuf {
    beside_config(FILE)
}

/// Where the settings stand.
#[derive(Clone, Debug, PartialEq)]
pub enum Setup {
    /// `config.kdl` includes the file: ours, and `config.kdl`'s own.
    Ready { ours: Sparse, theirs: Sparse },
    /// `config.kdl` doesn't include it yet: its own settings, shown.
    NotIncluded { theirs: Sparse },
    /// No `config.kdl` to include it from.
    Missing,
}

pub fn setup() -> Setup {
    let Ok(config) = fs::read_to_string(niri_config()) else {
        return Setup::Missing;
    };
    let theirs = Sparse::read(&config);
    let file = settings_file();
    if niri_files::includes(&config, &file) {
        let ours = Sparse::read(&fs::read_to_string(&file).unwrap_or_default());
        return Setup::Ready { ours, theirs };
    }
    Setup::NotIncluded { theirs }
}

const NOTE: &str = "Emaki Settings writes this file; it comes last, so its settings win:";

/// Includes the file as `config.kdl`'s last line, as one commit when the
/// folder is in a git repo.
pub fn set_up() -> Result<(), String> {
    let config_path = niri_config();
    let config = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let file = settings_file();
    if niri_files::includes(&config, &file) {
        return Ok(());
    }
    let folder = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    if dirty(&folder, &config_path) {
        return Err(format!(
            "{} has uncommitted changes; commit or discard them first",
            config_path.display()
        ));
    }
    // the file first, so the include never names a missing one; one there
    // already, written by hand, is kept
    if !file.exists() {
        write_atomic(&file, &Sparse::default().write())?;
    }
    write_atomic(
        &config_path,
        &niri_files::append_include(&config, &file, NOTE),
    )?;
    commit(
        &folder,
        &[&config_path, &file],
        "include: settings.kdl (Emaki Settings)",
    )
}

/// The file as it is now, to put back if niri won't take a change.
pub fn current_text() -> Option<String> {
    fs::read_to_string(settings_file()).ok()
}

/// Writes `ours` (niri reads it again by itself) and commits it.
pub fn save(ours: &Sparse, what: &str) -> Result<(), String> {
    write_text(&ours.write(), &format!("settings: {what}"))
}

/// Puts back `text`, what the file held before a change niri refused.
pub fn restore(text: &str) -> Result<(), String> {
    write_text(text, "settings: back to what niri took")
}

fn write_text(text: &str, message: &str) -> Result<(), String> {
    let file = settings_file();
    write_atomic(&file, text)?;
    let folder = file.parent().map(Path::to_path_buf).unwrap_or_default();
    commit(&folder, &[&file], message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r##"prefer-no-csd
input {
    keyboard { numlock; }
}
layout {
    gaps 8
    center-focused-column "never"
    preset-column-widths {
        proportion 0.5
    }
    default-column-width { proportion 0.5; }
    focus-ring {
        off
        width 2
        active-color "#7fc8ff"
    }
    mat {
        color "#000000"
        behind-bars false
    }
    border {
        off
    }
}
/-cursor {
    xcursor-size 64
}
screenshot-path "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png"
binds {
    Mod+T { spawn "foot"; }
}
"##;

    #[test]
    fn config_kdls_own_settings_read() {
        let theirs = Sparse::read(CONFIG);
        assert_eq!(theirs.value(("", "prefer-no-csd")).as_deref(), Some(""));
        assert_eq!(theirs.value(("layout", "gaps")).as_deref(), Some("8"));
        assert_eq!(
            theirs.value(("layout", "default-column-width")).as_deref(),
            Some("{ proportion 0.5; }")
        );
        assert_eq!(
            theirs.value(("layout/focus-ring", "state")).as_deref(),
            Some("off")
        );
        assert_eq!(
            theirs.value(("layout/focus-ring", "width")).as_deref(),
            Some("2")
        );
        // a mat block without `off` is on
        assert_eq!(theirs.value(("layout/mat", "state")).as_deref(), Some("on"));
        // a slashdashed section isn't read
        assert_eq!(theirs.value(("cursor", "xcursor-size")), None);
        assert!(theirs.value(("", "screenshot-path")).is_some());
    }

    #[test]
    fn ours_wins_then_theirs_then_niris() {
        let theirs = Sparse::read(CONFIG);
        let mut ours = Sparse::default();
        assert_eq!(
            effective(&ours, &theirs, ("layout", "gaps")).as_deref(),
            Some("8")
        );
        ours.set(("layout", "gaps"), "16");
        assert_eq!(
            effective(&ours, &theirs, ("layout", "gaps")).as_deref(),
            Some("16")
        );
        assert_eq!(
            effective(&ours, &theirs, ("cursor", "xcursor-size")).as_deref(),
            Some("24")
        );
        ours.remove(("layout", "gaps"));
        assert_eq!(
            effective(&ours, &theirs, ("layout", "gaps")).as_deref(),
            Some("8")
        );
    }

    #[test]
    fn what_it_writes_it_reads_back_the_same() {
        let mut ours = Sparse::default();
        ours.set(("", "prefer-no-csd"), "false");
        ours.set(("", "screenshot-path"), "null");
        ours.set(("layout", "gaps"), "12");
        ours.set(("layout", "default-column-width"), "{}");
        ours.set(("layout/focus-ring", "state"), "on");
        ours.set_line(
            ("layout/focus-ring", "active-color"),
            String::from("active-color \"#0a84ff\" // follows the accent"),
        );
        ours.set(("layout/mat", "state"), "on");
        ours.set(("layout/shadow", "state"), "off");
        ours.set(("gestures", "hot-corners"), "{ top-right; }");
        ours.set(("gestures", "mod-swipe"), "{ off; }");
        ours.set(("cursor", "xcursor-size"), "32");
        ours.set(("cursor", "hide-when-typing"), "true");
        ours.set(("recent-windows", "state"), "off");
        let text = ours.write();
        assert!(
            text.contains("    mat {\n    }\n"),
            "the mat is on by its block: {text}"
        );
        assert!(
            text.contains("layout {\n") && text.contains("\n    gaps 12\n"),
            "{text}"
        );
        assert_eq!(Sparse::read(&text), ours);
        assert_eq!(
            Sparse::read(&text)
                .value(("layout/focus-ring", "active-color"))
                .as_deref(),
            Some("\"#0a84ff\"")
        );
        assert!(
            Sparse::default()
                .write()
                .trim()
                .lines()
                .all(|l| l.starts_with("//"))
        );
    }

    #[test]
    fn lines_it_doesnt_know_are_kept() {
        let text = format!(
            "{HEADER}\nlayout {{\n    // mine\n    struts {{\n        top 4\n    }}\n    gaps 4\n}}\n\nwindow-rule {{\n    opacity 0.9\n}}\n"
        );
        let mut read = Sparse::read(&text);
        read.set(("layout", "gaps"), "6");
        let written = read.write();
        assert!(written.contains("// mine"));
        assert!(written.contains("struts {"));
        assert!(written.contains("window-rule {\n    opacity 0.9\n}"));
        assert!(written.contains("gaps 6"));
        assert_eq!(written.matches("gaps").count(), 1);
    }

    #[test]
    fn this_is_the_files_one_writer() {
        let others = niri_files::others_naming(FILE, Path::new("src/niri_settings/kdl.rs"));
        assert!(others.is_empty(), "{FILE} is named in {others:?}");
    }

    /// Writes a copy of the live config, including a settings.kdl with every
    /// setting changed, into `$EMAKI_TEST_DIR`, for `niri validate -c`.
    #[test]
    #[ignore]
    fn write_for_niri_validate() {
        let dir = PathBuf::from(std::env::var("EMAKI_TEST_DIR").expect("EMAKI_TEST_DIR"));
        let config = fs::read_to_string(niri_config()).expect("config.kdl");
        let file = dir.join(FILE);
        let mut ours = Sparse::default();
        for (key, value) in [
            (("", "prefer-no-csd"), "false"),
            (("", "screenshot-path"), "null"),
            (("layout", "gaps"), "12"),
            (("layout", "center-focused-column"), "\"on-overflow\""),
            (("layout", "always-center-single-column"), "true"),
            (("layout", "empty-workspace-above-first"), "false"),
            (("layout", "default-column-width"), "{}"),
            (("layout/focus-ring", "state"), "on"),
            (("layout/focus-ring", "width"), "3"),
            (("layout/focus-ring", "active-color"), "\"#0a84ff\""),
            (("layout/border", "state"), "on"),
            (("layout/border", "width"), "2"),
            (("layout/shadow", "state"), "on"),
            (("layout/mat", "state"), "off"),
            (("overview", "zoom"), "0.4"),
            (("recent-windows", "state"), "off"),
            (("gestures", "hot-corners"), "{ top-right; bottom-right; }"),
            (("gestures", "mod-swipe"), "{ off; }"),
            (("cursor", "xcursor-theme"), "\"Adwaita\""),
            (("cursor", "xcursor-size"), "32"),
            (("cursor", "hide-when-typing"), "true"),
            (("cursor", "hide-after-inactive-ms"), "5000"),
            (("hotkey-overlay", "skip-at-startup"), "false"),
        ] {
            ours.set(key, value);
        }
        fs::write(&file, ours.write()).expect("settings.kdl");
        let moved = niri_files::append_include(&config, &file, NOTE);
        fs::write(dir.join("config.kdl"), moved).expect("config.kdl");
    }
}
