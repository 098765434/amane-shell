//! Installed: everything on the system in one list, whoever put it there:
//! the packages added in Settings (with where they stand against the last
//! Apply), the ones the repo's `.nix` files list, and the Flatpaks. A filter
//! field and *All · Apps · Command-line · Flatpak* narrow it down; an app is a
//! package with an entry in the launcher. Each row has the one thing that can
//! be done to it here: Remove (or Undo), Move to Settings, or Uninstall.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, Once};
use std::thread;
use std::time::SystemTime;

use amane::{DesktopApp, Service, TextInput};
use amane_kit::app::{Dialog, field_box, text_field};
use amane_kit::buttons::PushButton;
use amane_kit::dropdown::Dropdown;
use amane_kit::form::{FormRow, Group};
use amane_kit::layout::boxed;

use super::adopt;
use super::apps::Found;
use super::apps::{
    Flatpaks, Looks, Removal, Standing, change_packages, icon_side, leading, standings, uninstall,
};
use super::catalog::lookup_in;
use super::controls::{self, Feedback};
use super::nixos::{Entry, MANAGED_MODULE, Managed, Snapshot, repo, strings};
use super::package::Details;
use crate::group;
use amane_kit::sync::Locked as _;

/// The filter field, which keeps its text under this id.
const FILTER: &str = "settings.system.apps.filter";

/// Which kind the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Show {
    #[default]
    All,
    Apps,
    Tools,
    Flatpaks,
}

const SHOWS: [Show; 4] = [Show::All, Show::Apps, Show::Tools, Show::Flatpaks];

impl Show {
    fn label(self) -> &'static str {
        match self {
            Show::All => "All",
            Show::Apps => "Apps",
            Show::Tools => "Command-line",
            Show::Flatpaks => "Flatpak",
        }
    }
}

/// The kind picked; the filter's text lives with its field.
#[derive(Default)]
pub struct Filter {
    pub show: Show,
}

impl Service for Filter {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

/// Who put a package there.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Settings(Standing),
    File(String),
    Flatpak { id: String, origin: String },
}

/// One row's worth.
#[derive(Clone, Debug)]
pub struct Item {
    pub name: String,
    /// The nixpkgs attribute path, when it's known.
    pub attr: Option<String>,
    pub pname: String,
    pub version: String,
    pub description: String,
    pub source: Source,
    pub app: Option<DesktopApp>,
    /// Whether it has an entry in the launcher (`app`, or a Flatpak's).
    pub launcher: bool,
    /// Added in Settings but not in this nixpkgs: left out of the build,
    /// and what the catalog has by that name instead.
    pub gone: Option<Option<String>>,
}

impl Item {
    pub fn is_flatpak(&self) -> bool {
        matches!(self.source, Source::Flatpak { .. })
    }

    /// An app: a Flatpak, or a package with an entry in the launcher.
    pub fn is_app(&self) -> bool {
        self.is_flatpak() || self.launcher
    }

    pub fn shown_by(&self, show: Show) -> bool {
        match show {
            Show::All => true,
            Show::Apps => self.is_app(),
            Show::Tools => !self.is_app(),
            Show::Flatpaks => self.is_flatpak(),
        }
    }

    /// Whether every word of the filter is in its name, attribute or
    /// description, ignoring case.
    pub fn matches(&self, filter: &str) -> bool {
        let haystack = format!(
            "{} {} {} {}",
            self.name,
            self.attr.as_deref().unwrap_or(""),
            self.pname,
            self.description
        )
        .to_lowercase();
        filter
            .split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }

    /// Pending changes first, then by name.
    fn order(&self) -> (bool, String) {
        let pending = matches!(
            self.source,
            Source::Settings(Standing::Adding | Standing::Removing)
        );
        (!pending, self.name.to_lowercase())
    }
}

// ───── which attribute a .nix file's package is ────────────────────────────────

/// The bare names a `.nix` file lists on lines of their own (`ripgrep`,
/// `foot  # terminal`), `pkgs.` taken off: the packages it might install.
pub fn listed_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let code = line.split('#').next().unwrap_or("").trim();
            let name = code.strip_prefix("pkgs.").unwrap_or(code);
            let plain = name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '\'' | '.'));
            plain.then(|| name.to_string())
        })
        .collect()
}

/// Each `.nix` file's packages by `pname`, to their attribute paths: the
/// names the file lists, looked up in the catalog. `github-cli` is installed
/// as `gh`, so the snapshot's `pname` alone can't say.
#[derive(Default)]
pub struct FileAttrs {
    pub by_pname: HashMap<(String, String), String>,
}

impl Service for FileAttrs {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

type FilesKey = (PathBuf, Vec<(String, Option<SystemTime>)>);

static READING: Mutex<Option<FilesKey>> = Mutex::new(None);

impl FileAttrs {
    /// Reads the files again when they or the catalog changed.
    pub fn ensure(catalog: PathBuf, snapshot: &Snapshot) {
        let mut files: Vec<String> = snapshot
            .system_packages
            .iter()
            .map(|p| p.file.clone())
            .filter(|f| f != MANAGED_MODULE && f != "modules/options.nix")
            .collect();
        files.sort();
        files.dedup();
        let root = repo();
        let stamped: Vec<(String, Option<SystemTime>)> = files
            .into_iter()
            .map(|file| {
                let modified = fs::metadata(root.join(&file))
                    .and_then(|meta| meta.modified())
                    .ok();
                (file, modified)
            })
            .collect();
        let key = (catalog, stamped);
        {
            let mut reading = READING.locked();
            if reading.as_ref() == Some(&key) {
                return;
            }
            *reading = Some(key.clone());
        }
        thread::spawn(move || {
            let Ok(catalog) = fs::read_to_string(&key.0) else {
                return;
            };
            let mut by_pname = HashMap::new();
            for (file, _) in &key.1 {
                let names = fs::read_to_string(root.join(file))
                    .map(|text| listed_names(&text))
                    .unwrap_or_default();
                let found = lookup_in(&catalog, &names);
                // in the file's order, so the first name for a pname wins
                for name in names {
                    if let Some(found) = found.get(&name) {
                        by_pname
                            .entry((file.clone(), found.pname.clone()))
                            .or_insert(name);
                    }
                }
            }
            Self::write().by_pname = by_pname;
        });
    }
}

// ───── the list ──────────────────────────────────────────────────────────────

/// Every package and Flatpak, as rows.
pub fn items(
    entry: Option<&Entry>,
    managed: &Managed,
    snapshot: &Snapshot,
    flatpaks: &Flatpaks,
    looks: &Looks,
    file_attrs: &FileAttrs,
) -> Vec<Item> {
    let mut items = Vec::new();

    if let Some(entry) = entry {
        let current = managed.list(&entry.id, &entry.value);
        let committed = strings(managed.committed.get(&entry.id));
        for (attr, standing) in standings(&current, &committed) {
            let found = looks.named.found.get(&attr).cloned().unwrap_or(Found {
                attr: attr.clone(),
                pname: String::new(),
                version: String::new(),
                description: String::new(),
            });
            let app = looks.app(&found.pname, &found.version);
            let gone = snapshot
                .missing_packages
                .contains(&attr)
                .then(|| looks.named.instead.get(&attr).cloned());
            items.push(Item {
                name: app.as_ref().map_or(attr.clone(), |a| a.name().to_string()),
                attr: Some(attr),
                pname: found.pname,
                version: found.version,
                description: found.description,
                gone,
                source: Source::Settings(standing),
                launcher: app.is_some(),
                app,
            });
        }
    }

    for package in &snapshot.system_packages {
        if package.file == MANAGED_MODULE || package.file == "modules/options.nix" {
            continue;
        }
        let app = looks.app(&package.name, &package.version);
        let attr = file_attrs
            .by_pname
            .get(&(package.file.clone(), package.name.clone()))
            .cloned();
        items.push(Item {
            name: app
                .as_ref()
                .map_or(package.name.clone(), |a| a.name().to_string()),
            attr,
            pname: package.name.clone(),
            version: package.version.clone(),
            description: String::new(),
            source: Source::File(package.file.clone()),
            launcher: app.is_some(),
            app,
            gone: None,
        });
    }

    for flatpak in &flatpaks.apps {
        let desktop = format!("{}.desktop", flatpak.id);
        let app = looks
            .apps
            .list()
            .iter()
            .find(|app| app.id() == desktop)
            .cloned();
        items.push(Item {
            name: flatpak.name.clone(),
            attr: None,
            pname: flatpak.id.clone(),
            version: flatpak.version.clone(),
            description: String::new(),
            source: Source::Flatpak {
                id: flatpak.id.clone(),
                origin: flatpak.origin.clone(),
            },
            launcher: true,
            app,
            gone: None,
        });
    }

    items.sort_by_cached_key(Item::order);
    items
}

/// The second line: what it is and where it's from.
fn supporting(item: &Item) -> String {
    if let Some(instead) = &item.gone {
        let attr = item.attr.as_deref().unwrap_or(&item.pname);
        return match instead {
            Some(other) => {
                format!("{attr} · no longer in this nixpkgs, so not installed · try {other}")
            }
            None => format!("{attr} · no longer in this nixpkgs, so not installed"),
        };
    }
    let what = item.attr.as_deref().unwrap_or(&item.pname);
    let mut parts: Vec<String> = Vec::new();
    // an app's own name is the headline; say which package it is
    if !what.is_empty() && !what.eq_ignore_ascii_case(&item.name) {
        parts.push(what.to_string());
    }
    if !item.version.is_empty() {
        parts.push(item.version.clone());
    }
    parts.push(match &item.source {
        Source::Settings(Standing::Installed) => String::from("Added in Settings"),
        Source::Settings(Standing::Adding) => String::from("Installs on Apply"),
        Source::Settings(Standing::Removing) => String::from("Removed on Apply"),
        Source::File(file) => file.clone(),
        Source::Flatpak { origin, .. } => format!("Flatpak · {origin}"),
    });
    parts.join(" · ")
}

fn row(item: &Item, entry: Option<&Entry>, managed: &Managed, removal: &Removal) -> FormRow {
    let key = item.attr.clone().unwrap_or_else(|| item.pname.clone());
    let id = |what: &str| format!("settings.apps.installed.{what}.{key}");
    let mut row = FormRow::new(id("row"), item.name.clone())
        .leading(leading(item.app.as_ref()), icon_side())
        .subtitle(supporting(item));
    if let Some(attr) = item.attr.clone() {
        row = row.on_press(move || Details::open(&attr));
    }

    let button = match (&item.source, entry) {
        (Source::Settings(standing), Some(entry)) => {
            let (label, add) = match standing {
                Standing::Installed => ("Remove", false),
                Standing::Adding => ("Undo", false),
                Standing::Removing => ("Undo", true),
            };
            let (e, m, a) = (entry.clone(), managed.clone(), key.clone());
            Some(
                PushButton::new(id("change"), label)
                    .disabled(!entry.managed())
                    .on_press(move || change_packages(&e, &m, &a, add))
                    .build(),
            )
        }
        (Source::File(file), Some(entry)) => item.attr.as_ref().map(|attr| {
            let (e, file, attr) = (entry.clone(), file.clone(), attr.clone());
            PushButton::new(id("move"), "Move…")
                .disabled(!entry.managed())
                .on_press(move || adopt::confirm_package(&e, &file, &attr))
                .build()
        }),
        (Source::Flatpak { id: app, .. }, _) => {
            let busy = removal.is_busy(app);
            let (app, name) = (app.clone(), item.name.clone());
            Some(
                PushButton::new(
                    id("uninstall"),
                    if busy { "Removing…" } else { "Uninstall…" },
                )
                .disabled(removal.busy.is_some())
                .on_press(move || {
                    let app = app.clone();
                    Dialog::new(
                        "settings.apps.uninstall-dialog",
                        format!("Uninstall {name}?"),
                    )
                    .message("Its data in ~/.var/app stays until you delete it.")
                    .button("Cancel", || {})
                    .destructive("Uninstall", move || uninstall(app.clone()))
                    .open("main");
                })
                .build(),
            )
        }
        _ => None,
    };
    match button {
        Some(button) => row.trailing(button),
        None => row,
    }
}

pub fn section(
    entry: Option<&Entry>,
    managed: &Managed,
    feedback: &Feedback,
    snapshot: &Snapshot,
    looks: &Looks,
) -> Group {
    // the list's first poll is half a minute off
    static LOOKED: Once = Once::new();
    LOOKED.call_once(Flatpaks::refresh);
    let (flatpaks, removal) = (Flatpaks::read(), Removal::read());
    let file_attrs = FileAttrs::read();
    let show = Filter::read().show;
    let filter = TextInput::text(FILTER);
    let all = items(entry, managed, snapshot, &flatpaks, looks, &file_attrs);

    let shows: Vec<Show> = SHOWS
        .into_iter()
        .filter(|show| *show != Show::Flatpaks || flatpaks.available)
        .collect();
    let labels = shows
        .iter()
        .map(|show| {
            let count = all
                .iter()
                .filter(|item| item.shown_by(*show) && item.matches(&filter))
                .count();
            format!("{} ({count})", show.label())
        })
        .collect();
    let picked = shows.iter().position(|s| *s == show).unwrap_or(0);
    let chips = Dropdown::new("settings.apps.installed.show", labels, picked)
        .width(170.0)
        .on_select(move |index| {
            let shows: Vec<Show> = SHOWS
                .into_iter()
                .filter(|show| *show != Show::Flatpaks || Flatpaks::read().available)
                .collect();
            if let Some(show) = shows.get(index) {
                Filter::write().show = *show;
            }
        });
    let field = text_field(FILTER, "Filter…");

    let rows: Vec<FormRow> = all
        .iter()
        .filter(|item| item.shown_by(show) && item.matches(&filter))
        .map(|item| row(item, entry, managed, &removal))
        .collect();

    let filters = FormRow::new("settings.apps.installed.filters", "Show").trailing(
        amane::Row::new(vec![
            boxed(chips.build()),
            boxed(field_box(FILTER, field, 160.0)),
        ])
        .gap(8.0)
        .align(amane::Center),
    );
    let mut group = group().header("Installed").row(filters);
    if rows.is_empty() && snapshot.loading && all.is_empty() {
        group = group.row(
            FormRow::new(
                "settings.apps.installed.reading",
                "Reading what's installed…",
            )
            .loading(),
        );
    } else if rows.is_empty() {
        group = group.row(FormRow::new(
            "settings.apps.installed.none",
            if filter.trim().is_empty() {
                "Nothing of this kind installed"
            } else {
                "Nothing installed matches"
            },
        ));
    }
    group = group.rows(rows);

    let mut notes: Vec<String> = Vec::new();
    if let Some(entry) = entry {
        let (summary, _) = controls::summary(entry, managed, feedback);
        if !summary.trim().is_empty() {
            notes.push(format!("{}.", summary.trim_end_matches('.')));
        }
    }
    let removal_error = removal
        .error
        .as_ref()
        .map(|(app, why)| format!("{app}: {why}"));
    if let Some(error) = removal_error.or_else(|| flatpaks.error.clone()) {
        notes.push(error);
    }
    notes.push(String::from(
        "Press an app for its details. Move… hands an app listed in one of your own \
         configuration files over to Settings. Flatpaks are installed apart from the \
         configuration.",
    ));
    group.footer(notes.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_lists_names_on_lines_of_their_own() {
        let text = "{ pkgs, ... }:\n{\n  environment.systemPackages = (with pkgs; [\n    foot             # terminal\n    pkgs.ripgrep\n    # bat\n    github-cli\n  ]) ++ [\n    inputs.amane.packages.x.default\n  ];\n}\n";
        assert_eq!(
            listed_names(text),
            [
                "foot",
                "ripgrep",
                "github-cli",
                "inputs.amane.packages.x.default"
            ]
        );
    }

    fn item(name: &str, source: Source, app: bool) -> Item {
        Item {
            name: name.into(),
            attr: Some(name.into()),
            pname: name.into(),
            version: String::new(),
            description: String::from("a fine tool"),
            source,
            app: None,
            launcher: app,
            gone: None,
        }
    }

    #[test]
    fn kinds_and_filters() {
        let zed = item("Zed", Source::File("modules/apps.nix".into()), true);
        let rg = item("ripgrep", Source::Settings(Standing::Installed), false);
        let zen = item(
            "Zen",
            Source::Flatpak {
                id: "app.zen_browser.zen".into(),
                origin: "flathub".into(),
            },
            false,
        );
        assert!(zed.shown_by(Show::Apps) && !zed.shown_by(Show::Tools));
        assert!(rg.shown_by(Show::Tools) && !rg.shown_by(Show::Apps));
        assert!(zen.shown_by(Show::Apps) && zen.shown_by(Show::Flatpaks));
        assert!(rg.matches("RIP") && rg.matches("fine tool") && !rg.matches("zed"));
    }

    #[test]
    fn pending_changes_come_first() {
        let mut all = [
            item("bat", Source::File("modules/apps.nix".into()), false),
            item("zoxide", Source::Settings(Standing::Adding), false),
            item("Alacritty", Source::File("modules/apps.nix".into()), true),
        ];
        all.sort_by_cached_key(Item::order);
        let names: Vec<&str> = all.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["zoxide", "Alacritty", "bat"]);
    }

    #[test]
    fn the_second_line_says_where_from() {
        let mut zed = item("Zed", Source::File("modules/apps.nix".into()), true);
        zed.attr = Some("zed-editor".into());
        zed.version = "1.3.6".into();
        assert_eq!(supporting(&zed), "zed-editor · 1.3.6 · modules/apps.nix");
        let rg = item("ripgrep", Source::Settings(Standing::Adding), false);
        assert_eq!(supporting(&rg), "Installs on Apply");
        let mut gone = item(
            "youtube-music",
            Source::Settings(Standing::Installed),
            false,
        );
        gone.gone = Some(Some("pear-desktop".into()));
        assert_eq!(
            supporting(&gone),
            "youtube-music · no longer in this nixpkgs, so not installed · try pear-desktop"
        );
    }
}
