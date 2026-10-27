//! The nixpkgs catalog the Apps page searches: every package `nix search`
//! finds in the configuration's nixpkgs (less nixpkgs' own `tests`), a line each (`attr`, `pname`, `version`,
//! `description`, tab-separated), in a file per locked nixpkgs revision
//! under `$XDG_CACHE_HOME/amane-shell/nixpkgs`. It's built once per nixpkgs
//! update from `nix search --json … ^`, streamed straight into the file, and
//! each search reads the file again (from the page cache, a few hundred
//! thousand lines in tens of milliseconds) rather than the app keeping the
//! packages in memory.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use amane::Service;
use serde::Deserialize;
use serde::de::{self, Deserializer as _, MapAccess, Visitor};
use serde_json::Value;

use super::apps::{Found, RESULTS, Search, attr_of, rank};
use super::nixos::{failure, repo};

/// How often the page looks at `flake.lock` again for a new nixpkgs.
const RECHECK: Duration = Duration::from_secs(10);

static CHECKED: Mutex<Option<Instant>> = Mutex::new(None);

/// Where the catalog stands: built for `rev` (`ready`, `count` packages),
/// being built since `building`, or failed with `error`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub rev: Option<String>,
    pub ready: bool,
    pub count: usize,
    pub building: Option<Instant>,
    pub error: Option<String>,
}

impl Service for Catalog {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

/// `$XDG_CACHE_HOME/amane-shell/nixpkgs`.
fn cache_dir() -> PathBuf {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_default();

    cache.join("amane-shell").join("nixpkgs")
}

fn path_for(rev: &str) -> PathBuf {
    // a narHash has slashes in it
    cache_dir().join(format!("{}.tsv", rev.replace('/', "_")))
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

impl Catalog {
    /// Makes sure there's a catalog for the nixpkgs `flake.lock` pins,
    /// building it when there isn't; looks at most every [`RECHECK`].
    pub fn ensure() {
        {
            let mut checked = CHECKED.lock().unwrap_or_else(PoisonError::into_inner);
            if checked.is_some_and(|at| at.elapsed() < RECHECK) {
                return;
            }
            *checked = Some(Instant::now());
        }
        thread::spawn(Self::check);
    }

    fn check() {
        if Self::read().building.is_some() {
            return;
        }
        let rev = match fs::read_to_string(repo().join("flake.lock")) {
            Ok(lock) => locked_rev(&lock),
            Err(error) => {
                Self::fail(format!("flake.lock: {error}"));
                return;
            }
        };
        let Some(rev) = rev else {
            Self::fail(String::from("flake.lock pins no nixpkgs input"));
            return;
        };
        {
            let now = Self::read();
            if now.ready && now.rev.as_deref() == Some(rev.as_str()) {
                return;
            }
        }

        let path = path_for(&rev);
        match fs::read(&path) {
            Ok(bytes) => {
                let count = bytes.iter().filter(|&&b| b == b'\n').count();
                *Self::write() = Self {
                    rev: Some(rev),
                    ready: true,
                    count,
                    building: None,
                    error: None,
                };
                Search::resume();
            }
            Err(_) => Self::build(rev, path),
        }
    }

    fn fail(error: String) {
        let mut now = Self::write();
        now.building = None;
        now.ready = false;
        now.error = Some(error);
        drop(now);
        // the page searches with nix instead
        Search::resume();
    }

    fn build(rev: String, path: PathBuf) {
        {
            let mut now = Self::write();
            now.rev = Some(rev.clone());
            now.ready = false;
            now.building = Some(Instant::now());
            now.error = None;
        }
        // redraw each second, for the time it's taken so far
        thread::spawn(|| {
            loop {
                thread::sleep(Duration::from_secs(1));
                if Self::read().building.is_none() {
                    break;
                }
                drop(Self::write());
            }
        });

        match write_catalog(&path) {
            Ok(count) => {
                // the catalogs of earlier revisions
                if let Ok(entries) = fs::read_dir(cache_dir()) {
                    for entry in entries.flatten() {
                        if entry.path() != path {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
                *Self::write() = Self {
                    rev: Some(rev),
                    ready: true,
                    count,
                    building: None,
                    error: None,
                };
                Search::resume();
            }
            Err(error) => Self::fail(error),
        }
    }

    /// The file to search, while there's one for the pinned nixpkgs.
    pub fn file(&self) -> Option<PathBuf> {
        self.rev.as_deref().filter(|_| self.ready).map(path_for)
    }
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
fn convert(json: impl Read, out: impl Write) -> Result<usize, String> {
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
fn write_catalog(path: &Path) -> Result<usize, String> {
    let repo = repo();
    let folder = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let new = path.with_extension("tsv.new");

    let mut child = Command::new("nix")
        .args([
            "search",
            "--json",
            "--no-warn-dirty",
            "--inputs-from",
            &repo.to_string_lossy(),
            "nixpkgs",
            "^",
        ])
        .current_dir(&repo)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("nix: {error}"))?;

    // read alongside, so a full stderr pipe never stalls nix
    let mut stderr = child.stderr.take();
    let errors = thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(stderr) = stderr.as_mut() {
            let _ = stderr.read_to_end(&mut text);
        }
        text
    });

    let stdout = child.stdout.take().ok_or("nix: no output")?;
    let written = File::create(&new)
        .map_err(|error| error.to_string())
        .and_then(|file| convert(stdout, BufWriter::new(file)));
    let status = child.wait().map_err(|error| error.to_string())?;
    let errors = errors.join().unwrap_or_default();

    // nix's own error first: a failed search also cuts the JSON short
    let result = if status.success() {
        written.and_then(|count| {
            fs::rename(&new, path)
                .map(|()| count)
                .map_err(|error| error.to_string())
        })
    } else {
        Err(failure(&errors, &[], status.code()))
    };
    if result.is_err() {
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

/// What the catalog says of the packages added in Settings, so their rows
/// can show a name, version and icon; and for one it hasn't got (renamed or
/// dropped), the closest package by the last part of its name.
#[derive(Default)]
pub struct Named {
    pub found: HashMap<String, Found>,
    pub instead: HashMap<String, String>,
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

impl Service for Named {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

static NAMING: Mutex<Option<(PathBuf, Vec<String>)>> = Mutex::new(None);

impl Named {
    /// Looks the packages up when the list or the catalog changed.
    pub fn ensure(file: PathBuf, attrs: Vec<String>) {
        let key = (file, attrs);
        {
            let mut naming = NAMING.lock().unwrap_or_else(PoisonError::into_inner);
            if naming.as_ref() == Some(&key) {
                return;
            }
            *naming = Some(key.clone());
        }
        thread::spawn(move || {
            let text = fs::read_to_string(&key.0).unwrap_or_default();
            let found = lookup_in(&text, &key.1);
            let instead = key
                .1
                .iter()
                .filter(|attr| !found.contains_key(*attr))
                .filter_map(|attr| Some((attr.clone(), instead_of(&text, attr)?)))
                .collect();
            let mut now = Self::write();
            now.found = found;
            now.instead = instead;
        });
    }
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
        assert!(path_for("sha256-x/y=").ends_with("sha256-x_y=.tsv"));
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
}
