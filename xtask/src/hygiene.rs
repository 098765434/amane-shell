//! What the polish plan made one copy of stays one copy: each rule names a
//! spelling that belongs in one place only, and `cargo test` fails on it
//! anywhere else. See `docs/developer/polish-plan.md`.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// A spelling, where it may be, and what to use instead.
    struct Rule {
        spelling: &'static str,
        allowed: &'static [&'static str],
        instead: &'static str,
    }

    const RULES: &[Rule] = &[
        Rule {
            spelling: ".lock().unwrap_or_else(",
            allowed: &["crates/base/src/sync.rs"],
            instead: "emaki_base::sync::Locked::locked()",
        },
        Rule {
            spelling: ".read().unwrap_or_else(",
            allowed: &["crates/base/src/sync.rs"],
            instead: "emaki_base::sync::Shared::reading()",
        },
        Rule {
            spelling: ".write().unwrap_or_else(",
            allowed: &["crates/base/src/sync.rs"],
            instead: "emaki_base::sync::Shared::writing()",
        },
        Rule {
            spelling: "Box::leak(",
            allowed: &["crates/base/src/intern.rs", "crates/kit/src/tokens/mod.rs"],
            instead: "emaki_base::intern::intern",
        },
        Rule {
            spelling: "var_os(\"XDG_",
            allowed: &["crates/base/src/paths.rs"],
            instead: "emaki_base::paths",
        },
        Rule {
            spelling: ".join(\"emaki\")",
            allowed: &[],
            instead: "app_ids::SHELL",
        },
        Rule {
            spelling: "emaki_log::init(\"",
            allowed: &[],
            instead: "an app_ids constant",
        },
        Rule {
            spelling: ".fill(Color::TRANSPARENT)",
            allowed: &[],
            instead: "nothing: a Rectangle starts transparent",
        },
    ];

    fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("a source folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "target") {
                    sources(&path, out);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    /// The text with line breaks and the spaces after them gone, so a
    /// chain rustfmt splits over lines reads as one.
    fn joined(text: &str) -> String {
        text.lines().map(str::trim_start).collect()
    }

    #[test]
    fn what_was_made_one_copy_stays_one() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        for dir in ["apps", "crates"] {
            sources(&root.join(dir), &mut files);
        }
        assert!(files.len() > 100, "found too few sources");
        let mut found = Vec::new();
        for file in &files {
            let at = file.strip_prefix(&root).unwrap_or(file);
            let at = at.to_string_lossy();
            let text = joined(&std::fs::read_to_string(file).expect("a source file"));
            for rule in RULES {
                if text.contains(rule.spelling) && !rule.allowed.contains(&at.as_ref()) {
                    found.push(format!("{at}: {} (use {})", rule.spelling, rule.instead));
                }
            }
        }
        assert!(found.is_empty(), "one copy each:\n  {}", found.join("\n  "));
    }
}
