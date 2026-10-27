//! Apps: finding packages and what's installed system-wide. Packages added
//! here go into `managed/settings.json` (`packages`, nixpkgs attribute
//! paths) and wait for Apply; the search reads the nixpkgs catalog
//! ([`super::catalog`]), or asks `nix search` while there's none; what's
//! installed, from Settings, the repo's `.nix` files and Flatpak, is one
//! list ([`super::inventory`]).

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Apps, Center, Column, DesktopApp, Rectangle, Row, Service, Size, TextInput, Widget};
use amane_kit::app::{field_box, text_field};
use amane_kit::app_icon::entry_tile;
use amane_kit::buttons::{PushButton, Style};
use amane_kit::icons::ICON_PACKAGE;
use amane_kit::layout::{boxed, icon};
use amane_kit::list::{List, ListItem, leading_side, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::tokens::sys;
use serde_json::Value;

use super::catalog::{self, Catalog, Named};
use super::controls::{self, Feedback};
use super::installed::Launchers;
use super::inventory::{self, FileAttrs};
use super::nixos::{Entry, Managed, Snapshot, failure, repo, run, strings};
use super::package::Details;
use crate::{PAGE_WIDTH, note, setting};

/// The registry id of the list of added packages.
pub const PACKAGES: &str = "packages";

/// The search field, which keeps its text under this id.
const SEARCH: &str = "settings.system.apps.search";

/// How long a search runs before it's said to be building the index.
const SLOW: Duration = Duration::from_secs(3);

/// How many results show.
pub const RESULTS: usize = 50;

/// How long typing pauses before the search runs.
const PAUSE: Duration = Duration::from_millis(150);

// ───── nixpkgs search ────────────────────────────────────────────────────────

/// A package `nix search` found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub attr: String,
    pub pname: String,
    pub version: String,
    pub description: String,
}

/// The last search: its query, results (the first [`RESULTS`] of `total`),
/// whether it's still running, and what was typed meanwhile.
pub struct Search {
    pub query: String,
    pub results: Vec<Found>,
    pub total: usize,
    pub running: bool,
    pub started: Option<Instant>,
    pub error: Option<String>,
    // typed while the catalog is built or a slow search fills nix's cache,
    // run after it
    pub queued: Option<String>,
}

static SEARCH_CHILD: Mutex<Option<Child>> = Mutex::new(None);

// the latest keystroke, so a pause that's been typed over doesn't search
static TYPED: AtomicU64 = AtomicU64::new(0);

impl Service for Search {
    fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            total: 0,
            running: false,
            started: None,
            error: None,
            queued: None,
        }
    }

    fn listen() {}
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

/// Whether a package is on the system already: added here, or installed by
/// a `.nix` file under the same name (only a top-level package can be,
/// there).
pub fn present(attr: &str, pname: &str, added: &[String], installed: &[&str]) -> bool {
    added.iter().any(|a| a == attr) || (!attr.contains('.') && installed.contains(&pname))
}

impl Search {
    /// Searches the configuration's nixpkgs for what the field says, now.
    pub fn run() {
        Self::search(TextInput::text(SEARCH).trim().to_string());
    }

    /// The field changed: search once typing pauses. A search filling nix's
    /// cache isn't cut short (that would start the minutes over); what was
    /// typed runs after it.
    pub fn typed(text: String) {
        let query = text.trim().to_string();
        let mine = TYPED.fetch_add(1, Ordering::Relaxed) + 1;
        thread::spawn(move || {
            thread::sleep(PAUSE);
            if TYPED.load(Ordering::Relaxed) != mine {
                return;
            }
            if query.chars().count() < 2 {
                let mut now = Self::write();
                if now.running {
                    now.queued = None;
                    now.quiet();
                } else {
                    *now = Self::new();
                }
                return;
            }
            {
                let mut now = Self::write();
                if now.query == query && (now.running || now.error.is_none()) {
                    now.quiet();
                    return;
                }
                if now.running && now.started.is_some_and(|at| at.elapsed() >= SLOW) {
                    now.queued = Some(query);
                    return;
                }
            }
            Self::search(query);
        });
    }

    /// The catalog is ready, or failed: runs what was typed meanwhile.
    pub fn resume() {
        let queued = {
            let mut now = Self::write();
            let queued = now.queued.take();
            if queued.is_none() {
                now.quiet();
            }
            queued
        };
        if let Some(query) = queued {
            Self::search(query);
        }
    }

    fn search(query: String) {
        if query.chars().count() < 2 {
            return;
        }
        Catalog::ensure();
        let (file, failed) = {
            let catalog = Catalog::read();
            (catalog.file(), catalog.error.is_some())
        };
        match file {
            Some(file) => Self::in_catalog(query, file),
            // until the catalog's there, or when it can't be built
            None if failed => Self::with_nix(query),
            None => {
                let mut now = Self::write();
                now.query = query.clone();
                now.running = false;
                now.error = None;
                now.queued = Some(query);
            }
        }
    }

    fn in_catalog(query: String, file: std::path::PathBuf) {
        {
            let mut now = Self::write();
            now.query = query.clone();
            now.running = true;
            now.started = Some(Instant::now());
            now.error = None;
            now.queued = None;
        }
        thread::spawn(move || {
            let result = catalog::find(&file, &query);
            let mut now = Self::write();
            if now.query != query {
                now.quiet();
                return;
            }
            now.running = false;
            match result {
                Ok((results, total)) => {
                    now.results = results;
                    now.total = total;
                }
                Err(error) => now.error = Some(error),
            }
        });
    }

    /// `nix search` for the query, when there's no catalog to read.
    fn with_nix(query: String) {
        {
            let mut now = Self::write();
            now.query = query.clone();
            now.running = true;
            now.started = Some(Instant::now());
            now.error = None;
            now.queued = None;
        }
        // a search still running is for an older query
        if let Some(mut old) = SEARCH_CHILD
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = old.kill();
        }

        thread::spawn(move || {
            let repo = repo();
            let child = Command::new("nix")
                .args([
                    "search",
                    "--json",
                    "--no-warn-dirty",
                    "--inputs-from",
                    &repo.to_string_lossy(),
                    "nixpkgs",
                    &query,
                ])
                .current_dir(&repo)
                .env("NO_COLOR", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
            let result = match child {
                Ok(child) => {
                    *SEARCH_CHILD.lock().unwrap_or_else(PoisonError::into_inner) = Some(child);
                    let child = SEARCH_CHILD
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take();
                    match child.map(Child::wait_with_output) {
                        Some(Ok(output)) if output.status.success() => {
                            parse_search(&String::from_utf8_lossy(&output.stdout), &query)
                        }
                        // nix says "no results" by exiting 1
                        Some(Ok(output))
                            if String::from_utf8_lossy(&output.stderr).contains("no results") =>
                        {
                            Ok(Vec::new())
                        }
                        Some(Ok(output)) => Err(failure(
                            &output.stderr,
                            &output.stdout,
                            output.status.code(),
                        )),
                        Some(Err(error)) => Err(error.to_string()),
                        // taken by a newer search
                        None => return,
                    }
                }
                Err(error) => Err(format!("nix: {error}")),
            };

            let next = {
                let mut now = Self::write();
                if now.query != query {
                    now.quiet();
                    return;
                }
                now.running = false;
                match result {
                    Ok(mut results) => {
                        now.total = results.len();
                        results.truncate(RESULTS);
                        now.results = results;
                    }
                    Err(error) => now.error = Some(error),
                }
                now.queued.take().filter(|next| *next != query)
            };
            if let Some(next) = next {
                Self::search(next);
            }
        });
    }
}

// ───── flatpak ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Flatpak {
    pub id: String,
    pub name: String,
    pub version: String,
    pub origin: String,
}

/// The Flatpak apps, as of the last look; `available` when flatpak is
/// installed at all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flatpaks {
    pub apps: Vec<Flatpak>,
    pub available: bool,
    pub error: Option<String>,
}

/// `flatpak list --columns=application,name,version,origin`: tab-separated.
pub fn parse_flatpaks(text: &str) -> Vec<Flatpak> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(Flatpak {
                id: parts.next()?.trim().to_string(),
                name: parts.next().unwrap_or("").trim().to_string(),
                version: parts.next().unwrap_or("").trim().to_string(),
                origin: parts.next().unwrap_or("").trim().to_string(),
            })
        })
        .filter(|app| !app.id.is_empty())
        .collect()
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

fn look() -> Flatpaks {
    if !on_path("flatpak") {
        return Flatpaks::default();
    }
    match run(
        "flatpak",
        &["list", "--app", "--columns=application,name,version,origin"],
        None,
    ) {
        Ok(text) => Flatpaks {
            apps: parse_flatpaks(&text),
            available: true,
            error: None,
        },
        Err(error) => Flatpaks {
            apps: Vec::new(),
            available: true,
            error: Some(error),
        },
    }
}

impl Service for Flatpaks {
    fn new() -> Self {
        Self::default()
    }

    fn interval() -> Duration {
        Duration::from_secs(30)
    }

    fn update(&mut self) -> bool {
        let fresh = look();
        let changed = fresh != *self;
        *self = fresh;
        changed
    }
}

impl Flatpaks {
    pub fn refresh() {
        thread::spawn(|| {
            let fresh = look();
            let mut now = Self::write();
            if fresh == *now {
                now.quiet();
            } else {
                *now = fresh;
            }
        });
    }
}

/// An uninstall under way, and what the last one said when it failed.
pub struct Removal {
    pub busy: Option<String>,
    pub error: Option<String>,
}

impl Service for Removal {
    fn new() -> Self {
        Self {
            busy: None,
            error: None,
        }
    }

    fn listen() {}
}

pub fn uninstall(id: String) {
    {
        let mut now = Removal::write();
        if now.busy.is_some() {
            now.quiet();
            return;
        }
        now.busy = Some(id.clone());
        now.error = None;
    }
    thread::spawn(move || {
        // flatpak asks through polkit itself for a system-wide app
        let result = run(
            "flatpak",
            &["uninstall", "-y", "--noninteractive", &id],
            None,
        );
        {
            let mut now = Removal::write();
            now.busy = None;
            if let Err(why) = result {
                now.error = Some(format!("{id}: {why}"));
            }
        }
        Flatpaks::refresh();
    });
}

// ───── the page ──────────────────────────────────────────────────────────────

/// Writes the list with `name` added or taken out.
pub fn change_packages(entry: &Entry, managed: &Managed, name: &str, add: bool) {
    let mut names = managed.list(&entry.id, &entry.value);
    names.retain(|n| n != name);
    if add {
        names.push(name.to_string());
    }
    controls::set(
        entry,
        Value::Array(names.into_iter().map(Value::from).collect()),
    );
}

/// Where an added package stands against what was last applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Installed,
    Adding,
    Removing,
}

/// The packages added here, and the ones a pending change takes out, each
/// with where it stands: the list on disk against HEAD's (what Apply last
/// committed).
pub fn standings(current: &[String], committed: &[String]) -> Vec<(String, Standing)> {
    let mut rows: Vec<(String, Standing)> = current
        .iter()
        .map(|name| {
            let standing = if committed.contains(name) {
                Standing::Installed
            } else {
                Standing::Adding
            };
            (name.clone(), standing)
        })
        .collect();
    rows.extend(
        committed
            .iter()
            .filter(|name| !current.contains(name))
            .map(|name| (name.clone(), Standing::Removing)),
    );
    rows
}

/// The packages added here since the last Apply: what an Apply now installs.
pub fn arriving(managed: &Managed, snapshot: &Snapshot) -> Vec<String> {
    let Some(entry) = snapshot.entry(PACKAGES) else {
        return Vec::new();
    };
    let current = managed.list(&entry.id, &entry.value);
    let committed = strings(managed.committed.get(&entry.id));
    standings(&current, &committed)
        .into_iter()
        .filter(|(_, standing)| *standing == Standing::Adding)
        .map(|(attr, _)| attr)
        .collect()
}

/// Where one package stands, when it's added here or a pending change takes
/// it out.
pub fn standing(attr: &str, managed: &Managed, entry: Option<&Entry>) -> Option<Standing> {
    let entry = entry?;
    let current = managed.list(&entry.id, &entry.value);
    let committed = strings(managed.committed.get(&entry.id));
    standings(&current, &committed)
        .into_iter()
        .find(|(name, _)| name == attr)
        .map(|(_, standing)| standing)
}

/// What draws the rows: the installed apps, and what the catalog knows of the
/// packages added here.
pub struct Looks<'a> {
    pub launchers: &'a Launchers,
    pub apps: &'a Apps,
    pub named: &'a Named,
}

impl Looks<'_> {
    pub fn app(&self, pname: &str, version: &str) -> Option<DesktopApp> {
        self.launchers.app(self.apps, pname, version)
    }
}

/// A row's leading tile: the app's icon, else a package glyph the same size.
pub fn leading(app: Option<&DesktopApp>) -> Rectangle {
    let side = leading_side();
    match app {
        Some(app) => entry_tile(app, side),
        None => Rectangle::new()
            .width(Size::Fixed(side))
            .height(Size::Fixed(side))
            .align_child(Center, Center)
            .child(icon(
                ICON_PACKAGE,
                sys::size::icon::m(),
                material::label(Level::Tertiary),
            )),
    }
}

fn search_section(
    entry: Option<&Entry>,
    managed: &Managed,
    snapshot: &Snapshot,
    search: &Search,
    catalog: &Catalog,
    looks: &Looks,
    checking: Option<&str>,
) -> Box<dyn Widget> {
    let placeholder = if catalog.ready {
        format!("Search {} packages…", thousands(catalog.count))
    } else {
        String::from("Search nixpkgs…")
    };
    let field = text_field(SEARCH, &placeholder)
        .on_change(Search::typed)
        .on_submit(|_| Search::run());
    let mut lines: Vec<Box<dyn Widget>> = vec![boxed(
        Row::new(vec![
            boxed(field_box(SEARCH, field, 240.0)),
            boxed(
                PushButton::new("settings.apps.search", "Search")
                    .disabled(search.running)
                    .on_press(Search::run)
                    .build(),
            ),
        ])
        .gap(sys::space::gap::s())
        .align(Center),
    )];

    if let Some(since) = catalog.building {
        let secs = since.elapsed().as_secs();
        lines.push(boxed(note(format!(
            "Preparing the app catalog… {}:{:02}. Once per nixpkgs update; the first can take a few minutes.",
            secs / 60,
            secs % 60
        ))));
    } else if let Some(error) = &catalog.error {
        lines.push(boxed(
            note(format!(
                "Searching with nix: the catalog couldn't be built. {error}"
            ))
            .color(sys::color::status::critical::text()),
        ));
    }

    // the catalog answers at once, so the last results stay until it has
    if search.running && !catalog.ready {
        let slow = search.started.is_some_and(|at| at.elapsed() >= SLOW);
        lines.push(boxed(note(if slow {
            "Building the search index… the first search after an update takes a few minutes."
        } else {
            "Searching…"
        })));
    } else if let Some(error) = &search.error {
        lines.push(boxed(
            note(error.clone()).color(sys::color::status::critical::text()),
        ));
    } else if !search.query.is_empty() && search.queued.is_none() {
        let added: Vec<String> = entry
            .map(|e| managed.list(&e.id, &e.value))
            .unwrap_or_default();
        let installed: Vec<&str> = snapshot
            .system_packages
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        let items: Vec<ListItem> = search
            .results
            .iter()
            .map(|found| {
                let there = present(&found.attr, &found.pname, &added, &installed);
                let app = there
                    .then(|| looks.app(&found.pname, &found.version))
                    .flatten();
                let mut headline = app.as_ref().map_or(found.attr.clone(), |app| {
                    format!("{} ({})", app.name(), found.attr)
                });
                if !found.version.is_empty() {
                    headline.push_str(&format!(" · {}", found.version));
                }
                let mine = checking == Some(found.attr.as_str());
                let (attr, open) = (found.attr.clone(), found.attr.clone());
                let e = entry.cloned();
                ListItem::new(format!("settings.apps.found.{}", found.attr), headline)
                    .leading_widget(leading(app.as_ref()))
                    .supporting(found.description.clone())
                    .bare(true)
                    .on_press(move || Details::open(&open))
                    .trailing(
                        PushButton::new(
                            format!("settings.apps.add.{}", found.attr),
                            if there {
                                "Added"
                            } else if mine {
                                "Checking…"
                            } else {
                                "Add"
                            },
                        )
                        .style(Style::Tonal)
                        .disabled(there || checking.is_some() || entry.is_none_or(|e| !e.managed()))
                        .on_press(move || {
                            let Some(e) = e.clone() else { return };
                            let attr2 = attr.clone();
                            // what it'd build, read first; added as the list is then
                            Details::check_then_add(&attr, move || {
                                let managed = Managed::read().clone();
                                change_packages(&e, &managed, &attr2, true);
                            });
                        })
                        .build(),
                    )
            })
            .collect();
        lines.push(boxed(
            List::new(items)
                .empty(if search.running {
                    String::from("Searching…")
                } else {
                    format!("Nothing in nixpkgs matches \"{}\".", search.query)
                })
                .build(),
        ));
        if search.total > search.results.len() {
            lines.push(boxed(note(format!(
                "The best {} of {} matches: type more to narrow them down.",
                search.results.len(),
                search.total
            ))));
        }
    }

    boxed(setting(
        "Add App",
        Column::new(lines).gap(sys::space::gap::xs()),
    ))
}

/// 108517 as "108,517".
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub fn page() -> Column {
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let feedback = Feedback::read();
    let search = Search::read();
    Catalog::ensure();
    let catalog = Catalog::read();
    let details = Details::read();
    let (launchers, apps, named) = (Launchers::read(), Apps::read(), Named::read());
    let entry = snapshot.entry(PACKAGES);
    if let (Some(file), Some(entry)) = (catalog.file(), entry) {
        Named::ensure(file, managed.list(&entry.id, &entry.value));
    }
    if let Some(file) = catalog.file() {
        FileAttrs::ensure(file, &snapshot);
    }
    let looks = Looks {
        launchers: &launchers,
        apps: &apps,
        named: &named,
    };

    with_row_width(PAGE_WIDTH, || {
        let parts: Vec<Box<dyn Widget>> = vec![
            search_section(
                entry,
                &managed,
                &snapshot,
                &search,
                &catalog,
                &looks,
                details.checking.as_deref(),
            ),
            inventory::section(entry, &managed, &feedback, &snapshot, &looks),
        ];

        Column::new(parts)
            .gap(sys::space::gap::l())
            .width(Size::Parent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn present_by_attribute_or_installed_name() {
        let installed = ["gh", "requests"];
        // github-cli installs as pname gh
        assert!(present("github-cli", "gh", &[], &installed));
        // a package in a set isn't the top-level one of the same name
        assert!(!present(
            "python3Packages.requests",
            "requests",
            &[],
            &installed
        ));
        assert!(present(
            "python3Packages.requests",
            "requests",
            &["python3Packages.requests".into()],
            &installed
        ));
        assert!(!present("ripgrep", "ripgrep", &[], &installed));
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(108517), "108,517");
    }

    #[test]
    fn standings_against_the_last_commit() {
        let current = vec!["ripgrep".to_string(), "bat".to_string()];
        let committed = vec!["ripgrep".to_string(), "htop".to_string()];
        assert_eq!(
            standings(&current, &committed),
            [
                ("ripgrep".to_string(), Standing::Installed),
                ("bat".to_string(), Standing::Adding),
                ("htop".to_string(), Standing::Removing),
            ]
        );
    }

    #[test]
    fn flatpak_list_lines_are_apps() {
        let apps = parse_flatpaks("app.zen_browser.zen\tZen\t1.23.2b\tflathub\n\n");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Zen");
        assert_eq!(apps[0].origin, "flathub");
    }
}
