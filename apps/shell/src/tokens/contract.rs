//! The tokens' contract with the code that reads them: the shell's surfaces
//! and shared pieces, and the kit. Here rather than in the kit, as it reads
//! the shell's sources and the kit builds on its own.

use std::collections::BTreeSet;

use crate::tokens::{COMPONENTS, GENERATED};

/// Source files and the components whose tokens they may read, besides `sys`.
/// The shared components (`buttons/`, `card.rs`, `list.rs`, `layout.rs` …)
/// are left out on purpose: any component can use them, so they read
/// `sys` only (manual §10.4).
const OWNERS: &[(&str, &[&str])] = &[
    ("status_bar/", &["bar"]),
    ("controls/menu/", &["menu"]),
    ("notifications/", &["notification"]),
    ("quick_settings/", &["quick"]),
    // the desktop widgets are laid out on Quick Settings' grid, and read
    // its tokens for what they share (the grid, the shadow, the lift)
    ("widgets/", &["widget", "quick"]),
    // the kit's dialog, and the shell's alert that shows it
    ("containers/dialog.rs", &["dialog"]),
    ("alert/", &["dialog"]),
    ("osd/", &["osd"]),
];

fn component_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    // the shell's surfaces, and the shared pieces of the shell and the kit,
    // which belong to no component
    let shell = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let roots = [
        shell.join("src/surfaces"),
        shell.join("src/chrome"),
        shell.join("../../crates/kit/src"),
    ];

    let mut sources = Vec::new();
    for root in roots {
        let mut files = Vec::new();
        walk(&root, &mut files);
        for f in files {
            let rel = f
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            // the token system itself is no component
            if rel.starts_with("tokens/") {
                continue;
            }
            sources.push((rel, std::fs::read_to_string(&f).unwrap()));
        }
    }
    sources
}

#[test]
fn components_read_only_sys_and_their_own_tokens() {
    for (file, source) in component_sources() {
        let own = OWNERS
            .iter()
            .find(|(prefix, _)| file.starts_with(prefix))
            .map(|(_, c)| *c)
            .unwrap_or(&[]);

        // every `crate::tokens::…` path or import list, by its first segment
        let mut used = BTreeSet::new();
        for part in source.split("crate::tokens::").skip(1) {
            let names = match part.strip_prefix('{') {
                Some(list) => list.split('}').next().unwrap_or(""),
                // one path: its first segment alone, not the text after it
                None => part
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or(""),
            };
            for name in names.split(',') {
                let name: String = name
                    .trim()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                used.insert(name);
            }
        }

        for name in used.iter().filter(|n| COMPONENTS.contains(&n.as_str())) {
            assert!(
                own.contains(&name.as_str()),
                "manual §3: {file} reads {name}.* tokens; code reads only \
                 sys.* and its own component's tokens"
            );
        }
    }
}

/// A generated function's call as it appears in code: the last two
/// segments of its path, e.g. `popover::bg(`.
fn call_suffix(path: &str) -> String {
    let parts: Vec<&str> = path.rsplit("::").take(2).collect();
    format!("{}::{}(", parts[1], parts[0])
}

/// Tokens no function call reads today. Each is either read some other
/// way or waiting to be used or removed; a token added to Penpot and
/// never read fails the test below instead of joining this list quietly.
const UNREAD_ALLOWED: &[&str] = &[
    // not read by code yet: use or remove in the kit consolidation
    "bar.overview.height",
    "bar.pill.padding-y",
    "bar.volume.width",
    "bar.window-item.max-width",
    "bar.window-item.radius",
    "bar.workspace-dot.active-size",
    "bar.workspace-dot.size",
    "menu.label.advance-ratio",
    "sys.color.accent.text",
    "sys.color.scrollbar",
    "sys.color.surface.notification.bg",
    "sys.color.surface.notification.fg",
    "sys.color.surface.notification.fg-secondary",
    "sys.color.surface.notification.stroke",
    "sys.color.surface.panel.bg",
    "sys.color.surface.panel.fg",
    "sys.color.surface.popover.bg",
    "sys.color.surface.popover.fg",
    "sys.color.surface.popover.fg-secondary",
    "sys.color.surface.popover.stroke",
    // in the Penpot set for a raised surface; nothing draws one yet
    "sys.color.surface.raised.bg",
    "sys.size.list.max-height",
    "sys.size.popover.height",
    "wifi.button.width",
    "wifi.icon-button.width",
];

/// Every generated token is read by some code, or is on the list above.
/// `sys.density.factor` is read by the theme, and `material.*` by name
/// when the layer rules are written, not through a function.
#[test]
fn every_token_is_read_or_listed() {
    let code: String = component_sources().into_iter().map(|(_, s)| s).collect();
    let unread: Vec<&str> = GENERATED
        .iter()
        .filter(|(name, _, path)| {
            *name != "sys.density.factor"
                && !name.starts_with("material.")
                && !code.contains(&call_suffix(path))
        })
        .map(|(name, _, _)| *name)
        .filter(|name| !UNREAD_ALLOWED.contains(name))
        .collect();
    assert!(
        unread.is_empty(),
        "tokens no code reads (read them, remove them, or list them in UNREAD_ALLOWED):\n  {}",
        unread.join("\n  ")
    );

    let stale: Vec<&str> = UNREAD_ALLOWED
        .iter()
        .copied()
        .filter(|name| {
            GENERATED
                .iter()
                .find(|(n, _, _)| n == name)
                .is_none_or(|(_, _, path)| code.contains(&call_suffix(path)))
        })
        .collect();
    assert!(
        stale.is_empty(),
        "UNREAD_ALLOWED lists tokens that are read now, or gone; drop them:\n  {}",
        stale.join("\n  ")
    );
}
