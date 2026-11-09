//! The files Emaki Settings keeps beside niri's `config.kdl`, which
//! includes them (`displays.kdl`, `input.kdl`): finding niri's config, the
//! include line, a top-level block's lines, moving blocks out of
//! `config.kdl` once, and writing a file whole and committing it.
//!
//! None of this parses KDL. Each file's own module reads what it writes,
//! line by line with the braces counted, and keeps the lines it doesn't
//! know.

use std::fs;
use std::path::{Path, PathBuf};

use crate::system::nixos::run;

/// niri's own config, where its symlink leads (into the nixos-config repo
/// here), so the files land beside the real one.
pub fn niri_config() -> PathBuf {
    let link = emaki_kit::paths::config().join("niri/config.kdl");
    fs::canonicalize(&link).unwrap_or(link)
}

/// `name` beside niri's config.
pub fn beside_config(name: &str) -> PathBuf {
    niri_config().with_file_name(name)
}

/// The line `config.kdl` includes a file with. The path is absolute:
/// niri reads includes beside the config's symlink, not where it leads.
pub fn include_line(file: &Path) -> String {
    format!("include optional=true {}", quote(&file.to_string_lossy()))
}

/// Whether `config` includes `file` with [`include_line`].
pub fn includes(config: &str, file: &Path) -> bool {
    let wanted = include_line(file);
    config.lines().any(|line| line.trim() == wanted)
}

/// A line without its `//` comment, a `//` in a string aside.
pub fn code(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    for (index, &b) in bytes.iter().enumerate() {
        match b {
            _ if escaped => escaped = false,
            b'\\' if in_string => escaped = true,
            b'"' => in_string = !in_string,
            b'/' if !in_string && bytes.get(index + 1) == Some(&b'/') => return &line[..index],
            _ => {}
        }
    }
    line
}

/// How much deeper a line goes, braces in strings aside.
pub fn braces(code: &str) -> i32 {
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

/// The last line of the node that starts at `start`: where its braces close.
pub fn node_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0;
    for (index, line) in lines.iter().enumerate().skip(start) {
        depth += braces(code(line));
        if depth <= 0 {
            return index;
        }
    }
    lines.len() - 1
}

/// The top-level nodes named `keyword` that aren't commented out (`//` or
/// `/-`): their first and last lines, from 0.
pub fn spans(config: &str, keyword: &str) -> Vec<(usize, usize)> {
    let lines: Vec<&str> = config.lines().collect();
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        let code = code(line).trim();
        if depth == 0 {
            let name = code.split([' ', '{']).next().unwrap_or("");
            if name == keyword {
                start = Some(index);
            }
        }
        depth += braces(code);
        if depth == 0
            && let Some(first) = start.take()
        {
            found.push((first, index));
        }
    }
    found
}

/// `config` with the lines of `spans` commented out and the include of
/// `file`, under `note`, where the first one was (at the end without any).
pub fn moved(config: &str, spans: &[(usize, usize)], file: &Path, note: &str) -> String {
    let mut out = Vec::new();
    for (index, line) in config.lines().enumerate() {
        if spans.first().is_some_and(|(first, _)| *first == index) {
            out.push(format!("// {note}"));
            out.push(include_line(file));
        }
        if spans
            .iter()
            .any(|(first, last)| (*first..=*last).contains(&index))
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
    if spans.is_empty() {
        text.push_str(&format!("\n// {note}\n{}\n", include_line(file)));
    }
    text
}

pub fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn unquote(text: &str) -> Option<String> {
    let inner = text.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// Into place whole and durably, so niri never reads half a file and a
/// crash can't leave it empty.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    emaki_kit::fs::Whole::new()
        .durable()
        .write(path, text)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn in_repo(folder: &Path) -> bool {
    run("git", &["rev-parse", "--is-inside-work-tree"], Some(folder)).is_ok()
}

/// Whether `path` has changes git hasn't been given, when it's in a repo.
pub fn dirty(folder: &Path, path: &Path) -> bool {
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
pub fn commit(folder: &Path, paths: &[&Path], message: &str) -> Result<(), String> {
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

/// The `.rs` files under `apps/` and `crates/` other than `me` (from this
/// crate's folder) whose code, not comments, names `file`: for each file's
/// one-writer test.
#[cfg(test)]
pub fn others_naming(file: &str, me: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().is_some_and(|name| name != "target") {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let me = Path::new(env!("CARGO_MANIFEST_DIR")).join(me);
    let mut all = Vec::new();
    walk(&root.join("apps"), &mut all);
    walk(&root.join("crates"), &mut all);
    all.into_iter()
        .filter(|path| fs::canonicalize(path).ok() != fs::canonicalize(&me).ok())
        .filter(|path| {
            fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .any(|line| !line.trim_start().starts_with("//") && line.contains(file))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_go_but_not_in_strings() {
        assert_eq!(code("tap // on"), "tap ");
        assert_eq!(code(r#"spawn "a//b" // c"#), r#"spawn "a//b" "#);
        assert_eq!(code(r#"x "\"//" y"#), r#"x "\"//" y"#);
    }

    #[test]
    fn spans_skip_commented_nodes() {
        let config = "input {\n    tap\n}\n/-input {\n}\n// input {\n// }\nbinds {\n    Mod+T { spawn \"{\"; }\n}\n";
        assert_eq!(spans(config, "input"), [(0, 2)]);
        assert_eq!(spans(config, "binds"), [(7, 9)]);
        assert!(spans(config, "output").is_empty());
    }
}
