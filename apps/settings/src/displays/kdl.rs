//! The displays' lasting settings: `displays.kdl`, beside niri's
//! `config.kdl`, which includes it. This module is the file's one writer
//! (a test holds every other file to that): it's written whole each time
//! and read back with a reader for what it writes, never a KDL parser.
//! Lines it doesn't know, in a block or between them, are kept as they
//! were and written back, so an edit by hand isn't lost.
//!
//! niri uses the first `output` block that matches, so the blocks
//! `config.kdl` had of its own are moved here once ([`migrate`]): they
//! become comments, an `include` takes the first one's place, and both
//! files are committed together when they live in a git repo.

use std::fs;
use std::path::{Path, PathBuf};

use super::outputs::{Transform, TransformExt};
use crate::system::nixos::run;

pub const FILE: &str = "displays.kdl";

/// How variable refresh is set for an output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Vrr {
    #[default]
    Off,
    /// Only while a window a rule names shows on it.
    OnDemand,
    Always,
}

/// One `output` block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutputConfig {
    /// What niri matches: a connector (`eDP-1`) or `make model serial`.
    pub name: String,
    pub off: bool,
    /// Width, height and refresh in Hz.
    pub mode: Option<(u16, u16, f64)>,
    /// A mode the monitor doesn't list.
    pub custom: bool,
    pub scale: Option<f64>,
    pub transform: Transform,
    pub position: Option<(i32, i32)>,
    pub vrr: Vrr,
    pub focus_at_startup: bool,
    /// HDR10, when the display takes it.
    pub hdr: bool,
    /// SDR white in HDR mode, in nits; niri's default (203) when unset.
    pub sdr_brightness: Option<f64>,
    /// `#rrggbb` behind the workspaces in the overview.
    pub backdrop: Option<String>,
    /// Lines in the block this reader doesn't know, as they were (a nested
    /// block whole), written back after the ones it does.
    pub extra: Vec<String>,
}

/// The whole file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Displays {
    pub outputs: Vec<OutputConfig>,
    /// Apps whose windows turn on-demand variable refresh on.
    pub vrr_apps: Vec<String>,
    /// Everything else between the blocks (comments, other nodes), as it
    /// was, written back after them.
    pub extra: Vec<String>,
}

const HEADER: &str = "// Written by Shell Settings (Displays), which reads it back; lines it\n\
                      // doesn't know are kept. niri's config.kdl includes it.\n";

/// The header an older Settings wrote, read as the header too.
const OLD_HEADER: [&str; 2] = [
    "// Written by Shell Settings (Displays), which reads it back: edits by",
    "// hand may be lost. niri's config.kdl includes it.",
];

const VRR_COMMENT: &str = "// on-demand variable refresh turns on for these apps' windows";

impl Displays {
    pub fn output(&self, name: &str) -> Option<&OutputConfig> {
        self.outputs.iter().find(|output| output.name == name)
    }

    /// The block for `name`, made if there's none yet.
    pub fn output_mut(&mut self, name: &str) -> &mut OutputConfig {
        let index = match self.outputs.iter().position(|output| output.name == name) {
            Some(index) => index,
            None => {
                self.outputs.push(OutputConfig {
                    name: name.to_string(),
                    ..OutputConfig::default()
                });
                self.outputs.len() - 1
            }
        };
        &mut self.outputs[index]
    }

    pub fn write_kdl(&self) -> String {
        let mut out = String::from(HEADER);
        for output in &self.outputs {
            out.push('\n');
            out.push_str(&format!("output {} {{\n", quote(&output.name)));
            let mut line = |text: String| out.push_str(&format!("    {text}\n"));
            if output.off {
                line(String::from("off"));
            }
            if let Some((width, height, hz)) = output.mode {
                let custom = if output.custom { "custom=true " } else { "" };
                line(format!("mode {custom}\"{width}x{height}@{hz:.3}\""));
            }
            if let Some(scale) = output.scale {
                line(format!("scale {}", number(scale)));
            }
            line(format!("transform {}", quote(output.transform.config())));
            if let Some((x, y)) = output.position {
                line(format!("position x={x} y={y}"));
            }
            match output.vrr {
                Vrr::Off => {}
                Vrr::OnDemand => line(String::from("variable-refresh-rate on-demand=true")),
                Vrr::Always => line(String::from("variable-refresh-rate")),
            }
            if output.focus_at_startup {
                line(String::from("focus-at-startup"));
            }
            if output.hdr {
                line(String::from("hdr"));
            }
            if let Some(nits) = output.sdr_brightness {
                line(format!("sdr-brightness {}", nits.round()));
            }
            if let Some(color) = &output.backdrop {
                line(format!("backdrop-color {}", quote(color)));
            }
            for raw in &output.extra {
                out.push_str(raw);
                out.push('\n');
            }
            out.push_str("}\n");
        }
        if !self.vrr_apps.is_empty() {
            out.push_str("\n// on-demand variable refresh turns on for these apps' windows\n");
            out.push_str("window-rule {\n");
            for app in &self.vrr_apps {
                out.push_str(&format!(
                    "    match app-id={}\n",
                    quote(&format!("^{}$", regex_escape(app)))
                ));
            }
            out.push_str("    variable-refresh-rate true\n}\n");
        }
        if !self.extra.is_empty() {
            out.push('\n');
            for raw in &self.extra {
                out.push_str(raw);
                out.push('\n');
            }
        }
        out
    }

    /// What [`write_kdl`](Self::write_kdl) wrote, with every line it
    /// doesn't know kept in [`extra`](Self::extra) or the block's own.
    pub fn read_kdl(text: &str) -> Self {
        let mut displays = Displays::default();
        let lines: Vec<&str> = text.lines().collect();
        let mut at = 0;
        while at < lines.len() {
            let raw = lines[at].trim_end();
            let line = raw.trim();
            if line.is_empty()
                || line == VRR_COMMENT
                || OLD_HEADER.contains(&line)
                || HEADER.lines().any(|header| header == line)
            {
                at += 1;
                continue;
            }
            // a node and the lines its braces span
            let end = node_end(&lines, at);
            let node = &lines[at..=end];
            if line.starts_with("output ") && line.ends_with('{') {
                displays.outputs.push(read_output(node));
            } else if let Some(apps) = vrr_rule(node) {
                displays.vrr_apps.extend(apps);
            } else {
                // nodes apart as they were; a comment stays on its node
                let after_comment = displays.extra.last().is_some_and(|last| {
                    last.trim_start().starts_with("//") && lines[at - 1].trim().starts_with("//")
                });
                if !displays.extra.is_empty() && !after_comment {
                    displays.extra.push(String::new());
                }
                displays
                    .extra
                    .extend(node.iter().map(|raw| raw.trim_end().to_string()));
            }
            at = end + 1;
        }
        displays
    }
}

/// The last line of the node that starts at `start`: where its braces close.
fn node_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0;
    for (index, line) in lines.iter().enumerate().skip(start) {
        depth += braces(line.split("//").next().unwrap_or(""));
        if depth <= 0 {
            return index;
        }
    }
    lines.len() - 1
}

/// The apps of a rule this file writes: only `match app-id=` lines and the
/// one setting. Any other rule is someone else's and kept as it is.
fn vrr_rule(node: &[&str]) -> Option<Vec<String>> {
    let (first, rest) = node.split_first()?;
    let (last, inner) = rest.split_last()?;
    if first.trim() != "window-rule {" || last.trim() != "}" {
        return None;
    }
    let mut apps = Vec::new();
    let mut set = false;
    for line in inner {
        let line = line.trim();
        if line == "variable-refresh-rate true" {
            set = true;
        } else {
            let regex = unquote(line.strip_prefix("match app-id=")?)?;
            apps.push(regex_unescape(
                regex.trim_start_matches('^').trim_end_matches('$'),
            ));
        }
    }
    set.then_some(apps)
}

/// One `output` block, from its first line to its last.
fn read_output(node: &[&str]) -> OutputConfig {
    let name = node[0].trim().trim_start_matches("output ");
    let mut output = OutputConfig {
        name: unquote(name.trim_end_matches('{').trim()).unwrap_or_default(),
        ..OutputConfig::default()
    };
    let inner = &node[1..node.len().saturating_sub(1)];
    let mut at = 0;
    while at < inner.len() {
        let raw = inner[at].trim_end();
        let line = raw.trim();
        // a nested block (a layout, hot corners): kept whole
        if braces(line.split("//").next().unwrap_or("")) > 0 {
            let end = node_end(inner, at);
            output
                .extra
                .extend(inner[at..=end].iter().map(|raw| raw.trim_end().to_string()));
            at = end + 1;
            continue;
        }
        at += 1;
        if line.is_empty() {
            continue;
        }
        if !read_setting(&mut output, line) {
            output.extra.push(raw.to_string());
        }
    }
    output
}

/// Reads one line of an `output` block into it; false when it's no
/// setting this file writes.
fn read_setting(output: &mut OutputConfig, line: &str) -> bool {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
        "off" if value.is_empty() => output.off = true,
        "mode" => {
            let quoted = value.rsplit_once(' ').map_or(value, |(_, last)| last);
            let Some(mode) = unquote(quoted).and_then(|mode| parse_mode(&mode)) else {
                return false;
            };
            output.custom = value.contains("custom=true");
            output.mode = Some(mode);
        }
        "scale" => match value.parse() {
            Ok(scale) => output.scale = Some(scale),
            Err(_) => return false,
        },
        "transform" => {
            let Some(transform) = unquote(value)
                .and_then(|name| Transform::ALL.into_iter().find(|t| t.config() == name))
            else {
                return false;
            };
            output.transform = transform;
        }
        "position" => {
            let field = |name: &str| {
                value
                    .split_whitespace()
                    .find_map(|part| part.strip_prefix(name))
                    .and_then(|v| v.parse().ok())
            };
            let Some(position) = field("x=").zip(field("y=")) else {
                return false;
            };
            output.position = Some(position);
        }
        "variable-refresh-rate" if value.is_empty() => output.vrr = Vrr::Always,
        "variable-refresh-rate" if value == "on-demand=true" => output.vrr = Vrr::OnDemand,
        "focus-at-startup" if value.is_empty() => output.focus_at_startup = true,
        "hdr" if value.is_empty() => output.hdr = true,
        "sdr-brightness" => match value.parse() {
            Ok(nits) => output.sdr_brightness = Some(nits),
            Err(_) => return false,
        },
        "backdrop-color" => match unquote(value) {
            Some(color) => output.backdrop = Some(color),
            None => return false,
        },
        _ => return false,
    }
    true
}

/// `2880x1800@120.000`, the refresh optional.
fn parse_mode(text: &str) -> Option<(u16, u16, f64)> {
    let (size, hz) = text.split_once('@').unwrap_or((text, "0"));
    let (width, height) = size.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?, hz.parse().ok()?))
}

/// A scale as short as it reads: `2`, `1.75`, `1.333333`.
fn number(value: f64) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.contains('.') {
        text.to_string()
    } else {
        format!("{text}.0")
    }
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn unquote(text: &str) -> Option<String> {
    let inner = text.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn regex_escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn regex_unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ───── the files ────────────────────────────────────────────────────────────

/// niri's own config, where its symlink leads (into the nixos-config repo
/// here), so `displays.kdl` lands beside the real file.
pub fn niri_config() -> PathBuf {
    let link = amane_kit::paths::config().join("niri/config.kdl");
    fs::canonicalize(&link).unwrap_or(link)
}

pub fn displays_file() -> PathBuf {
    niri_config().with_file_name(FILE)
}

/// The line `config.kdl` includes the file with. The path is absolute:
/// niri reads includes beside the config's symlink, not where it leads.
pub fn include_line(file: &Path) -> String {
    format!("include optional=true {}", quote(&file.to_string_lossy()))
}

/// Where the displays' settings stand.
#[derive(Clone, Debug, PartialEq)]
pub enum Setup {
    /// `config.kdl` includes the file.
    Ready(Displays),
    /// `config.kdl` still has its own blocks, these, so the page can't
    /// change anything until they move.
    Unmoved(Vec<Block>),
    /// No `config.kdl` to include it from.
    Missing,
}

/// An `output` block in `config.kdl`: the lines it spans, from 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub first: usize,
    pub last: usize,
    pub output: OutputConfig,
}

pub fn setup() -> Setup {
    let Ok(config) = fs::read_to_string(niri_config()) else {
        return Setup::Missing;
    };
    let file = displays_file();
    if includes(&config, &file) {
        let text = fs::read_to_string(&file).unwrap_or_default();
        return Setup::Ready(Displays::read_kdl(&text));
    }
    Setup::Unmoved(blocks(&config))
}

fn includes(config: &str, file: &Path) -> bool {
    let wanted = include_line(file);
    config.lines().any(|line| line.trim() == wanted)
}

/// The top-level `output` blocks that aren't commented out.
pub fn blocks(config: &str) -> Vec<Block> {
    let lines: Vec<&str> = config.lines().collect();
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if depth == 0 && code.starts_with("output ") {
            start = Some(index);
        }
        depth += braces(code);
        if depth == 0
            && let Some(first) = start.take()
        {
            let text = lines[first..=index].join("\n");
            if let Some(output) = Displays::read_kdl(&text).outputs.into_iter().next() {
                found.push(Block {
                    first,
                    last: index,
                    output,
                });
            }
        }
    }
    found
}

/// How much deeper a line goes, braces in strings aside.
fn braces(code: &str) -> i32 {
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for c in code.chars() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// `config.kdl` with its blocks commented out and the include where the
/// first one was.
pub fn moved(config: &str, blocks: &[Block], file: &Path) -> String {
    let mut out = Vec::new();
    for (index, line) in config.lines().enumerate() {
        if blocks.first().is_some_and(|block| block.first == index) {
            out.push(String::from(
                "// Displays are set in Shell Settings, which writes this file:",
            ));
            out.push(include_line(file));
        }
        if blocks
            .iter()
            .any(|block| (block.first..=block.last).contains(&index))
        {
            out.push(format!("// {line}"));
        } else {
            out.push(line.to_string());
        }
    }
    let mut text = out.join("\n");
    if config.ends_with('\n') {
        text.push('\n');
    }
    if blocks.is_empty() {
        text.push_str(&format!(
            "\n// Displays are set in Shell Settings, which writes this file:\n{}\n",
            include_line(file)
        ));
    }
    text
}

/// Moves `config.kdl`'s blocks into `displays.kdl` and includes it, as one
/// commit when the folder is in a git repo.
pub fn migrate() -> Result<Displays, String> {
    let config_path = niri_config();
    let config = fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let file = displays_file();
    if includes(&config, &file) {
        return Ok(Displays::read_kdl(
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

    let found = blocks(&config);
    let displays = Displays {
        outputs: found.iter().map(|block| block.output.clone()).collect(),
        ..Displays::default()
    };
    // the new file first, so the include never names a missing one
    write_atomic(&file, &displays.write_kdl())?;
    write_atomic(&config_path, &moved(&config, &found, &file))?;
    commit(
        &folder,
        &[&config_path, &file],
        "move: displays from niri/config.kdl to displays.kdl (Shell Settings)",
    )?;
    Ok(displays)
}

/// Writes `displays` (niri reads it again by itself) and commits it.
pub fn save(displays: &Displays, what: &str) -> Result<(), String> {
    let file = displays_file();
    write_atomic(&file, &displays.write_kdl())?;
    let folder = file.parent().map(Path::to_path_buf).unwrap_or_default();
    commit(&folder, &[&file], &format!("displays: {what}"))
}

/// Into place by a rename, so niri never reads half a file; the text is on
/// disk before the rename and the rename after it, or a crash in between can
/// leave the file empty (btrfs and ext4 both may), which loses every
/// display's settings.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write as _;

    let temp = path.with_extension("kdl.tmp");
    let fail = |at: &Path, e: std::io::Error| format!("{}: {e}", at.display());
    let mut file = fs::File::create(&temp).map_err(|e| fail(&temp, e))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| fail(&temp, e))?;
    fs::rename(&temp, path).map_err(|e| fail(path, e))?;
    if let Some(folder) = path.parent() {
        let _ = fs::File::open(folder).and_then(|folder| folder.sync_all());
    }
    Ok(())
}

fn in_repo(folder: &Path) -> bool {
    run("git", &["rev-parse", "--is-inside-work-tree"], Some(folder)).is_ok()
}

fn dirty(folder: &Path, path: &Path) -> bool {
    in_repo(folder)
        && run(
            "git",
            &["status", "--porcelain", "--", &path.to_string_lossy()],
            Some(folder),
        )
        .is_ok_and(|out| !out.trim().is_empty())
}

/// Commits just `paths`, when the folder is in a repo; elsewhere the files
/// are simply written.
fn commit(folder: &Path, paths: &[&Path], message: &str) -> Result<(), String> {
    if !in_repo(folder) {
        return Ok(());
    }
    let paths: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    // git leaves loose objects unsynced by default, and a crash then leaves the
    // commit as empty files: a broken HEAD
    let mut add = vec!["-c", "core.fsync=committed", "add", "--"];
    add.extend(paths.iter().map(String::as_str));
    run("git", &add, Some(folder))?;
    // nothing to commit when the files came out the same
    let mut staged = vec!["diff", "--cached", "--quiet", "--"];
    staged.extend(paths.iter().map(String::as_str));
    if run("git", &staged, Some(folder)).is_ok() {
        return Ok(());
    }
    let mut args = vec![
        "-c",
        "core.fsync=committed",
        "commit",
        "-q",
        "-m",
        message,
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    run("git", &args, Some(folder)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"prefer-no-csd

// the panel
output "eDP-1" {
    mode "2880x1800@120.000"
    scale 1.75
    transform "normal"
    variable-refresh-rate on-demand=true
}

/-output "HDMI-A-1" {
    off
}

layout {
    gaps 8
}
"#;

    fn panel() -> OutputConfig {
        OutputConfig {
            name: String::from("eDP-1"),
            mode: Some((2880, 1800, 120.0)),
            scale: Some(1.75),
            vrr: Vrr::OnDemand,
            ..OutputConfig::default()
        }
    }

    #[test]
    fn finds_the_live_output_blocks() {
        let found = blocks(CONFIG);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].first, found[0].last), (3, 8));
        assert_eq!(found[0].output, panel());
    }

    #[test]
    fn the_file_round_trips() {
        let displays = Displays {
            outputs: vec![
                panel(),
                OutputConfig {
                    name: String::from("Dell Inc. DELL U2723QE \"7\""),
                    off: true,
                    mode: Some((3840, 2160, 59.997)),
                    custom: true,
                    scale: Some(1.5),
                    transform: Transform::Flipped90,
                    position: Some((-1645, 200)),
                    vrr: Vrr::Always,
                    focus_at_startup: true,
                    backdrop: Some(String::from("#101010")),
                    hdr: true,
                    sdr_brightness: Some(300.),
                    extra: Vec::new(),
                },
            ],
            vrr_apps: vec![
                String::from("steam_app_1091500"),
                String::from("org.mpv.mpv"),
            ],
            extra: Vec::new(),
        };
        let text = displays.write_kdl();
        assert!(
            text.contains("match app-id=\"^org\\\\.mpv\\\\.mpv$\""),
            "{text}"
        );
        assert_eq!(Displays::read_kdl(&text), displays);
    }

    #[test]
    fn moving_comments_the_blocks_and_includes_the_file() {
        let file = Path::new("/home/me/nixos-config/niri/displays.kdl");
        let text = moved(CONFIG, &blocks(CONFIG), file);
        assert!(text.contains(
            "include optional=true \"/home/me/nixos-config/niri/displays.kdl\"\n// output \"eDP-1\" {"
        ));
        assert!(text.contains("//     variable-refresh-rate on-demand=true\n// }"));
        assert!(blocks(&text).is_empty());
        assert!(includes(&text, file));
        assert!(text.ends_with("gaps 8\n}\n"));
    }

    // the live file as the older Settings and the shell's HDR level left it
    const LIVE: &str = "// Written by Shell Settings (Displays), which reads it back: edits by
// hand may be lost. niri's config.kdl includes it.

output \"eDP-1\" {
    mode \"2880x1800@120.000\"
    scale 1.75
    transform \"normal\"
    variable-refresh-rate on-demand=true
    sdr-brightness 80
}
";

    const BY_HAND: &str = r#"// Written by Shell Settings (Displays), which reads it back; lines it
// doesn't know are kept. niri's config.kdl includes it.

output "eDP-1" {
    mode "2880x1800@120.000"
    scale 1.75
    // the panel's own note
    transform "normal"
    some-new-setting 3
    layout {
        gaps 4
    }
}

/-output "HDMI-A-1" {
    off
}

// my rule
window-rule {
    match app-id="^foot$"
    opacity 0.9
}
"#;

    #[test]
    fn the_live_file_reads_and_writes_back_the_same() {
        let displays = Displays::read_kdl(LIVE);
        assert_eq!(displays.outputs.len(), 1);
        assert_eq!(displays.outputs[0].sdr_brightness, Some(80.0));
        assert!(displays.outputs[0].extra.is_empty() && displays.extra.is_empty());
        // only the header says something new
        let written = displays.write_kdl();
        assert_eq!(
            written.lines().skip(2).collect::<Vec<_>>(),
            LIVE.lines().skip(2).collect::<Vec<_>>()
        );
        assert_eq!(Displays::read_kdl(&written).write_kdl(), written);
    }

    #[test]
    fn lines_it_doesnt_know_are_kept() {
        let displays = Displays::read_kdl(BY_HAND);
        let panel = &displays.outputs[0];
        assert_eq!(panel.scale, Some(1.75));
        assert_eq!(
            panel.extra,
            [
                "    // the panel's own note",
                "    some-new-setting 3",
                "    layout {",
                "        gaps 4",
                "    }",
            ]
        );
        assert!(
            displays.vrr_apps.is_empty(),
            "the foot rule is someone else's"
        );
        let written = displays.write_kdl();
        for line in BY_HAND.lines().filter(|line| !line.is_empty()) {
            assert!(written.contains(line), "lost {line:?} from:\n{written}");
        }
        // known settings first, then the rest as it was
        assert!(written.contains("    transform \"normal\"\n    // the panel's own note\n"));
        assert!(written.contains("    off\n}\n\n// my rule\nwindow-rule {"));
        // and what this writes, it reads back the same
        assert_eq!(Displays::read_kdl(&written), displays);
        assert_eq!(Displays::read_kdl(&written).write_kdl(), written);
    }

    #[test]
    fn this_is_the_files_one_writer() {
        fn files(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() && path.file_name().is_some_and(|name| name != "target") {
                    files(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    out.push(path);
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut all = Vec::new();
        files(&root.join("apps"), &mut all);
        files(&root.join("crates"), &mut all);
        let me = Path::new(file!()).file_name();
        let named: Vec<_> = all
            .iter()
            .filter(|file| file.file_name() != me)
            .filter(|file| {
                fs::read_to_string(file)
                    .unwrap()
                    .lines()
                    .any(|line| !line.trim_start().starts_with("//") && line.contains(FILE))
            })
            .collect();
        assert!(named.is_empty(), "only kdl.rs names {FILE}: {named:#?}");
    }

    #[test]
    fn scales_print_short() {
        assert_eq!(number(2.0), "2.0");
        assert_eq!(number(1.75), "1.75");
        assert_eq!(number(1.0 / 0.75), "1.333333");
    }
}
