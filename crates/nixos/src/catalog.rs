//! The nixpkgs catalog the Apps page searches: every package `nix search`
//! finds in the configuration's nixpkgs (less nixpkgs' own `tests`), a line
//! each (`attr`, `pname`, `version`, `description`, tab-separated), in a
//! file per locked nixpkgs revision. It's built once per nixpkgs update
//! from `nix search --json … ^`, streamed straight into the file, and each
//! search reads the file again (from the page cache, a few hundred thousand
//! lines in tens of milliseconds) rather than keeping the packages in
//! memory. A search with no catalog yet asks `nix search` itself
//! ([`parse_search`]).

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use amane_process::Cmd;
use serde::Deserialize;
use serde::de::{self, Deserializer as _, MapAccess, Visitor};
use serde_json::Value;

use crate::Config;

/// How many results a search keeps.
pub const RESULTS: usize = 50;

/// A package `nix search` found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub attr: String,
    pub pname: String,
    pub version: String,
    pub description: String,
}

/// `nix search --json` output: `legacyPackages.<system>.<attr>` to
/// `{pname, version, description}`, best match for `query` first.
pub fn parse_search(json: &str, query: &str) -> Result<Vec<Found>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let map = value.as_object().ok_or("not an object")?;
    let mut found: Vec<Found> = map
        .iter()
        .map(|(key, info)| {
            let text = |field: &str| {
                info.get(field)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            Found {
                attr: attr_of(key).to_string(),
                pname: text("pname"),
                version: text("version"),
                description: text("description").lines().next().unwrap_or("").to_string(),
            }
        })
        .collect();
    found.sort_by_cached_key(|found| rank(found, query));
    Ok(found)
}

/// A `nix search` key's attribute path, past "legacyPackages.x86_64-linux.".
pub fn attr_of(key: &str) -> &str {
    key.strip_prefix("legacyPackages.")
        .and_then(|rest| rest.split_once('.'))
        .map_or(key, |(_, attr)| attr)
}

/// How well a package matches: top-level packages before the ones in a set
/// (`python3Packages.…`) unless the query names a set; then the name equal
/// to the query, starting with it, containing it, and last a match only in
/// the description; then shorter attribute paths.
pub fn rank(found: &Found, query: &str) -> (bool, u8, usize, String) {
    let query = query.to_lowercase();
    let name = found
        .attr
        .rsplit('.')
        .next()
        .unwrap_or(&found.attr)
        .to_lowercase();
    let pname = found.pname.to_lowercase();
    let names = [name.as_str(), pname.as_str()];
    let tier = if names.contains(&query.as_str()) {
        0
    } else if names.iter().any(|n| n.starts_with(&query)) {
        1
    } else if names.iter().any(|n| n.contains(&query)) {
        2
    } else {
        3
    };
    let in_set = found.attr.contains('.') && !query.contains('.');
    (in_set, tier, found.attr.len(), found.attr.clone())
}

/// The catalog's file for a nixpkgs revision, in `dir`.
pub fn file_for(dir: &Path, rev: &str) -> PathBuf {
    // a narHash has slashes in it
    dir.join(format!("{}.tsv", rev.replace('/', "_")))
}

/// The revision `flake.lock` pins the root's `nixpkgs` input to (its
/// narHash when it has no rev, as a path input).
pub fn locked_rev(lock: &str) -> Option<String> {
    let lock: Value = serde_json::from_str(lock).ok()?;
    let nodes = lock.get("nodes")?;
    let root = lock.get("root").and_then(Value::as_str).unwrap_or("root");
    let node = nodes.get(root)?.get("inputs")?.get("nixpkgs")?.as_str()?;
    let locked = nodes.get(node)?.get("locked")?;
    locked
        .get("rev")
        .or_else(|| locked.get("narHash"))
        .and_then(Value::as_str)
        .map(String::from)
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    pname: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: Option<String>,
}

/// One line of the file, a package's fields with no tabs or newlines in them.
fn line(attr: &str, pname: &str, version: &str, description: &str) -> String {
    let clean = |text: &str| text.replace(['\t', '\n', '\r'], " ");
    let description = description.lines().next().unwrap_or("");
    format!(
        "{}\t{}\t{}\t{}\n",
        clean(attr),
        clean(pname),
        clean(version),
        clean(description)
    )
}

/// Writes each entry of `nix search --json`'s object as a line as it's read.
struct Lines<W> {
    out: W,
    count: usize,
}

impl<'de, W: Write> Visitor<'de> for &mut Lines<W> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("nix search's JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some((key, raw)) = map.next_entry::<String, Raw>()? {
            let attr = attr_of(&key);
            // nixpkgs' own test derivations, nothing to install
            if attr.starts_with("tests.") {
                continue;
            }
            let text = line(
                attr,
                &raw.pname,
                &raw.version,
                raw.description.as_deref().unwrap_or(""),
            );
            self.out
                .write_all(text.as_bytes())
                .map_err(de::Error::custom)?;
            self.count += 1;
        }
        Ok(())
    }
}

/// Turns `nix search --json` output into the catalog's lines.
pub fn convert(json: impl Read, out: impl Write) -> Result<usize, String> {
    let mut lines = Lines { out, count: 0 };
    let mut reader = serde_json::Deserializer::from_reader(BufReader::new(json));
    reader
        .deserialize_map(&mut lines)
        .map_err(|error| error.to_string())?;
    reader.end().map_err(|error| error.to_string())?;
    lines.out.flush().map_err(|error| error.to_string())?;
    Ok(lines.count)
}

/// Runs `nix search … ^` into `path` (through a rename), and says how many
/// packages it holds.
pub fn write_catalog(config: &Config, path: &Path) -> Result<usize, String> {
    let repo = &config.repo;
    let folder = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let new = path.with_extension("tsv.new");

    let mut running = Cmd::new(&config.nix)
        .args(["search", "--json", "--no-warn-dirty", "--inputs-from"])
        .arg(repo)
        .args(["nixpkgs", "^"])
        .dir(repo)
        .spawn()
        .map_err(|error| format!("nix: {error}"))?;

    // converted as it comes; finish reads stderr alongside, so a full pipe
    // never stalls nix
    let stdout = running.take_stdout().ok_or("nix: no output")?;
    let written = File::create(&new)
        .map_err(|error| error.to_string())
        .and_then(|file| convert(stdout, BufWriter::new(file)));
    let finished = running.finish();

    // nix's own error first: a failed search also cuts the JSON short
    let result = match finished {
        Ok(_) => written.and_then(|count| {
            fs::rename(&new, path)
                .map(|()| count)
                .map_err(|error| error.to_string())
        }),
        Err(failure) => Err(failure.message),
    };
    if result.is_err() {
        // a part-written catalog is only space; the next build writes another
        let _ = fs::remove_file(&new);
    }
    result
}

/// Whether `hay` holds `needle` (lowercase), ignoring ASCII case.
fn contains(hay: &str, needle: &str) -> bool {
    let (hay, needle) = (hay.as_bytes(), needle.as_bytes());
    needle.is_empty()
        || hay
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

/// The packages in the catalog's text matching every word of `query`, by
/// name or description, best first; the first [`RESULTS`], and how many
/// matched.
pub fn find_in(text: &str, query: &str) -> (Vec<Found>, usize) {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut found: Vec<Found> = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '\t');
            let attr = fields.next()?;
            let pname = fields.next().unwrap_or("");
            let version = fields.next().unwrap_or("");
            let description = fields.next().unwrap_or("");
            words
                .iter()
                .all(|word| {
                    contains(attr, word) || contains(pname, word) || contains(description, word)
                })
                .then(|| Found {
                    attr: attr.to_string(),
                    pname: pname.to_string(),
                    version: version.to_string(),
                    description: description.to_string(),
                })
        })
        .collect();
    let total = found.len();
    found.sort_by_cached_key(|found| rank(found, query));
    found.truncate(RESULTS);
    (found, total)
}

/// The catalog's packages with these attribute paths.
pub fn lookup_in(text: &str, attrs: &[String]) -> HashMap<String, Found> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '\t');
            let attr = fields.next()?;
            attrs.iter().any(|a| a == attr).then(|| {
                let found = Found {
                    attr: attr.to_string(),
                    pname: fields.next().unwrap_or("").to_string(),
                    version: fields.next().unwrap_or("").to_string(),
                    description: fields.next().unwrap_or("").to_string(),
                };
                (found.attr.clone(), found)
            })
        })
        .collect()
}

/// The best match for an attribute the catalog lacks, by its last part
/// (`python311Packages.foo` → `foo`), when there's one other than itself.
pub fn instead_of(text: &str, attr: &str) -> Option<String> {
    let last = attr.rsplit('.').next().unwrap_or(attr);
    find_in(text, last)
        .0
        .into_iter()
        .map(|found| found.attr)
        .find(|other| other != attr)
}

/// Searches the catalog file.
pub fn find(path: &Path, query: &str) -> Result<(Vec<Found>, usize), String> {
    let text = fs::read_to_string(path).map_err(|error| format!("the catalog: {error}"))?;
    Ok(find_in(&text, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_nixpkgs_input_is_followed_to_its_rev() {
        let lock = r#"{"nodes":{"nixpkgs_2":{"locked":{"rev":"26f4107e","narHash":"sha256-x/y="}},
            "root":{"inputs":{"nixpkgs":"nixpkgs_2","niri":"niri"}}},"root":"root","version":7}"#;
        assert_eq!(locked_rev(lock).as_deref(), Some("26f4107e"));
        let path = r#"{"nodes":{"nixpkgs":{"locked":{"narHash":"sha256-x/y="}},
            "root":{"inputs":{"nixpkgs":"nixpkgs"}}},"root":"root"}"#;
        assert_eq!(locked_rev(path).as_deref(), Some("sha256-x/y="));
        assert!(file_for(Path::new("/c"), "sha256-x/y=").ends_with("sha256-x_y=.tsv"));
        assert_eq!(locked_rev("{}"), None);
    }

    #[test]
    fn nix_search_json_streams_into_lines() {
        let json = r#"{"legacyPackages.x86_64-linux.ripgrep":{"description":"Fast\tgrep\nmore","pname":"ripgrep","version":"14.1.1"},
            "legacyPackages.x86_64-linux.python3Packages.rich":{"description":null,"pname":"rich","version":"13"},
            "legacyPackages.x86_64-linux.tests.cc-wrapper":{"description":"","pname":"cc-wrapper-test","version":""}}"#;
        let mut out = Vec::new();
        assert_eq!(convert(json.as_bytes(), &mut out).unwrap(), 2);
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text,
            "ripgrep\tripgrep\t14.1.1\tFast grep\npython3Packages.rich\trich\t13\t\n"
        );
        assert!(convert("[]".as_bytes(), Vec::new()).is_err());
    }

    #[test]
    fn a_gone_package_suggests_the_nearest_name() {
        let text = "pear-desktop\tpear-desktop\t3.11\tYouTube Music\n\
                    python313Packages.foo\tfoo\t1\tFoo\n";
        assert_eq!(
            instead_of(text, "python311Packages.foo").as_deref(),
            Some("python313Packages.foo")
        );
        assert_eq!(instead_of(text, "nothing-like-it"), None);
    }

    #[test]
    fn lookup_finds_exact_attributes() {
        let text = "spotify\tspotify\t1.2\tPlay music\nspotify-player\tspotify-player\t0.2\tTUI\n";
        let found = lookup_in(text, &["spotify".into(), "absent".into()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found["spotify"].version, "1.2");
    }

    #[test]
    fn every_word_matches_name_or_description_case_blind() {
        let text = "spotify\tspotify\t1.2\tPlay music from the Spotify database\n\
                    ncspot\tncspot\t1.1\tCross-platform ncurses Spotify client\n\
                    musikcube\tmusikcube\t3\tTerminal-based music player\n\
                    python3Packages.spotipy\tspotipy\t2\tLibrary for the Spotify Web API\n";
        let (found, total) = find_in(text, "Spotify");
        assert_eq!(total, 3);
        let order: Vec<&str> = found.iter().map(|f| f.attr.as_str()).collect();
        assert_eq!(order, ["spotify", "ncspot", "python3Packages.spotipy"]);
        let (found, _) = find_in(text, "music player");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attr, "musikcube");
        assert!(find_in(text, "nothing-like-it").0.is_empty());
    }

    #[test]
    fn search_results_are_named_by_attribute_top_level_first() {
        let json = r#"{"legacyPackages.x86_64-linux.bat-extras.batgrep":{"description":"Quickly search","pname":"batgrep","version":"2024"},
            "legacyPackages.x86_64-linux.ripgrep":{"description":"Fast grep\nmore","pname":"ripgrep","version":"14.1.1"}}"#;
        let found = parse_search(json, "grep").unwrap();
        assert_eq!(found[0].attr, "ripgrep");
        assert_eq!(found[0].pname, "ripgrep");
        assert_eq!(found[0].description, "Fast grep");
        assert_eq!(found[1].attr, "bat-extras.batgrep");
        assert!(parse_search("[]", "grep").is_err());
    }

    fn found(attr: &str, pname: &str) -> Found {
        Found {
            attr: attr.into(),
            pname: pname.into(),
            version: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn the_closest_name_ranks_first_and_sets_last() {
        let mut all = [
            found("python3Packages.spotify", "spotify"),
            found("musikcube", "musikcube"),
            found("spotify-player", "spotify-player"),
            found("ncspot", "ncspot"),
            found("spotify", "spotify"),
        ];
        all.sort_by_cached_key(|f| rank(f, "Spotify"));
        let order: Vec<&str> = all.iter().map(|f| f.attr.as_str()).collect();
        assert_eq!(
            order,
            [
                "spotify",
                "spotify-player",
                "ncspot",
                "musikcube",
                "python3Packages.spotify"
            ]
        );
        // naming the set puts it back in the running
        assert!(!rank(&all[4], "python3Packages.spotify").0);
    }
}
