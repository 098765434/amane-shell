//! The displays' lasting settings: `displays.kdl`, beside niri's
//! `config.kdl`, which includes it. Settings owns the file outright, so it
//! is written whole each time and read back with a reader for what it
//! writes, never a KDL parser.
//!
//! niri uses the first `output` block that matches, so the blocks
//! `config.kdl` had of its own are moved here once ([`migrate`]): they
//! become comments, an `include` takes the first one's place, and both
//! files are committed together when they live in a git repo.

use std::fs;
use std::path::{Path, PathBuf};

use super::niri::Transform;
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
}

/// The whole file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Displays {
    pub outputs: Vec<OutputConfig>,
    /// Apps whose windows turn on-demand variable refresh on.
    pub vrr_apps: Vec<String>,
}

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
        let mut out = String::from(
            "// Written by Shell Settings (Displays), which reads it back: edits by\n\
             // hand may be lost. niri's config.kdl includes it.\n",
        );
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
        out
    }

    /// What [`write_kdl`](Self::write_kdl) wrote, and the subset of a
    /// hand-written `output` block it shares; other lines are skipped.
    pub fn read_kdl(text: &str) -> Self {
        let mut displays = Displays::default();
        let mut current: Option<OutputConfig> = None;
        let mut in_rule = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("output ") {
                let name = rest.trim_end_matches('{').trim();
                current = Some(OutputConfig {
                    name: unquote(name).unwrap_or_default(),
                    ..OutputConfig::default()
                });
                continue;
            }
            if line.starts_with("window-rule") {
                in_rule = true;
                continue;
            }
            if line == "}" {
                if let Some(output) = current.take() {
                    displays.outputs.push(output);
                }
                in_rule = false;
                continue;
            }
            if in_rule {
                if let Some(app) =
                    line.strip_prefix("match app-id=")
                        .and_then(unquote)
                        .map(|regex| {
                            regex_unescape(regex.trim_start_matches('^').trim_end_matches('$'))
                        })
                {
                    displays.vrr_apps.push(app);
                }
                continue;
            }
            let Some(output) = current.as_mut() else {
                continue;
            };
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "off" => output.off = true,
                "mode" => {
                    output.custom = value.contains("custom=true");
                    let quoted = value.rsplit_once(' ').map_or(value, |(_, last)| last);
                    output.mode = unquote(quoted).and_then(|mode| parse_mode(&mode));
                }
                "scale" => output.scale = value.parse().ok(),
                "transform" => {
                    output.transform = unquote(value)
                        .and_then(|name| Transform::ALL.into_iter().find(|t| t.config() == name))
                        .unwrap_or_default();
                }
                "position" => {
                    let field = |name: &str| {
                        value
                            .split_whitespace()
                            .find_map(|part| part.strip_prefix(name))
                            .and_then(|v| v.parse().ok())
                    };
                    output.position = field("x=").zip(field("y="));
                }
                "variable-refresh-rate" => {
                    output.vrr = if value.contains("on-demand=true") {
                        Vrr::OnDemand
                    } else {
                        Vrr::Always
                    };
                }
                "focus-at-startup" => output.focus_at_startup = true,
                "hdr" => output.hdr = true,
                "sdr-brightness" => output.sdr_brightness = value.parse().ok(),
                "backdrop-color" => output.backdrop = unquote(value),
                _ => {}
            }
        }
        displays
    }
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
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let link = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"))
        .join("niri/config.kdl");
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
        vrr_apps: Vec::new(),
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

/// Into place by a rename, so niri never reads half a file.
fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let temp = path.with_extension("kdl.tmp");
    fs::write(&temp, text).map_err(|e| format!("{}: {e}", temp.display()))?;
    fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))
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
    let mut add = vec!["add", "--"];
    add.extend(paths.iter().map(String::as_str));
    run("git", &add, Some(folder))?;
    // nothing to commit when the files came out the same
    let mut staged = vec!["diff", "--cached", "--quiet", "--"];
    staged.extend(paths.iter().map(String::as_str));
    if run("git", &staged, Some(folder)).is_ok() {
        return Ok(());
    }
    let mut args = vec!["commit", "-q", "-m", message, "--"];
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
                },
            ],
            vrr_apps: vec![
                String::from("steam_app_1091500"),
                String::from("org.mpv.mpv"),
            ],
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

    #[test]
    fn scales_print_short() {
        assert_eq!(number(2.0), "2.0");
        assert_eq!(number(1.75), "1.75");
        assert_eq!(number(1.0 / 0.75), "1.333333");
    }
}
