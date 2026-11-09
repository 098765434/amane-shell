//! The keyboard's, touchpad's and mouse's lasting settings: `input.kdl`,
//! beside niri's `config.kdl`, which includes it. This module is the
//! file's one writer (a test holds every other file to that). The file is
//! written whole each time and read back with a reader for what it
//! writes, never a KDL parser. Lines it doesn't know, at any depth, are
//! kept as they were and written back, so an edit by hand isn't lost.
//!
//! niri merges the `keyboard` section of an included file field by field,
//! but takes a `touchpad` or `mouse` section whole, dropping the one
//! before. So the file can't hold only the settings this page changes:
//! it owns the whole `input` block, and [`migrate`] moves `config.kdl`'s
//! own there once. The old block becomes comments, an `include` takes its
//! place, and both files are committed together when they live in a git
//! repo.

use std::fs;
use std::path::{Path, PathBuf};

use crate::niri_files::{
    self, Child, beside_config, braces, children, code, commit, dirty, niri_config, node_end,
    spans, write_atomic,
};

pub const FILE: &str = "input.kdl";

const HEADER: &str = "// Written by Emaki Settings (Input), which reads it back; lines it\n\
                      // doesn't know are kept. niri's config.kdl includes it.\n";

/// `keyboard { … }`. The layout (`xkb`) isn't set here: it stays the
/// NixOS setting General shows, and its block is kept as it was.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keyboard {
    /// Before a held key repeats, in ms; niri's default (600) when unset.
    pub repeat_delay: Option<u16>,
    /// Repeats a second; niri's default (25) when unset.
    pub repeat_rate: Option<u8>,
    pub numlock: bool,
    pub extra: Vec<String>,
}

/// `touchpad { … }` or `mouse { … }`. A mouse takes only some of the
/// settings: the page never sets the others on one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pointer {
    pub tap: bool,
    /// Off while typing.
    pub dwt: bool,
    pub natural_scroll: bool,
    /// From -1 to 1; libinput's default (0) when unset.
    pub accel_speed: Option<f64>,
    /// `adaptive` or `flat`.
    pub accel_profile: Option<String>,
    /// `button-areas` or `clickfinger`.
    pub click_method: Option<String>,
    /// `two-finger`, `edge`, `on-button-down` or `no-scroll`.
    pub scroll_method: Option<String>,
    pub left_handed: bool,
    /// A middle click by pressing left and right together.
    pub middle_emulation: bool,
    /// A touchpad off while a mouse is plugged in.
    pub disabled_on_external_mouse: bool,
    /// How far a scroll goes, 1 being libinput's; niri takes 0 to 100.
    pub scroll_factor: Option<f64>,
    pub extra: Vec<String>,
}

/// The whole file: its `input` block, then whatever else it holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputConfig {
    pub keyboard: Keyboard,
    pub touchpad: Pointer,
    pub mouse: Pointer,
    /// Focus goes to the window under the pointer.
    pub focus_follows_mouse: bool,
    /// The pointer jumps to a window focused from the keyboard.
    pub warp_mouse_to_focus: bool,
    /// The rest of the `input` block (`trackpoint`, `focus-follows-mouse`
    /// and the like), as it was.
    pub extra: Vec<String>,
    /// Lines outside the `input` block, as they were.
    pub outside: Vec<String>,
}

/// A block's lines without its first and last.
fn inner<'a>(lines: &'a [&'a str]) -> &'a [&'a str] {
    &lines[1..lines.len().saturating_sub(1)]
}

/// `name {}` on one line: an empty block of that name.
fn empty_block(line: &str) -> Option<&str> {
    let code = code(line).trim();
    let name = code.strip_suffix('}')?.trim_end().strip_suffix('{')?.trim();
    (!name.is_empty() && !name.contains(' ')).then_some(name)
}

fn owned(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|raw| raw.trim_end().to_string()).collect()
}

impl InputConfig {
    pub fn write_kdl(&self) -> String {
        let mut out = String::from(HEADER);
        out.push_str("\ninput {\n");

        out.push_str("    keyboard {\n");
        let mut line = |text: String| out.push_str(&format!("        {text}\n"));
        let keyboard = &self.keyboard;
        if let Some(ms) = keyboard.repeat_delay {
            line(format!("repeat-delay {ms}"));
        }
        if let Some(rate) = keyboard.repeat_rate {
            line(format!("repeat-rate {rate}"));
        }
        if keyboard.numlock {
            line(String::from("numlock"));
        }
        let written: Vec<&str> = [
            ("repeat-delay", keyboard.repeat_delay.is_some()),
            ("repeat-rate", keyboard.repeat_rate.is_some()),
            ("numlock", keyboard.numlock),
        ]
        .into_iter()
        .filter_map(|(key, set)| set.then_some(key))
        .collect();
        for raw in kept(&keyboard.extra, &written) {
            out.push_str(raw);
            out.push('\n');
        }
        out.push_str("    }\n");

        write_pointer(&mut out, "touchpad", &self.touchpad);
        write_pointer(&mut out, "mouse", &self.mouse);
        let flags = [
            ("focus-follows-mouse", self.focus_follows_mouse),
            ("warp-mouse-to-focus", self.warp_mouse_to_focus),
        ];
        for (key, on) in flags {
            if on {
                out.push_str(&format!("    {key}\n"));
            }
        }
        let written: Vec<&str> = flags
            .iter()
            .filter_map(|(key, on)| on.then_some(*key))
            .collect();
        for raw in kept(&self.extra, &written) {
            out.push_str(raw);
            out.push('\n');
        }
        out.push_str("}\n");
        if !self.outside.is_empty() {
            out.push('\n');
            for raw in &self.outside {
                out.push_str(raw);
                out.push('\n');
            }
        }
        out
    }

    /// What [`write_kdl`](Self::write_kdl) wrote, or an `input` block as
    /// `config.kdl` had it, with every line it doesn't know kept.
    pub fn read_kdl(text: &str) -> Self {
        let mut config = InputConfig::default();
        let lines: Vec<&str> = text.lines().collect();
        let mut at = 0;
        while at < lines.len() {
            let line = lines[at].trim();
            if line.is_empty() || HEADER.lines().any(|header| header == line) {
                at += 1;
                continue;
            }
            let end = node_end(&lines, at);
            let node = &lines[at..=end];
            if code(line).trim() == "input {" {
                config.read_input(inner(node));
            } else {
                // nodes apart as they were; a comment stays on its node
                let after_comment = config.outside.last().is_some_and(|last| {
                    last.trim_start().starts_with("//") && lines[at - 1].trim().starts_with("//")
                });
                if !config.outside.is_empty() && !after_comment {
                    config.outside.push(String::new());
                }
                config.outside.extend(owned(node));
            }
            at = end + 1;
        }
        config
    }

    fn read_input(&mut self, inner_lines: &[&str]) {
        for child in children(inner_lines) {
            match child {
                Child::Block {
                    name: "keyboard",
                    lines,
                } => self.read_keyboard(inner(lines)),
                Child::Block {
                    name: "touchpad",
                    lines,
                } => read_pointer(&mut self.touchpad, inner(lines)),
                Child::Block {
                    name: "mouse",
                    lines,
                } => read_pointer(&mut self.mouse, inner(lines)),
                Child::Block { lines, .. } => self.extra.extend(owned(lines)),
                // `touchpad {}` sets nothing, which is what's written for it
                Child::Line(raw)
                    if matches!(empty_block(raw), Some("keyboard" | "touchpad" | "mouse")) => {}
                // bare flags only: one with options of its own is kept as it was
                Child::Line(raw) if raw.trim() == "focus-follows-mouse" => {
                    self.focus_follows_mouse = true
                }
                Child::Line(raw) if raw.trim() == "warp-mouse-to-focus" => {
                    self.warp_mouse_to_focus = true
                }
                Child::Line(raw) => self.extra.push(raw.trim_end().to_string()),
            }
        }
    }

    fn read_keyboard(&mut self, inner_lines: &[&str]) {
        let keyboard = &mut self.keyboard;
        for child in children(inner_lines) {
            let raw = match child {
                Child::Block { lines, .. } => {
                    keyboard.extra.extend(owned(lines));
                    continue;
                }
                Child::Line(raw) => raw,
            };
            let line = code(raw).trim();
            // a known setting with a comment of its own is kept as it was
            if line != raw.trim() || !read_key(keyboard, line) {
                keyboard.extra.push(raw.trim_end().to_string());
            }
        }
    }
}

/// Reads one line of a `keyboard` block; false when it's no setting this
/// file writes.
fn read_key(keyboard: &mut Keyboard, line: &str) -> bool {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
        "repeat-delay" => value
            .parse()
            .map(|v| keyboard.repeat_delay = Some(v))
            .is_ok(),
        "repeat-rate" => value
            .parse()
            .map(|v| keyboard.repeat_rate = Some(v))
            .is_ok(),
        "numlock" if value.is_empty() => {
            keyboard.numlock = true;
            true
        }
        _ => false,
    }
}

/// `extra`'s lines but those setting one of `written`, so a setting the
/// page has changed isn't there twice (a duplicate niri refuses).
/// Only lines of the block itself go, never one in a nested block.
fn kept<'a>(extra: &'a [String], written: &'a [&str]) -> impl Iterator<Item = &'a String> {
    let mut depth = 0;
    extra.iter().filter(move |raw| {
        let code = code(raw);
        let top = depth == 0;
        depth += braces(code);
        let key = code.split_whitespace().next().unwrap_or("");
        !(top && written.contains(&key))
    })
}

fn read_pointer(pointer: &mut Pointer, inner_lines: &[&str]) {
    for child in children(inner_lines) {
        let raw = match child {
            Child::Block { lines, .. } => {
                pointer.extra.extend(owned(lines));
                continue;
            }
            Child::Line(raw) => raw,
        };
        let line = code(raw).trim();
        // a known setting with a comment of its own is kept as it was
        if line != raw.trim() || !read_setting(pointer, line) {
            pointer.extra.push(raw.trim_end().to_string());
        }
    }
}

/// Reads one line of a `touchpad` or `mouse` block; false when it's no
/// setting this file writes.
fn read_setting(pointer: &mut Pointer, line: &str) -> bool {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    let word = || niri_files::unquote(value);
    match key {
        "tap" if value.is_empty() => pointer.tap = true,
        "dwt" if value.is_empty() => pointer.dwt = true,
        "natural-scroll" if value.is_empty() => pointer.natural_scroll = true,
        "left-handed" if value.is_empty() => pointer.left_handed = true,
        "middle-emulation" if value.is_empty() => pointer.middle_emulation = true,
        "disabled-on-external-mouse" if value.is_empty() => {
            pointer.disabled_on_external_mouse = true
        }
        // one factor for both directions; one per direction is kept as it was
        "scroll-factor" => match value.parse::<f64>() {
            Ok(factor) if (0.0..=100.0).contains(&factor) => pointer.scroll_factor = Some(factor),
            _ => return false,
        },
        "accel-speed" => match value.parse::<f64>() {
            Ok(speed) if (-1.0..=1.0).contains(&speed) => pointer.accel_speed = Some(speed),
            _ => return false,
        },
        "accel-profile" => match word() {
            Some(profile) => pointer.accel_profile = Some(profile),
            None => return false,
        },
        "click-method" => match word() {
            Some(method) => pointer.click_method = Some(method),
            None => return false,
        },
        "scroll-method" => match word() {
            Some(method) => pointer.scroll_method = Some(method),
            None => return false,
        },
        _ => return false,
    }
    true
}

fn write_pointer(out: &mut String, name: &str, pointer: &Pointer) {
    out.push_str(&format!("    {name} {{\n"));
    let mut line = |text: String| out.push_str(&format!("        {text}\n"));
    if pointer.tap {
        line(String::from("tap"));
    }
    if pointer.dwt {
        line(String::from("dwt"));
    }
    if pointer.natural_scroll {
        line(String::from("natural-scroll"));
    }
    if pointer.left_handed {
        line(String::from("left-handed"));
    }
    if pointer.middle_emulation {
        line(String::from("middle-emulation"));
    }
    if pointer.disabled_on_external_mouse {
        line(String::from("disabled-on-external-mouse"));
    }
    if let Some(factor) = pointer.scroll_factor {
        line(format!("scroll-factor {}", factor_text(factor)));
    }
    if let Some(speed) = pointer.accel_speed {
        line(format!("accel-speed {}", speed_text(speed)));
    }
    let words = [
        ("accel-profile", &pointer.accel_profile),
        ("click-method", &pointer.click_method),
        ("scroll-method", &pointer.scroll_method),
    ];
    for (key, value) in words {
        if let Some(value) = value {
            line(format!("{key} {}", niri_files::quote(value)));
        }
    }
    let mut written: Vec<&str> = [
        ("tap", pointer.tap),
        ("dwt", pointer.dwt),
        ("natural-scroll", pointer.natural_scroll),
        ("left-handed", pointer.left_handed),
        ("middle-emulation", pointer.middle_emulation),
        (
            "disabled-on-external-mouse",
            pointer.disabled_on_external_mouse,
        ),
        ("scroll-factor", pointer.scroll_factor.is_some()),
        ("accel-speed", pointer.accel_speed.is_some()),
    ]
    .into_iter()
    .filter_map(|(key, set)| set.then_some(key))
    .collect();
    written.extend(words.iter().filter(|(_, v)| v.is_some()).map(|(k, _)| *k));
    for raw in kept(&pointer.extra, &written) {
        out.push_str(raw);
        out.push('\n');
    }
    out.push_str("    }\n");
}

/// A scroll factor as niri takes it: `1.0`, `0.25`, `1.75`.
fn factor_text(factor: f64) -> String {
    let text = format!("{:.2}", factor.clamp(0.0, 100.0));
    let text = text.trim_end_matches('0');
    if text.ends_with('.') {
        format!("{text}0")
    } else {
        text.to_string()
    }
}

/// A speed as niri takes it, two places at most: `0.0`, `-0.35`, `1.0`.
fn speed_text(speed: f64) -> String {
    let text = format!("{:.2}", speed.clamp(-1.0, 1.0));
    let text = text.trim_end_matches('0');
    if text.ends_with('.') {
        format!("{text}0")
    } else {
        text.to_string()
    }
}

// ───── the files ────────────────────────────────────────────────────────────

pub fn input_file() -> PathBuf {
    beside_config(FILE)
}

/// Where the input settings stand.
#[derive(Clone, Debug, PartialEq)]
pub enum Setup {
    /// `config.kdl` includes the file.
    Ready(InputConfig),
    /// `config.kdl` still has its own `input` block, read here, so the
    /// page shows it and can't change it until it moves.
    Unmoved(InputConfig),
    /// No `config.kdl` to include it from.
    Missing,
}

pub fn setup() -> Setup {
    let Ok(config) = fs::read_to_string(niri_config()) else {
        return Setup::Missing;
    };
    let file = input_file();
    if niri_files::includes(&config, &file) {
        let text = fs::read_to_string(&file).unwrap_or_default();
        return Setup::Ready(InputConfig::read_kdl(&text));
    }
    Setup::Unmoved(own_block(&config))
}

/// `config.kdl`'s own `input` blocks, read as one.
fn own_block(config: &str) -> InputConfig {
    let lines: Vec<&str> = config.lines().collect();
    let text: Vec<&str> = spans(config, "input")
        .into_iter()
        .flat_map(|(first, last)| lines[first..=last].iter().copied())
        .collect();
    InputConfig::read_kdl(&text.join("\n"))
}

const NOTE: &str = "Input is set in Emaki Settings, which writes this file:";

/// `config.kdl` with its `input` blocks commented out and the include
/// where the first one was.
pub fn moved(config: &str, file: &Path) -> String {
    niri_files::moved(config, &spans(config, "input"), file, NOTE)
}

/// Moves `config.kdl`'s `input` block into `input.kdl` and includes it,
/// as one commit when the folder is in a git repo.
pub fn migrate() -> Result<InputConfig, String> {
    let config_path = niri_config();
    let config = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let file = input_file();
    if niri_files::includes(&config, &file) {
        return Ok(InputConfig::read_kdl(
            &fs::read_to_string(&file).unwrap_or_default(),
        ));
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
    let input = own_block(&config);
    // the new file first, so the include never names a missing one
    write_atomic(&file, &input.write_kdl())?;
    write_atomic(&config_path, &moved(&config, &file))?;
    commit(
        &folder,
        &[&config_path, &file],
        "move: input from niri/config.kdl to input.kdl (Emaki Settings)",
    )?;
    Ok(input)
}

/// The file as it is now, to put back if niri won't take a change.
pub fn current_text() -> Option<String> {
    fs::read_to_string(input_file()).ok()
}

/// Writes `input` (niri reads it again by itself) and commits it.
pub fn save(input: &InputConfig, what: &str) -> Result<(), String> {
    write_text(&input.write_kdl(), &format!("input: {what}"))
}

/// Puts back `text`, what the file held before a change niri refused.
pub fn restore(text: &str) -> Result<(), String> {
    write_text(text, "input: back to what niri took")
}

fn write_text(text: &str, message: &str) -> Result<(), String> {
    let file = input_file();
    write_atomic(&file, text)?;
    let folder = file.parent().map(Path::to_path_buf).unwrap_or_default();
    commit(&folder, &[&file], message)
}

#[cfg(test)]
mod tests {
    use super::*;

    // config.kdl's block as it is on this machine
    const OWN: &str = "prefer-no-csd

input {
    keyboard {
        xkb {
            // Empty: niri fetches xkb from org.freedesktop.locale1,
            // which NixOS drives via services.xserver.xkb (layout \"no\").
        }
        numlock
    }
    touchpad {
        tap
        natural-scroll
    }
    mouse {}
    trackpoint {}
}

layout {
    gaps 8
}
";

    #[test]
    fn the_configs_own_block_reads() {
        let input = own_block(OWN);
        assert!(input.keyboard.numlock);
        assert_eq!(input.keyboard.repeat_delay, None);
        assert_eq!(input.keyboard.extra.len(), 4, "{:?}", input.keyboard.extra);
        assert!(input.keyboard.extra[0].trim() == "xkb {");
        assert!(input.touchpad.tap && input.touchpad.natural_scroll);
        assert!(!input.touchpad.dwt);
        assert_eq!(input.mouse, Pointer::default());
        assert_eq!(input.extra, ["    trackpoint {}"]);
        assert!(input.outside.is_empty(), "{:?}", input.outside);
    }

    #[test]
    fn what_it_writes_it_reads_back_the_same() {
        let mut input = own_block(OWN);
        input.keyboard.repeat_delay = Some(300);
        input.keyboard.repeat_rate = Some(40);
        input.touchpad.dwt = true;
        input.touchpad.accel_speed = Some(-0.35);
        input.touchpad.click_method = Some(String::from("clickfinger"));
        input.touchpad.scroll_method = Some(String::from("edge"));
        input.mouse.accel_profile = Some(String::from("flat"));
        input.mouse.accel_speed = Some(1.0);
        input.mouse.left_handed = true;
        input.mouse.middle_emulation = true;
        input.mouse.scroll_factor = Some(0.75);
        input.touchpad.disabled_on_external_mouse = true;
        input.focus_follows_mouse = true;
        input.warp_mouse_to_focus = true;
        let text = input.write_kdl();
        assert!(text.contains("        scroll-factor 0.75\n"), "{text}");
        assert!(text.contains("    focus-follows-mouse\n"), "{text}");
        assert!(text.contains("        accel-speed -0.35\n"), "{text}");
        assert!(
            text.contains("        click-method \"clickfinger\"\n"),
            "{text}"
        );
        assert!(
            text.contains("            // Empty: niri fetches xkb"),
            "{text}"
        );
        assert_eq!(InputConfig::read_kdl(&text), input);
        assert_eq!(InputConfig::read_kdl(&text).write_kdl(), text);
    }

    #[test]
    fn lines_it_doesnt_know_are_kept() {
        let by_hand = "input {
    keyboard {
        repeat-rate 30 // quick
        track-layout \"window\"
    }
    touchpad {
        tap
        scroll-factor 0.5
        tap-button-map \"left-middle-right\"
    }
    focus-follows-mouse max-scroll-amount=\"10%\"
}

// mine
cursor {
    xcursor-size 32
}
";
        let input = InputConfig::read_kdl(by_hand);
        assert_eq!(
            input.keyboard.repeat_rate, None,
            "its comment is kept with it"
        );
        assert!(input.touchpad.tap);
        let written = input.write_kdl();
        for line in by_hand.lines().filter(|line| !line.is_empty()) {
            assert!(
                written.contains(line.trim()),
                "lost {line:?} from:\n{written}"
            );
        }
        assert_eq!(InputConfig::read_kdl(&written), input);
    }

    #[test]
    fn a_setting_changed_here_isnt_written_twice() {
        let mut input = InputConfig::read_kdl(
            "input {\n    keyboard {\n        repeat-rate 30 // quick\n        xkb {\n            repeat-rate 1\n        }\n    }\n}\n",
        );
        input.keyboard.repeat_rate = Some(40);
        let text = input.write_kdl();
        assert_eq!(text.matches("\n        repeat-rate").count(), 1, "{text}");
        assert!(text.contains("        repeat-rate 40\n"), "{text}");
        // a nested block keeps its lines whatever they say
        assert!(text.contains("            repeat-rate 1\n"), "{text}");
    }

    #[test]
    fn moving_comments_the_block_and_includes_the_file() {
        let file = Path::new("/home/me/nixos-config/niri/input.kdl");
        let text = moved(OWN, file);
        assert!(text.contains(
            "include optional=true \"/home/me/nixos-config/niri/input.kdl\"\n// input {"
        ));
        assert!(spans(&text, "input").is_empty());
        assert!(niri_files::includes(&text, file));
        assert!(text.ends_with("gaps 8\n}\n"));
    }

    #[test]
    fn speeds_print_short() {
        assert_eq!(speed_text(0.0), "0.0");
        assert_eq!(speed_text(-0.35), "-0.35");
        assert_eq!(speed_text(1.0), "1.0");
        assert_eq!(speed_text(0.5), "0.5");
        assert_eq!(speed_text(3.0), "1.0");
    }

    #[test]
    fn this_is_the_files_one_writer() {
        let named = niri_files::others_naming(FILE, Path::new("src/input/kdl.rs"));
        assert!(
            named.is_empty(),
            "only input/kdl.rs names {FILE}: {named:#?}"
        );
    }
}

#[cfg(test)]
mod niri_takes_it {
    use super::*;

    /// Writes a moved copy of this machine's config and an input.kdl with
    /// every setting changed into `$EMAKI_TEST_DIR`, for `niri validate`.
    #[test]
    #[ignore = "writes into $EMAKI_TEST_DIR for niri validate"]
    fn write_for_niri_validate() {
        let dir = PathBuf::from(std::env::var("EMAKI_TEST_DIR").unwrap());
        let config = fs::read_to_string(niri_config()).unwrap();
        let file = dir.join(FILE);
        let mut input = own_block(&config);
        input.keyboard.repeat_delay = Some(300);
        input.keyboard.repeat_rate = Some(40);
        input.touchpad.dwt = true;
        input.touchpad.accel_speed = Some(-0.3);
        input.touchpad.accel_profile = Some(String::from("flat"));
        input.touchpad.click_method = Some(String::from("clickfinger"));
        input.touchpad.scroll_method = Some(String::from("edge"));
        input.mouse.natural_scroll = true;
        input.mouse.accel_speed = Some(0.5);
        input.mouse.accel_profile = Some(String::from("adaptive"));
        fs::write(&file, input.write_kdl()).unwrap();
        fs::write(dir.join("config.kdl"), moved(&config, &file)).unwrap();
    }
}
