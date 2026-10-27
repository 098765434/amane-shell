//! One package's details, in the page's place: what it is, its licence and
//! homepage, the command it installs, where it stands, and why it can't be
//! added when it can't. Read with `nix eval` from the configuration's own
//! `pkgs` (`nixosConfigurations.<host>.pkgs.<attr>`), so its nixpkgs config
//! counts: an unfree package is available when the configuration allows
//! unfree ones. The same read checks a package before **Add** writes it.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread;

use amane::{Apps, Center, Color, Column, Rectangle, Row, Service, Size, Text, Widget};
use amane_kit::app_icon::entry_tile;
use amane_kit::buttons::{PushButton, Style};
use amane_kit::icons::{ICON_PACKAGE, ICON_TERMINAL};
use amane_kit::layout::{boxed, icon};
use amane_kit::list::{List, ListItem, trailing_text, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::tokens::{Styled, sys};
use serde::Deserialize;

use super::apply_bar::Overlay;
use super::apps::{PACKAGES, Standing, change_packages, present, standing};
use super::catalog::{Catalog, Named};
use super::installed::Launchers;
use super::nixos::{Managed, Snapshot, failure, host, repo};
use crate::{Navigation, PAGE_WIDTH, note};

/// What `--apply` makes of the package: plain JSON whatever the meta's shape.
const APPLY: &str = r#"p: let
  m = p.meta or { };
  l = m.license or [ ];
  ls = if builtins.isList l then l else [ l ];
  h = m.homepage or "";
in {
  pname = p.pname or (builtins.parseDrvName (p.name or "")).name;
  version = p.version or "";
  description = m.description or "";
  longDescription = m.longDescription or "";
  homepage = if builtins.isList h then (if h == [ ] then "" else builtins.head h) else h;
  licenses = map (x: if builtins.isAttrs x then x.fullName or x.spdxId or "" else toString x) ls;
  free = builtins.all (x: !(builtins.isAttrs x) || (x.free or true)) ls;
  mainProgram = m.mainProgram or "";
  broken = m.broken or false;
  vulnerabilities = m.knownVulnerabilities or [ ];
  available = m.available or true;
}"#;

/// A package's `meta`, as [`APPLY`] gives it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Meta {
    pub pname: String,
    pub version: String,
    pub description: String,
    pub long_description: String,
    pub homepage: String,
    pub licenses: Vec<String>,
    pub free: bool,
    pub main_program: String,
    pub broken: bool,
    pub vulnerabilities: Vec<String>,
    pub available: bool,
}

impl Meta {
    /// Why the build would refuse it, if it would.
    pub fn problem(&self) -> Option<String> {
        if self.broken {
            Some(String::from(
                "Marked broken in this nixpkgs: it doesn't build here.",
            ))
        } else if !self.vulnerabilities.is_empty() {
            Some(format!(
                "Has known vulnerabilities, so nixpkgs won't build it: {}",
                self.vulnerabilities.join(" ")
            ))
        } else if !self.available {
            Some(String::from(
                "Not available here: it doesn't support this system, or its licence isn't one \
                 the configuration allows.",
            ))
        } else {
            None
        }
    }
}

/// `pkgs.<attr>` as an attribute path nix reads, each part quoted when it
/// has more than letters, digits, `_`, `-` and `'` in it.
fn attr_path(attr: &str) -> String {
    attr.split('.')
        .map(|part| {
            let plain = !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '\''));
            if plain {
                part.to_string()
            } else {
                format!("\"{}\"", part.replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

// read once per nixpkgs revision and package
static KNOWN: Mutex<Option<HashMap<(String, String), Meta>>> = Mutex::new(None);

/// The package's meta when it's been read already (its details, or the
/// check before **Add**), without reading it.
pub fn known(attr: &str) -> Option<Meta> {
    let rev = Catalog::read().rev.clone().unwrap_or_default();
    KNOWN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()?
        .get(&(rev, attr.to_string()))
        .cloned()
}

/// What an Apply installed, in a few words: the apps by their names, a
/// command-line tool with its command (*Zed, ripgrep (rg)*); and the app to
/// offer to open when there's exactly one.
pub fn arrived(attrs: &[String]) -> (String, Option<amane::DesktopApp>) {
    let (launchers, apps, named) = (Launchers::read(), Apps::read(), Named::read());
    let mut names = Vec::new();
    let mut opened = Vec::new();
    for attr in attrs {
        let found = named.found.get(attr);
        let meta = known(attr);
        let (pname, version) = match (found, &meta) {
            (Some(f), _) => (f.pname.clone(), f.version.clone()),
            (None, Some(m)) => (m.pname.clone(), m.version.clone()),
            (None, None) => (String::new(), String::new()),
        };
        match launchers.app(&apps, &pname, &version) {
            Some(app) => {
                names.push(app.name().to_string());
                opened.push(app);
            }
            None => match meta.filter(|m| !m.main_program.is_empty()) {
                Some(meta) => names.push(format!("{attr} ({})", meta.main_program)),
                None => names.push(attr.clone()),
            },
        }
    }
    let one = (opened.len() == 1).then(|| opened.remove(0));
    (names.join(", "), one)
}

/// The package's meta, from the configuration's `pkgs` (half a second).
pub fn meta(attr: &str) -> Result<Meta, String> {
    let rev = Catalog::read().rev.clone().unwrap_or_default();
    let key = (rev, attr.to_string());
    if let Some(meta) = KNOWN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_default()
        .get(&key)
    {
        return Ok(meta.clone());
    }

    let installable = format!(
        ".#nixosConfigurations.{}.pkgs.{}",
        attr_path(&host()),
        attr_path(attr)
    );
    let output = Command::new("nix")
        .args([
            "eval",
            "--json",
            "--no-warn-dirty",
            &installable,
            "--apply",
            APPLY,
        ])
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("nix: {error}"))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stderr);
        if text.contains("does not provide attribute") {
            return Err(format!("{attr} isn't in this nixpkgs."));
        }
        return Err(failure(
            &output.stderr,
            &output.stdout,
            output.status.code(),
        ));
    }
    let meta: Meta = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    KNOWN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_default()
        .insert(key, meta.clone());
    Ok(meta)
}

/// The package shown, and its meta once read; `checking` is a package
/// **Add** is reading before it writes it.
#[derive(Default)]
pub struct Details {
    pub attr: String,
    pub meta: Option<Result<Meta, String>>,
    pub checking: Option<String>,
}

impl Service for Details {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

impl Details {
    /// Shows `attr`'s details in the page's place.
    pub fn open(attr: &str) {
        {
            let mut now = Self::write();
            now.attr = attr.to_string();
            now.meta = None;
        }
        Navigation::write().overlay = Some(Overlay::Package);
        let attr = attr.to_string();
        thread::spawn(move || {
            let result = meta(&attr);
            let mut now = Self::write();
            if now.attr == attr {
                now.meta = Some(result);
            } else {
                now.quiet();
            }
        });
    }

    /// Reads the package, then adds it with `add`, or shows its details when
    /// the build would refuse it (or it couldn't be read), so they say why.
    pub fn check_then_add(attr: &str, add: impl FnOnce() + Send + 'static) {
        {
            let mut now = Self::write();
            if now.checking.is_some() {
                now.quiet();
                return;
            }
            now.checking = Some(attr.to_string());
        }
        let attr = attr.to_string();
        thread::spawn(move || {
            let result = meta(&attr);
            Self::write().checking = None;
            match &result {
                Ok(meta) if meta.problem().is_none() => add(),
                _ => {
                    {
                        let mut now = Self::write();
                        now.attr = attr;
                        now.meta = Some(result);
                    }
                    Navigation::write().overlay = Some(Overlay::Package);
                }
            }
        });
    }
}

/// The view's title: the package's attribute path.
pub fn title() -> String {
    Details::read().attr.clone()
}

/// Wrapped text as tall as its lines at the page's width: wrapped text
/// alone would take the rest of the column.
fn paragraph(text: Text) -> Rectangle {
    let text = text.wrap();
    let height = text.height_in(PAGE_WIDTH);
    Rectangle::new()
        .width(Size::Fixed(PAGE_WIDTH))
        .height(Size::Fixed(height))
        .fill(Color::TRANSPARENT)
        .child(text)
}

fn fact(id: &str, name: &str, value: impl Widget + 'static) -> ListItem {
    ListItem::new(format!("settings.package.{id}"), name)
        .bare(true)
        .trailing(value)
}

fn value(text: impl Into<String>) -> Text {
    trailing_text(text, material::label(Level::Primary))
}

pub fn view() -> Column {
    let details = Details::read();
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let launchers = Launchers::read();
    let apps = Apps::read();
    let attr = details.attr.clone();

    let meta = match &details.meta {
        None => {
            return Column::new(vec![boxed(note("Reading the package…")), back()])
                .gap(sys::space::gap::l());
        }
        Some(Err(error)) => {
            return Column::new(vec![
                boxed(paragraph(
                    note(error.clone()).color(sys::color::status::critical::text()),
                )),
                back(),
            ])
            .gap(sys::space::gap::l());
        }
        Some(Ok(meta)) => meta,
    };

    let app = launchers.app(&apps, &meta.pname, &meta.version);
    let side = 48.0;
    let tile: Box<dyn Widget> = match &app {
        Some(app) => boxed(entry_tile(app, side)),
        None => boxed(icon(
            if meta.main_program.is_empty() {
                ICON_PACKAGE
            } else {
                ICON_TERMINAL
            },
            side * 0.75,
            material::label(Level::Secondary),
        )),
    };
    let name = app
        .as_ref()
        .map_or(meta.pname.clone(), |app| app.name().to_string());
    let mut identity = attr.clone();
    if !meta.version.is_empty() {
        identity.push_str(&format!(" · {}", meta.version));
    }
    let header = Row::new(vec![
        tile,
        boxed(Column::new(vec![
            boxed(
                Text::new(name)
                    .style(sys::r#type::title())
                    .color(material::label(Level::Primary)),
            ),
            boxed(note(identity)),
        ])),
    ])
    .gap(sys::space::gap::m())
    .align(Center);

    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(header)];
    if !meta.description.is_empty() {
        parts.push(boxed(paragraph(
            Text::new(meta.description.clone())
                .style(sys::r#type::label())
                .color(material::label(Level::Primary)),
        )));
    }
    if !meta.long_description.is_empty() {
        // nixpkgs wraps its long descriptions at 80 columns
        let text = meta
            .long_description
            .split("\n\n")
            .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n\n");
        parts.push(boxed(paragraph(note(text))));
    }
    let problem = meta.problem();
    if let Some(problem) = &problem {
        parts.push(boxed(paragraph(
            note(problem.clone()).color(sys::color::status::critical::text()),
        )));
    }

    // where it stands
    let entry = snapshot.entry(PACKAGES);
    let current = entry
        .map(|e| managed.list(&e.id, &e.value))
        .unwrap_or_default();
    let added = standing(&attr, &managed, entry);
    let installed: Vec<&str> = snapshot
        .system_packages
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    let from_file = (added.is_none() && present(&attr, &meta.pname, &current, &installed))
        .then(|| {
            snapshot
                .system_packages
                .iter()
                .find(|p| p.name == meta.pname)
                .map(|p| p.file.clone())
        })
        .flatten();
    let status = match (added, &from_file) {
        (Some(Standing::Installed), _) => String::from("Installed"),
        (Some(Standing::Adding), _) => String::from("Installs on Apply"),
        (Some(Standing::Removing), _) => String::from("Removed on Apply"),
        (None, Some(file)) => format!("Installed by {file}"),
        (None, None) => String::from("Not installed"),
    };

    let mut facts = vec![fact("status", "Status", value(status))];
    if !meta.licenses.is_empty() {
        let mut licence = meta.licenses.join(", ");
        // nixpkgs' own unfree licence already says so
        if !meta.free && !licence.to_lowercase().contains("unfree") {
            licence.push_str(" · Unfree");
        }
        facts.push(fact("licence", "Licence", value(licence)));
    }
    if !meta.main_program.is_empty() {
        facts.push(fact("command", "Command", value(meta.main_program.clone())));
    }
    if !meta.homepage.is_empty() {
        let url = meta.homepage.clone();
        facts.push(
            fact(
                "homepage",
                "Homepage",
                PushButton::new("settings.package.homepage.open", "Open")
                    .style(Style::Plain)
                    .on_press(move || {
                        let _ = Command::new("xdg-open")
                            .arg(&url)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn();
                    })
                    .build(),
            )
            .supporting(meta.homepage.clone()),
        );
    }
    parts.push(boxed(with_row_width(PAGE_WIDTH, || {
        List::new(facts).build()
    })));

    // what can be done
    let mut buttons: Vec<Box<dyn Widget>> = vec![back()];
    if let Some(entry) = entry {
        let (label, add) = match added {
            Some(Standing::Installed) => (Some("Remove"), false),
            Some(Standing::Adding) => (Some("Undo Add"), false),
            Some(Standing::Removing) => (Some("Undo Remove"), true),
            None if from_file.is_none() => (Some("Add"), true),
            None => (None, false),
        };
        if let Some(label) = label {
            let (e, m, a) = (entry.clone(), managed.clone(), attr.clone());
            buttons.push(boxed(
                PushButton::new("settings.package.change", label)
                    .style(if add { Style::Filled } else { Style::Tonal })
                    .disabled(!entry.managed() || (add && problem.is_some()))
                    .on_press(move || change_packages(&e, &m, &a, add))
                    .build(),
            ));
        }
    }
    if let Some(app) = app {
        buttons.push(boxed(
            PushButton::new("settings.package.open", "Open App")
                .style(Style::Tonal)
                .on_press(move || app.launch())
                .build(),
        ));
    }
    parts.push(boxed(
        Row::new(buttons).gap(sys::space::gap::s()).align(Center),
    ));
    if from_file.is_some() && added.is_none() {
        parts.push(boxed(note(
            "Part of your configuration: edit that file to remove it, then Apply.",
        )));
    } else if !matches!(added, None | Some(Standing::Installed)) {
        parts.push(boxed(note("Takes effect after Apply.")));
    }

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}

fn back() -> Box<dyn Widget> {
    boxed(
        PushButton::new("settings.package.back", "Back")
            .style(Style::Plain)
            .on_press(|| Navigation::write().overlay = None)
            .build(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_parts_are_quoted_when_they_must_be() {
        assert_eq!(attr_path("zed-editor"), "zed-editor");
        assert_eq!(
            attr_path("python313Packages.requests"),
            "python313Packages.requests"
        );
        assert_eq!(attr_path("nixos-niri"), "nixos-niri");
        assert_eq!(attr_path("a.b+c"), "a.\"b+c\"");
    }

    #[test]
    fn the_build_refusing_it_is_a_problem() {
        let fine = Meta {
            available: true,
            free: true,
            ..Meta::default()
        };
        assert_eq!(fine.problem(), None);
        let broken = Meta {
            broken: true,
            available: false,
            ..fine.clone()
        };
        assert!(broken.problem().unwrap().contains("broken"));
        let insecure = Meta {
            vulnerabilities: vec!["CVE-1".into()],
            available: false,
            ..fine.clone()
        };
        assert!(insecure.problem().unwrap().contains("CVE-1"));
        let elsewhere = Meta {
            available: false,
            ..fine
        };
        assert!(elsewhere.problem().unwrap().contains("Not available"));
    }

    #[test]
    fn meta_json_reads_with_defaults() {
        let meta: Meta = serde_json::from_str(
            r#"{"available":true,"broken":false,"description":"Code editor","free":false,
                "homepage":"https://code.visualstudio.com/","licenses":["Unfree"],
                "longDescription":"","mainProgram":"code","pname":"vscode","version":"1.119.0",
                "vulnerabilities":[]}"#,
        )
        .unwrap();
        assert_eq!(meta.main_program, "code");
        assert!(!meta.free && meta.available);
        assert_eq!(meta.problem(), None);
    }
}
