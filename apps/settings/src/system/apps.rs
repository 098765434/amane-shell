//! Apps: what's installed system-wide. Packages added here go into
//! `managed/settings.json` (`packages`, nixpkgs attribute paths) and wait
//! for Apply; the ones the repo's `.nix` files list are shown as they are;
//! Flatpaks are listed from `flatpak list`, with Uninstall.

use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use amane::{Center, Column, Row, Service, Size, TextInput, Widget};
use amane_kit::app::{Dialog, field_box, text_field};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::layout::boxed;
use amane_kit::list::{List, ListItem, trailing_text, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::tokens::sys;
use serde_json::Value;

use super::controls::{self, Feedback};
use super::nixos::{Entry, MANAGED_MODULE, Managed, Snapshot, failure, repo, run};
use crate::{PAGE_WIDTH, note, setting};

/// The registry id of the list of added packages.
const PACKAGES: &str = "packages";

/// The search field, which keeps its text under this id.
const SEARCH: &str = "settings.system.apps.search";

/// How long a search runs before it's said to be building the index.
const SLOW: Duration = Duration::from_secs(3);

/// How many results show.
const RESULTS: usize = 50;

// ───── nixpkgs search ────────────────────────────────────────────────────────

/// A package `nix search` found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub attr: String,
    pub version: String,
    pub description: String,
}

/// The last search: its query, results, and whether it's still running.
pub struct Search {
    pub query: String,
    pub results: Vec<Found>,
    pub running: bool,
    pub started: Option<Instant>,
    pub error: Option<String>,
}

static SEARCH_CHILD: Mutex<Option<Child>> = Mutex::new(None);

impl Service for Search {
    fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            running: false,
            started: None,
            error: None,
        }
    }

    fn listen() {}
}

/// `nix search --json` output: `legacyPackages.<system>.<attr>` to
/// `{pname, version, description}`; shortest attribute paths first.
pub fn parse_search(json: &str) -> Result<Vec<Found>, String> {
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
            // past "legacyPackages.x86_64-linux."
            let attr = key
                .strip_prefix("legacyPackages.")
                .and_then(|rest| rest.split_once('.'))
                .map_or(key.as_str(), |(_, attr)| attr);
            Found {
                attr: attr.to_string(),
                version: text("version"),
                description: text("description").lines().next().unwrap_or("").to_string(),
            }
        })
        .collect();
    found.sort_by(|a, b| (a.attr.len(), &a.attr).cmp(&(b.attr.len(), &b.attr)));
    found.truncate(RESULTS);
    Ok(found)
}

impl Search {
    /// Searches the configuration's nixpkgs for what the field says.
    pub fn run() {
        let query = TextInput::text(SEARCH).trim().to_string();
        if query.chars().count() < 2 {
            return;
        }
        {
            let mut now = Self::write();
            now.query = query.clone();
            now.running = true;
            now.started = Some(Instant::now());
            now.error = None;
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
                            parse_search(&String::from_utf8_lossy(&output.stdout))
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

            let mut now = Self::write();
            if now.query != query {
                now.quiet();
                return;
            }
            now.running = false;
            match result {
                Ok(results) => now.results = results,
                Err(error) => now.error = Some(error),
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

fn uninstall(id: String) {
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
fn change_packages(entry: &Entry, managed: &Managed, name: &str, add: bool) {
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

fn added_section(entry: Option<&Entry>, managed: &Managed, feedback: &Feedback) -> Box<dyn Widget> {
    let Some(entry) = entry else {
        return boxed(setting(
            "Added in Settings",
            note("The registry has no `packages` entry: nothing can be added from here."),
        ));
    };
    let names = managed.list(&entry.id, &entry.value);
    let items: Vec<ListItem> = names
        .iter()
        .map(|name| {
            let (e, m, n) = (entry.clone(), managed.clone(), name.clone());
            ListItem::new(format!("settings.apps.added.{name}"), name)
                .bare(true)
                .trailing(
                    PushButton::new(format!("settings.apps.remove.{name}"), "Remove")
                        .style(Style::Plain)
                        .disabled(!entry.managed())
                        .on_press(move || change_packages(&e, &m, &n, false))
                        .build(),
                )
        })
        .collect();

    let mut lines: Vec<Box<dyn Widget>> = vec![boxed(
        List::new(items)
            .empty("Nothing added here yet: search below.")
            .build(),
    )];
    lines.extend(controls::notes(entry, managed, feedback));
    boxed(setting(
        &entry.label,
        Column::new(lines).gap(sys::space::gap::xs()),
    ))
}

fn search_section(
    entry: Option<&Entry>,
    managed: &Managed,
    snapshot: &Snapshot,
    search: &Search,
) -> Box<dyn Widget> {
    let field = text_field(SEARCH, "Search nixpkgs…").on_submit(|_| Search::run());
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

    if search.running {
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
    } else if !search.query.is_empty() {
        let have: Vec<String> = entry
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
                let last = found.attr.rsplit('.').next().unwrap_or(&found.attr);
                let there = have.contains(&found.attr) || installed.contains(&last);
                let mut headline = found.attr.clone();
                if !found.version.is_empty() {
                    headline.push_str(&format!(" · {}", found.version));
                }
                if !found.description.is_empty() {
                    headline.push_str(&format!(" — {}", found.description));
                }
                let attr = found.attr.clone();
                let (e, m) = (entry.cloned(), managed.clone());
                ListItem::new(format!("settings.apps.found.{}", found.attr), headline)
                    .bare(true)
                    .trailing(
                        PushButton::new(
                            format!("settings.apps.add.{}", found.attr),
                            if there { "Added" } else { "Add" },
                        )
                        .style(Style::Tonal)
                        .disabled(there || entry.is_none_or(|e| !e.managed()))
                        .on_press(move || {
                            if let Some(e) = &e {
                                change_packages(e, &m, &attr, true);
                            }
                        })
                        .build(),
                    )
            })
            .collect();
        lines.push(boxed(
            List::new(items)
                .empty(format!("Nothing in nixpkgs matches \"{}\".", search.query))
                .build(),
        ));
    }

    boxed(setting(
        "Add App",
        Column::new(lines).gap(sys::space::gap::xs()),
    ))
}

fn system_sections(snapshot: &Snapshot) -> Vec<Box<dyn Widget>> {
    // by the file that lists them; ours are shown above
    let mut by_file: BTreeMap<&str, Vec<&super::nixos::SystemPackage>> = BTreeMap::new();
    for package in &snapshot.system_packages {
        if package.file == MANAGED_MODULE || package.file == "modules/options.nix" {
            continue;
        }
        by_file
            .entry(package.file.as_str())
            .or_default()
            .push(package);
    }

    // apps.nix, the one that's meant for apps, first
    let mut files: Vec<_> = by_file.into_iter().collect();
    files.sort_by_key(|(file, _)| (*file != "modules/apps.nix", *file));

    files
        .into_iter()
        .map(|(file, packages)| {
            let mut packages = packages;
            packages.sort_by(|a, b| a.name.cmp(&b.name));
            let items: Vec<ListItem> = packages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    ListItem::new(format!("settings.apps.system.{file}.{i}"), &p.name)
                        .bare(true)
                        .trailing(trailing_text(
                            p.version.clone(),
                            material::label(Level::Secondary),
                        ))
                })
                .collect();
            boxed(setting(
                &format!("In {file}"),
                Column::new(vec![
                    boxed(List::new(items).build()),
                    boxed(note(format!(
                        "Listed in {file}: change them there, then Apply."
                    ))),
                ])
                .gap(sys::space::gap::xs()),
            ))
        })
        .collect()
}

fn flatpak_section(flatpaks: &Flatpaks, removal: &Removal) -> Option<Box<dyn Widget>> {
    if !flatpaks.available {
        return None;
    }
    let items: Vec<ListItem> = flatpaks
        .apps
        .iter()
        .map(|app| {
            let (id, name) = (app.id.clone(), app.name.clone());
            let busy = removal.busy.as_deref() == Some(app.id.as_str());
            ListItem::new(
                format!("settings.apps.flatpak.{}", app.id),
                format!("{} · {} · {}", app.name, app.version, app.origin),
            )
            .bare(true)
            .trailing(
                PushButton::new(
                    format!("settings.apps.uninstall.{}", app.id),
                    if busy { "Removing…" } else { "Uninstall…" },
                )
                .style(Style::Plain)
                .disabled(removal.busy.is_some())
                .on_press(move || {
                    let id = id.clone();
                    Dialog::new(
                        "settings.apps.uninstall-dialog",
                        format!("Uninstall {name}?"),
                    )
                    .message("Its data in ~/.var/app stays until you delete it.")
                    .button("Cancel", || {})
                    .destructive("Uninstall", move || uninstall(id.clone()))
                    .open("main");
                })
                .build(),
            )
        })
        .collect();

    let mut lines: Vec<Box<dyn Widget>> =
        vec![boxed(List::new(items).empty("No Flatpak apps.").build())];
    if let Some(error) = removal.error.as_ref().or(flatpaks.error.as_ref()) {
        lines.push(boxed(
            note(error.clone()).color(sys::color::status::critical::text()),
        ));
    }
    lines.push(boxed(note(
        "Installed with `flatpak install`, outside the configuration.",
    )));
    Some(boxed(setting(
        "Flatpak",
        Column::new(lines).gap(sys::space::gap::xs()),
    )))
}

pub fn page() -> Column {
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let feedback = Feedback::read();
    let search = Search::read();
    let flatpaks = Flatpaks::read();
    let removal = Removal::read();
    let entry = snapshot.entry(PACKAGES);

    with_row_width(PAGE_WIDTH, || {
        let mut parts: Vec<Box<dyn Widget>> = vec![
            added_section(entry, &managed, &feedback),
            search_section(entry, &managed, &snapshot, &search),
        ];
        parts.extend(system_sections(&snapshot));
        parts.extend(flatpak_section(&flatpaks, &removal));

        Column::new(parts)
            .gap(sys::space::gap::l())
            .width(Size::Parent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_results_are_named_by_attribute_shortest_first() {
        let json = r#"{"legacyPackages.x86_64-linux.bat-extras.batgrep":{"description":"Quickly search","pname":"batgrep","version":"2024"},
            "legacyPackages.x86_64-linux.ripgrep":{"description":"Fast grep\nmore","pname":"ripgrep","version":"14.1.1"}}"#;
        let found = parse_search(json).unwrap();
        assert_eq!(found[0].attr, "ripgrep");
        assert_eq!(found[0].description, "Fast grep");
        assert_eq!(found[1].attr, "bat-extras.batgrep");
        assert!(parse_search("[]").is_err());
    }

    #[test]
    fn flatpak_list_lines_are_apps() {
        let apps = parse_flatpaks("app.zen_browser.zen\tZen\t1.23.2b\tflathub\n\n");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Zen");
        assert_eq!(apps[0].origin, "flathub");
    }
}
