//! `cargo xtask <task>`: the workspace's own tasks, from the root.
//!
//! - `layer-rules`: niri's layer rules from the tokens, as the committed
//!   `niri-layer-rule.kdl` holds them (the shell builds them: they name its
//!   surfaces).
//! - `tokens normalize <file>`, `tokens check <file>`: the Penpot export,
//!   sorted as it's committed, or checked against the manual's rules.
//! - `lint-terms`: `scripts/lint-terms`, no other vendor's platform terms.
//! - `links`: every relative link in the Markdown points at a file that
//!   exists.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = root();

    let ok = match args.as_slice() {
        ["layer-rules"] => run(Command::new(cargo())
            .current_dir(&root)
            .args(["run", "-q", "-p", "amane-shell", "--", "layer-rules"])
            .env("AMANE_FROM_XTASK", "1")),
        ["tokens", "normalize", path] => {
            read(path).is_some_and(|json| match amane_kit::tokens::check::normalize(&json) {
                Ok(out) => {
                    print!("{out}");
                    true
                }
                Err(e) => {
                    eprintln!("xtask: {path}: {e}");
                    false
                }
            })
        }
        ["tokens", "check", path] => {
            read(path).is_some_and(|json| amane_kit::tokens::check::check(&json))
        }
        ["lint-terms", rest @ ..] => run(Command::new(root.join("scripts/lint-terms")).args(rest)),
        ["links"] => links(&root),
        _ => {
            eprintln!(
                "usage: cargo xtask layer-rules | tokens (normalize | check) <file> | lint-terms | links"
            );
            false
        }
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The workspace root: xtask's own folder's parent.
fn root() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent().unwrap_or(here).to_path_buf()
}

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"))
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .map_err(|e| eprintln!("xtask: {path}: {e}"))
        .ok()
}

fn run(command: &mut Command) -> bool {
    match command.status() {
        Ok(status) => status.success(),
        Err(e) => {
            eprintln!("xtask: {e}");
            false
        }
    }
}

/// Every relative link in the Markdown outside `target/` and `.git/` must
/// name a file or folder that exists; an anchor alone, a web address or a
/// mail address isn't checked.
fn links(root: &Path) -> bool {
    let mut files = Vec::new();
    markdown(root, &mut files);
    files.sort();

    let mut broken = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let dir = file.parent().unwrap_or(root);
        for (n, line) in prose_lines(&text) {
            for target in targets(line) {
                let path = target.split('#').next().unwrap_or("");
                if path.is_empty() || path.contains("://") || path.starts_with("mailto:") {
                    continue;
                }
                if !dir.join(path).exists() {
                    let shown = file.strip_prefix(root).unwrap_or(file);
                    broken.push(format!("{}:{n}: {target}", shown.display()));
                }
            }
        }
    }

    for line in &broken {
        println!("{line}");
    }
    if broken.is_empty() {
        true
    } else {
        eprintln!(
            "xtask links: {} links name a file that isn't there",
            broken.len()
        );
        false
    }
}

fn markdown(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" || name == "result" {
            continue;
        }
        if path.is_dir() {
            markdown(&path, out);
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}

/// The lines outside fenced code blocks, numbered from 1.
fn prose_lines(text: &str) -> Vec<(usize, &str)> {
    let mut fenced = false;
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            out.push((i + 1, line));
        }
    }
    out
}

/// The targets of `[text](target)` links on a line, outside inline code.
fn targets(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut code = false;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'`' => code = !code,
            b']' if !code && bytes.get(i + 1) == Some(&b'(') => {
                let start = i + 2;
                if let Some(len) = line[start..].find(')') {
                    let target = line[start..start + len].trim();
                    // `[a](b "title")`: the title isn't part of the path
                    let target = target.split_whitespace().next().unwrap_or("");
                    out.push(target.trim_start_matches('<').trim_end_matches('>'));
                    i = start + len;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_found_outside_code() {
        assert_eq!(
            targets("see [a](x.md#y) and `[b](z.md)`, [c](<w v.md> \"t\")"),
            ["x.md#y", "w"]
        );
        let text = "a\n```\n[x](gone.md)\n```\n[y](here.md)";
        let lines: Vec<usize> = prose_lines(text).iter().map(|(n, _)| *n).collect();
        assert_eq!(lines, [1, 5]);
    }
}
