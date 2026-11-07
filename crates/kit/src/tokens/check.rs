//! `amane-shell tokens normalize <file>` and `amane-shell tokens check <file>`
//! (manual §10.3).

use std::collections::BTreeMap;

use serde_json::Value as Json;

use super::envelope::Library;
use super::{SOURCE, rules};

/// Keys sorted, two-space indent, trailing newline: Penpot builds its export
/// from hash maps, so this keeps diffs to the tokens that changed. Arrays keep
/// their order. The sorting is serde_json's default map; it would stop if a
/// dependency turned on its `preserve_order` feature.
pub fn normalize(json: &str) -> Result<String, String> {
    let value: Json = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    serde_json::to_string_pretty(&value)
        .map(|s| s + "\n")
        .map_err(|e| e.to_string())
}

/// Runs the manual's rules on `json` and prints how it differs from the
/// committed tokens. Returns whether every rule passed.
pub fn check(json: &str) -> bool {
    let lib = match Library::parse(json) {
        Ok(lib) => lib,
        Err(e) => {
            println!("does not parse: {e}");
            return false;
        }
    };

    let violations = rules::check(&lib, json);
    for v in &violations {
        println!("✗ {v}");
    }

    let committed = Library::parse(SOURCE).ok();
    let (old, new) = (committed.as_ref().map(flat).unwrap_or_default(), flat(&lib));
    for (name, value) in &new {
        match old.get(name) {
            None => println!("+ {name} = {value}"),
            Some(before) if before != value => println!("~ {name}: {before} → {value}"),
            _ => {}
        }
    }
    for name in old.keys().filter(|n| !new.contains_key(*n)) {
        println!("- {name}");
    }

    for (tier, count, budget) in rules::tier_counts(&lib) {
        println!("{tier}: {count} tokens (budget about {budget})");
    }

    if violations.is_empty() {
        println!("✓ all rules pass");
    }
    violations.is_empty()
}

/// `set: token` → raw value, for the diff.
fn flat(lib: &Library) -> BTreeMap<String, String> {
    lib.sets
        .iter()
        .flat_map(|s| {
            s.tokens
                .iter()
                .map(move |t| (format!("{}: {}", s.name, t.name), t.value.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_sorts_keys_and_keeps_arrays() {
        let out = normalize(r#"{"b": [3, 1], "a": {"d": 1, "c": 2}}"#).unwrap();
        assert_eq!(
            out,
            "{\n  \"a\": {\n    \"c\": 2,\n    \"d\": 1\n  },\n  \"b\": [\n    3,\n    1\n  ]\n}\n"
        );
        assert_eq!(normalize(&out).unwrap(), out);
    }
}
