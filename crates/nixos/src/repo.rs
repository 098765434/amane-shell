//! What a system and a commit say about themselves.

use serde_json::Value;

/// A commit as a label shows it: the first seven characters, keeping
/// `-dirty`.
pub fn short(revision: &str) -> String {
    let (sha, dirty) = match revision.strip_suffix("-dirty") {
        Some(sha) => (sha, "-dirty"),
        None => (revision, ""),
    };
    format!("{}{dirty}", &sha[..sha.len().min(7)])
}

/// `nixos-version --json` for a system: its version, nixpkgs and commit.
pub fn version_of(toplevel: &std::path::Path) -> Option<(String, Option<String>, Option<String>)> {
    let script = toplevel.join("sw/bin/nixos-version");
    let json = crate::run(&script.to_string_lossy(), &["--json"], None).ok()?;
    let value: Value = serde_json::from_str(&json).ok()?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);

    Some((
        text("nixosVersion").unwrap_or_default(),
        text("nixpkgsRevision"),
        text("configurationRevision"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_revision_shortens_and_keeps_its_dirt() {
        assert_eq!(short("5d056b8af6262450c485ab8abe3d4e376ba267d1"), "5d056b8");
        assert_eq!(
            short("5d056b8af6262450c485ab8abe3d4e376ba267d1-dirty"),
            "5d056b8-dirty"
        );
        assert_eq!(short("abc"), "abc");
    }
}
