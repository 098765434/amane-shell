//! `cargo xtask contract`: amane and niri still agree, at the revisions the shell pins.
//!
//! - Every protocol XML the niri fork adds (`resources/niri-*.xml`) is in amane's
//!   `protocols/`, byte for byte, and amane has no fork protocol niri lacks.
//! - Every niri IPC name amane's niri client sends or parses (requests, actions, events,
//!   and their fields) exists in `niri-ipc` at the pinned niri revision.
//! - `contract.toml`'s niri revision is the one nixos-config's lock builds, when that lock is
//!   there to read.
//!
//! It reads both repositories with `git show <rev>:<path>`, from checkouts at `../amane` and
//! `../niri` (or `--amane <dir>`, `--niri <dir>`), so the working trees can be anywhere. The
//! amane revision comes from the root `Cargo.toml`, niri's from `contract.toml`
//! (docs/integration/contract.md).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs the checks; prints each problem and returns whether there were none.
pub fn run(root: &Path, args: &[&str]) -> bool {
    let mut amane = root.join("../amane");
    let mut niri = root.join("../niri");
    let mut nixos_lock =
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("nixos-config/flake.lock"));
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match (*arg, rest.next()) {
            ("--amane", Some(dir)) => amane = PathBuf::from(dir),
            ("--niri", Some(dir)) => niri = PathBuf::from(dir),
            ("--nixos-lock", Some(file)) => nixos_lock = Some(PathBuf::from(file)),
            _ => {
                eprintln!(
                    "usage: cargo xtask contract [--amane <dir>] [--niri <dir>] [--nixos-lock <file>]"
                );
                return false;
            }
        }
    }

    let Some(amane_rev) = read(&root.join("Cargo.toml")).and_then(|t| amane_rev(&t)) else {
        eprintln!("contract: no amane rev in Cargo.toml's [workspace.dependencies]");
        return false;
    };
    let Some(niri_rev) = read(&root.join("contract.toml")).and_then(|t| toml_string(&t, "niri"))
    else {
        eprintln!("contract: no `niri = \"<rev>\"` in contract.toml");
        return false;
    };

    let mut problems = Vec::new();
    let amane = Repo::new(&amane, &amane_rev, &mut problems);
    let niri = Repo::new(&niri, &niri_rev, &mut problems);
    if let (Some(amane), Some(niri)) = (&amane, &niri) {
        protocols(amane, niri, &mut problems);
        ipc_names(amane, niri, &mut problems);
    }
    if let Some(lock) = nixos_lock.as_deref().filter(|lock| lock.exists()) {
        locked_niri(lock, &niri_rev, &mut problems);
    }

    for problem in &problems {
        eprintln!("contract: {problem}");
    }
    if problems.is_empty() {
        println!(
            "contract: amane {} and niri {} agree",
            &amane_rev[..8.min(amane_rev.len())],
            &niri_rev[..8.min(niri_rev.len())]
        );
    }
    problems.is_empty()
}

/// A repository read at one revision.
struct Repo {
    dir: PathBuf,
    rev: String,
}

impl Repo {
    fn new(dir: &Path, rev: &str, problems: &mut Vec<String>) -> Option<Self> {
        let repo = Self {
            dir: dir.to_path_buf(),
            rev: rev.to_owned(),
        };
        let known = repo.git(&["cat-file", "-e", &format!("{rev}^{{commit}}")]);
        if known.is_none() {
            problems.push(format!(
                "{} doesn't have {rev}: fetch it, or pass the checkout's path",
                dir.display()
            ));
            return None;
        }
        Some(repo)
    }

    fn git(&self, args: &[&str]) -> Option<Vec<u8>> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .output()
            .ok()?;
        out.status.success().then_some(out.stdout)
    }

    /// The paths under `dir` at the revision.
    fn files(&self, dir: &str) -> Vec<String> {
        let out = self
            .git(&["ls-tree", "-r", "--name-only", &self.rev, "--", dir])
            .unwrap_or_default();
        String::from_utf8_lossy(&out)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn show(&self, path: &str) -> Option<Vec<u8>> {
        self.git(&["show", &format!("{}:{path}", self.rev)])
    }

    fn text(&self, path: &str) -> String {
        String::from_utf8_lossy(&self.show(path).unwrap_or_default()).into_owned()
    }
}

/// The fork protocols are the same XML on both sides.
fn protocols(amane: &Repo, niri: &Repo, problems: &mut Vec<String>) {
    let name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_owned();
    let is_fork = |path: &String| {
        let file = name(path);
        file.starts_with("niri-") && file.ends_with(".xml")
    };
    let theirs: BTreeSet<String> = niri
        .files("resources")
        .into_iter()
        .filter(is_fork)
        .collect();
    let ours: BTreeSet<String> = amane
        .files("protocols")
        .into_iter()
        .filter(is_fork)
        .collect();

    for path in &theirs {
        let file = name(path);
        let copy = format!("protocols/{file}");
        match amane.show(&copy) {
            None => problems.push(format!("amane has no {copy} (niri has {path})")),
            Some(bytes) if Some(&bytes) != niri.show(path).as_ref() => {
                problems.push(format!("amane's {copy} differs from niri's {path}"))
            }
            Some(_) => (),
        }
    }
    for path in &ours {
        let file = name(path);
        if !theirs.iter().any(|theirs| name(theirs) == file) {
            problems.push(format!("niri has no resources/{file} (amane has {path})"));
        }
    }
}

/// The names amane's niri client uses exist in niri-ipc.
fn ipc_names(amane: &Repo, niri: &Repo, problems: &mut Vec<String>) {
    let client: String = amane
        .files("src/compositor")
        .iter()
        .filter(|path| path.contains("niri") && path.ends_with(".rs"))
        .map(|path| amane.text(path))
        .collect::<Vec<_>>()
        .join("\n");
    if client.is_empty() {
        problems.push(String::from(
            "amane has no niri client under src/compositor",
        ));
        return;
    }
    let ipc = niri.text("niri-ipc/src/lib.rs");
    if ipc.is_empty() {
        problems.push(String::from("niri has no niri-ipc/src/lib.rs"));
        return;
    }

    let (variants, fields) = ipc_vocabulary(&ipc);
    let (used_variants, used_fields) = used_names(&strip_tests(&client));
    for name in used_variants.difference(&variants) {
        problems.push(format!(
            "amane uses the niri IPC name {name:?}, which niri-ipc lacks"
        ));
    }
    for name in used_fields.difference(&fields) {
        problems.push(format!(
            "amane uses the niri IPC field {name:?}, which niri-ipc lacks"
        ));
    }
}

/// Drops `#[cfg(test)]` modules: their JSON is fixtures, checked by amane's own tests.
fn strip_tests(source: &str) -> String {
    match source.find("#[cfg(test)]") {
        Some(at) => source[..at].to_owned(),
        None => source.to_owned(),
    }
}

/// niri-ipc's enum variants and field names: what serde puts on the wire.
fn ipc_vocabulary(source: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut variants = BTreeSet::new();
    let mut fields = BTreeSet::new();
    let mut in_enum = false;
    for line in source.lines() {
        if line.starts_with("pub enum ") {
            in_enum = true;
            continue;
        }
        if line.starts_with('}') {
            in_enum = false;
            continue;
        }
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if in_enum && indent == 4 {
            let name: String = trimmed
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                variants.insert(name);
            }
        }
        // `pub name: T` in structs, `name: T` in struct variants
        let field = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        if let Some((name, _)) = field.split_once(':')
            && !name.is_empty()
            && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        {
            fields.insert(name.to_owned());
        }
    }
    (variants, fields)
}

/// The names in a niri client's string literals: `"Name":` and `"name":` keys in `json!`,
/// `"Name" =>` match arms, and `.get("name")` reads.
fn used_names(source: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut variants = BTreeSet::new();
    let mut fields = BTreeSet::new();
    let mut rest = source;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        let literal = &after[..end];
        let tail = after[end + 1..].trim_start();
        let before = rest[..start].trim_end();
        rest = &after[end + 1..];

        let is_ident = !literal.is_empty()
            && literal
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !is_ident || matches!(literal, "Ok" | "Err") {
            continue;
        }
        let used = tail.starts_with(':') && !tail.starts_with("::")
            || tail.starts_with("=>")
            || before.ends_with(".get(")
            || before.ends_with("json!(");
        if !used {
            continue;
        }
        if literal.starts_with(|c: char| c.is_ascii_uppercase()) {
            variants.insert(literal.to_owned());
        } else {
            fields.insert(literal.to_owned());
        }
    }
    (variants, fields)
}

/// `contract.toml` names the niri the shell is tested against; nixos-config's lock is what the
/// machine builds. They should be the same commit.
fn locked_niri(lock: &Path, niri_rev: &str, problems: &mut Vec<String>) {
    let Some(text) = read(lock) else { return };
    let Some(at) = text.find("\"niri-fork\": {") else {
        return;
    };
    let Some(rev) = toml_string(&text[at..], "\"rev\"") else {
        return;
    };
    if rev != niri_rev {
        problems.push(format!(
            "contract.toml pins niri {niri_rev}, but {} builds {rev}",
            lock.display()
        ));
    }
}

/// amane's `rev` in `[workspace.dependencies]`.
fn amane_rev(manifest: &str) -> Option<String> {
    let line = manifest
        .lines()
        .find(|line| line.starts_with("amane = {"))?;
    toml_string(line, "rev")
}

/// The string after the first `key = "…"` (TOML) or `key: "…"` (JSON) in `text`.
fn toml_string(text: &str, key: &str) -> Option<String> {
    text.match_indices(key).find_map(|(at, _)| {
        let rest = text[at + key.len()..].trim_start();
        let rest = rest
            .strip_prefix(['=', ':'])?
            .trim_start()
            .strip_prefix('"')?;
        Some(rest[..rest.find('"')?].to_owned())
    })
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .map_err(|e| eprintln!("contract: {}: {e}", path.display()))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_names_a_client_sends_and_reads() {
        let source = r#"
            let reply = ask(&json!("Outputs"))?;
            let request = json!({ "Output": { "action": { "SdrBrightness": { "nits": nits } } } });
            match name { "WindowFocusChanged" => focus(body), _ => {} }
            let hdr = output.get("hdr_enabled");
            let built_in = ["eDP", "LVDS"];
            let path = "a::b";
            reply.get("Ok");
        "#;
        let (variants, fields) = used_names(source);
        assert_eq!(
            variants.into_iter().collect::<Vec<_>>(),
            ["Output", "Outputs", "SdrBrightness", "WindowFocusChanged"]
        );
        assert_eq!(
            fields.into_iter().collect::<Vec<_>>(),
            ["action", "hdr_enabled", "nits"]
        );
    }

    #[test]
    fn reads_variants_and_fields_from_niri_ipc() {
        let source = "pub enum Request {\n    /// Doc.\n    Version,\n    Output {\n        output: String,\n    },\n}\npub struct Window {\n    pub is_fullscreen: bool,\n}\n";
        let (variants, fields) = ipc_vocabulary(source);
        assert_eq!(
            variants.into_iter().collect::<Vec<_>>(),
            ["Output", "Version"]
        );
        assert!(fields.contains("output") && fields.contains("is_fullscreen"));
    }

    #[test]
    fn reads_the_pins() {
        let manifest = "amane = { git = \"https://x\", rev = \"abc123\" }\n";
        assert_eq!(amane_rev(manifest).as_deref(), Some("abc123"));
        let contract = "# niri-ipc is read at\nniri = \"def\"\n";
        assert_eq!(toml_string(contract, "niri").as_deref(), Some("def"));
        let lock = r#""niri-fork": { "locked": { "owner": "x", "rev": "0123" } }"#;
        assert_eq!(toml_string(lock, "\"rev\"").as_deref(), Some("0123"));
    }
}
