//! The settings the NixOS configuration has, as the flake's `amaneSettings`
//! output gives them (every setting in `managed/registry.nix` with its
//! effective value, how it's set and which files set it), and
//! `managed/settings.json`, the one file Settings writes:
//! `modules/managed.nix` turns it into option definitions, so the
//! hand-written modules are never touched.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::{MANAGED_MODULE, xkb};

/// How a setting is changed, from the registry's `control`.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    /// On or off.
    Switch,
    /// A whole number in a range.
    Int {
        /// The smallest it takes.
        min: i64,
        /// The largest it takes.
        max: i64,
    },
    /// One of a few named values, in a dropdown.
    Choice(Vec<Choice>),
    /// A zone name under `/etc/zoneinfo`, such as `Europe/Oslo`.
    TimeZone,
    /// Free text, checked by the entry's [`Check`] when it has one.
    Text,
    /// A list of nixpkgs attribute paths; the Apps page draws it.
    Packages,
}

/// One option of a [`Control::Choice`].
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// What the dropdown shows.
    pub label: String,
    /// What it writes.
    pub value: Value,
}

/// What a text setting is checked against, from the registry's `check`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// A host name: letters, digits and dashes, up to 63.
    Hostname,
    /// A layout `localectl list-x11-keymap-layouts` knows.
    XkbLayout,
    /// Empty, or a variant of the keyboard layout set.
    XkbVariant,
}

/// Whether `name` is a host name: lowercase letters, digits and dashes,
/// not at the ends, up to 63 of them.
pub fn is_hostname(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// What a package attribute path may look like: `ripgrep`, `python3Packages.black`.
pub fn is_attr_path(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
        })
}

/// When a change takes effect, from the registry's `tier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Once the system is applied.
    Rebuild,
    /// Only after the computer restarts.
    Restart,
}

impl Tier {
    /// The note under the setting that says when it takes effect.
    pub fn note(self) -> &'static str {
        match self {
            Self::Rebuild => "Takes effect after Apply",
            Self::Restart => "Needs a restart",
        }
    }
}

/// One setting from the registry, with what the configuration makes of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its key in `settings.json`.
    pub id: String,
    /// The option's path, like `services.openssh.enable`.
    pub path: Vec<String>,
    /// The Settings page it's on, such as `services`.
    pub page: String,
    /// Its name on the page.
    pub label: String,
    /// How it's changed.
    pub control: Control,
    /// When a change takes effect.
    pub tier: Tier,
    /// The value the configuration gives it now.
    pub value: Value,
    /// The option's default.
    pub default: Value,
    /// The text under it.
    pub description: String,
    /// The repo files that define the option; ours is `modules/managed.nix`.
    pub set_in: Vec<String>,
    /// Where another file defines it, as (file, line), when nix could tell.
    pub set_at: Vec<(String, u64)>,
    /// What a text value is checked against.
    pub check_kind: Option<Check>,
    /// The systemd unit behind a service, for its running state.
    pub unit: Option<String>,
    /// Whether that unit is a user unit (`systemctl --user`).
    pub user_scope: bool,
    /// The id of the service's "only when needed" switch.
    pub on_demand: Option<String>,
}

impl Entry {
    /// Whether changing `settings.json` changes it: nothing but
    /// `managed.nix` sets it. Another file's definition would win over
    /// ours, or clash with it.
    pub fn managed(&self) -> bool {
        self.set_in.iter().all(|file| file == MANAGED_MODULE)
    }

    /// The other files that set it.
    pub fn set_elsewhere(&self) -> Vec<&str> {
        self.set_in
            .iter()
            .filter(|file| *file != MANAGED_MODULE)
            .map(String::as_str)
            .collect()
    }

    /// Whether a value is one the control takes, before it's written, with
    /// the keyboard layout a variant is checked against (an empty layout
    /// lets any variant through).
    pub fn check_with(&self, value: &Value, layout: &str) -> Result<(), String> {
        match (&self.control, value) {
            (Control::Switch, Value::Bool(_)) => Ok(()),
            (Control::Int { min, max }, Value::Number(n)) => match n.as_i64() {
                Some(n) if (*min..=*max).contains(&n) => Ok(()),
                _ => Err(format!("{} takes {min} to {max}", self.label)),
            },
            (Control::TimeZone, Value::String(zone)) => {
                let name = zone.trim();
                let fine = !name.is_empty()
                    && !name.starts_with('/')
                    && !name.contains("..")
                    && Path::new("/etc/zoneinfo").join(name).is_file();
                if fine {
                    Ok(())
                } else {
                    Err(format!("No time zone named {name}"))
                }
            }
            (Control::Text, Value::String(text)) => {
                let text = text.trim();
                match self.check_kind {
                    None => Ok(()),
                    Some(Check::Hostname) if is_hostname(text) => Ok(()),
                    Some(Check::Hostname) => Err(String::from(
                        "A computer name is lowercase letters, digits and dashes, up to 63",
                    )),
                    Some(Check::XkbLayout) if xkb::layouts().iter().any(|l| l == text) => Ok(()),
                    Some(Check::XkbLayout) => Err(format!(
                        "No keyboard layout named {text} (see localectl list-x11-keymap-layouts)"
                    )),
                    Some(Check::XkbVariant) if text.is_empty() || layout.is_empty() => Ok(()),
                    Some(Check::XkbVariant) => {
                        let variants = xkb::variants(layout);
                        if variants.iter().any(|v| v == text) {
                            Ok(())
                        } else {
                            Err(format!(
                                "No variant {text} of {layout}; it has {}",
                                xkb::some(&variants)
                            ))
                        }
                    }
                }
            }
            (Control::Choice(choices), value) if choices.iter().any(|c| c.value == *value) => {
                Ok(())
            }
            (Control::Packages, Value::Array(names)) => {
                let mut seen = Vec::new();
                for name in names {
                    let Some(name) = name.as_str() else {
                        return Err(String::from("A package is named by its attribute path"));
                    };
                    if !is_attr_path(name) {
                        return Err(format!("{name} isn't an attribute path"));
                    }
                    if seen.contains(&name) {
                        return Err(format!("{name} is listed twice"));
                    }
                    seen.push(name);
                }
                Ok(())
            }
            _ => Err(format!("{} can't be set to {}", self.label, show(value))),
        }
    }
}

/// A value as a label says it.
pub fn show(value: &Value) -> String {
    match value {
        Value::Null => String::from("unset"),
        Value::Bool(true) => String::from("on"),
        Value::Bool(false) => String::from("off"),
        Value::String(text) => text.clone(),
        Value::Number(n) => n.to_string(),
        Value::Array(items) if items.is_empty() => String::from("nothing"),
        Value::Array(items) => items.iter().map(show).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

/// A choice's label for a value, else the value itself.
pub fn show_as(control: &Control, value: &Value) -> String {
    if let Control::Choice(choices) = control
        && let Some(choice) = choices.iter().find(|c| c.value == *value)
    {
        return choice.label.clone();
    }
    show(value)
}

/// The strings in a JSON array, skipping anything else; none when it isn't one.
pub fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn entry(value: &Value) -> Option<Entry> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);
    let control = match text("control")?.as_str() {
        "switch" => Control::Switch,
        "int" => Control::Int {
            min: value.get("min").and_then(Value::as_i64).unwrap_or(0),
            max: value.get("max").and_then(Value::as_i64).unwrap_or(i64::MAX),
        },
        "timezone" => Control::TimeZone,
        "packages" => Control::Packages,
        "choice" => Control::Choice(
            value
                .get("choices")
                .and_then(Value::as_array)
                .map(|choices| {
                    choices
                        .iter()
                        .filter_map(|choice| {
                            Some(Choice {
                                label: choice.get("label")?.as_str()?.to_string(),
                                value: choice.get("value")?.clone(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        ),
        _ => Control::Text,
    };
    let tier = match text("tier").as_deref() {
        Some("restart") => Tier::Restart,
        _ => Tier::Rebuild,
    };

    Some(Entry {
        id: text("id")?,
        path: value
            .get("path")
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        page: text("page").unwrap_or_default(),
        label: text("label").unwrap_or_else(|| text("id").unwrap_or_default()),
        control,
        tier,
        value: value.get("value").cloned().unwrap_or(Value::Null),
        default: value.get("default").cloned().unwrap_or(Value::Null),
        description: text("description").unwrap_or_default(),
        set_in: value
            .get("setIn")
            .and_then(Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        set_at: value
            .get("setAt")
            .and_then(Value::as_array)
            .map(|places| {
                places
                    .iter()
                    .filter_map(|place| {
                        Some((
                            place.get("file")?.as_str()?.to_string(),
                            place.get("line")?.as_u64()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        check_kind: match text("check").as_deref() {
            Some("hostname") => Some(Check::Hostname),
            Some("xkbLayout") => Some(Check::XkbLayout),
            Some("xkbVariant") => Some(Check::XkbVariant),
            _ => None,
        },
        unit: text("unit"),
        user_scope: text("scope").as_deref() == Some("user"),
        on_demand: text("onDemand"),
    })
}

/// A package a repo file installs system-wide.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemPackage {
    /// The package's name.
    pub name: String,
    /// Its version, empty when nix didn't give one.
    pub version: String,
    /// The repo file that lists it.
    pub file: String,
}

fn system_package(value: &Value) -> Option<SystemPackage> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);
    Some(SystemPackage {
        name: text("name")?,
        version: text("version").unwrap_or_default(),
        file: text("file").unwrap_or_default(),
    })
}

/// What `nix eval --json .#amaneSettings` gave.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parsed {
    /// Every setting in the registry.
    pub entries: Vec<Entry>,
    /// The commit the repo was at, `-dirty` when it had changes.
    pub revision: Option<String>,
    /// The `nixosConfigurations` attribute it was evaluated as.
    pub host: Option<String>,
    /// What the repo's files install system-wide.
    pub system_packages: Vec<SystemPackage>,
    /// The packages added in Settings this nixpkgs doesn't have: the build
    /// leaves them out, with a warning.
    pub missing_packages: Vec<String>,
}

/// Reads `nix eval --json .#amaneSettings`; settings that don't parse are
/// left out, and it fails only when the JSON or its `entries` list is bad.
pub fn parse_snapshot(json: &str) -> Result<Parsed, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| format!("amaneSettings: {error}"))?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("amaneSettings has no entries")?
        .iter()
        .filter_map(entry)
        .collect();

    Ok(Parsed {
        entries,
        revision: value
            .get("revision")
            .and_then(Value::as_str)
            .map(String::from),
        host: value.get("host").and_then(Value::as_str).map(String::from),
        system_packages: value
            .get("systemPackages")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(system_package).collect())
            .unwrap_or_default(),
        missing_packages: strings(value.get("missingPackages")),
    })
}

impl Parsed {
    /// The setting with this id.
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

/// The setting with this id, among `entries`.
pub fn entry_in<'a>(entries: &'a [Entry], id: &str) -> Option<&'a Entry> {
    entries.iter().find(|entry| entry.id == id)
}

// ───── managed/settings.json ─────────────────────────────────────────────────

/// A setting that differs between the working tree and HEAD: what Apply
/// would make permanent.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// The setting's id.
    pub id: String,
    /// Its value at HEAD; none when HEAD doesn't set it.
    pub from: Option<Value>,
    /// Its value in the working tree; none when it's been removed.
    pub to: Option<Value>,
}

/// Reads `settings.json` as id to value; `null` is an empty map, anything
/// but an object an error.
pub fn parse_map(json: &str) -> Result<BTreeMap<String, Value>, String> {
    let value: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    match value {
        Value::Object(map) => Ok(map.into_iter().collect()),
        Value::Null => Ok(BTreeMap::new()),
        _ => Err(String::from("not an object")),
    }
}

/// What differs between the file (`current`) and HEAD's copy (`committed`).
pub fn changes(
    current: &BTreeMap<String, Value>,
    committed: &BTreeMap<String, Value>,
) -> Vec<Change> {
    let mut ids: Vec<&String> = current.keys().chain(committed.keys()).collect();
    ids.sort();
    ids.dedup();

    ids.into_iter()
        .filter(|id| current.get(*id) != committed.get(*id))
        .map(|id| Change {
            id: id.clone(),
            from: committed.get(id).cloned(),
            to: current.get(id).cloned(),
        })
        .collect()
}

/// Each change as a few words, with the registry's labels where `entries`
/// have them: "Time zone → Europe/Oslo", "Tailscale off".
pub fn summary(changes: &[Change], entries: &[Entry]) -> Vec<String> {
    changes
        .iter()
        .map(|change| {
            let entry = entry_in(entries, &change.id);
            let label = entry.map_or(change.id.clone(), |entry| entry.label.clone());
            match &change.to {
                Some(Value::Bool(on)) => format!("{label} {}", if *on { "on" } else { "off" }),
                // a list: what came and went, "Added in Settings + ripgrep, − htop"
                Some(Value::Array(_)) => {
                    let (from, to) = (strings(change.from.as_ref()), strings(change.to.as_ref()));
                    let added = to
                        .iter()
                        .filter(|n| !from.contains(n))
                        .map(|n| format!("+ {n}"));
                    let gone = from
                        .iter()
                        .filter(|n| !to.contains(n))
                        .map(|n| format!("− {n}"));
                    format!(
                        "{label} {}",
                        added.chain(gone).collect::<Vec<_>>().join(", ")
                    )
                }
                Some(value) => format!(
                    "{label} → {}",
                    entry.map_or_else(|| show(value), |e| show_as(&e.control, value))
                ),
                None => format!("{label} unset"),
            }
        })
        .collect()
}

/// The commit's subject for the changes.
pub fn commit_message(changes: &[Change], entries: &[Entry]) -> String {
    format!("settings: {}", summary(changes, entries).join(", "))
}

/// Writes the map as the file, whole.
pub fn save(path: &Path, map: &BTreeMap<String, Value>) -> Result<(), String> {
    amane_base::fs::save_json(path, map).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = r#"{"entries":[
        {"control":"timezone","default":null,"description":"","id":"timeZone","label":"Time zone",
         "page":"about","path":["time","timeZone"],"setIn":["modules/managed.nix"],"tier":"rebuild",
         "type":"null or string","value":"Europe/Oslo"},
        {"control":"switch","default":false,"id":"tailscale","label":"Tailscale","page":"about",
         "path":["services","tailscale","enable"],"setIn":["modules/services.nix"],"tier":"rebuild",
         "type":"boolean","value":true},
        {"control":"int","default":null,"id":"bootEntries","label":"Boot entries kept","max":50,"min":1,
         "page":"storage","path":["boot","loader","systemd-boot","configurationLimit"],
         "setIn":["modules/managed.nix"],"tier":"restart","type":"null or signed integer","value":10},
        {"control":"switch","default":false,"id":"openssh","label":"SSH server","page":"services",
         "path":["services","openssh","enable"],"setIn":["modules/managed.nix"],"tier":"rebuild",
         "type":"boolean","value":true,"unit":"sshd.service","onDemand":"sshdOnDemand"},
        {"control":"switch","default":false,"id":"sunshine","label":"Sunshine","page":"services",
         "path":["services","sunshine","enable"],"setIn":[],"tier":"rebuild",
         "type":"boolean","value":false,"unit":"sunshine.service","scope":"user"},
        {"control":"choice","choices":[{"label":"1 minute","value":60},{"label":"Never","value":0}],
         "default":180,"id":"dimAfter","label":"Dim the screen after","page":"power",
         "path":["amane","idle","dimAfter"],"setIn":[],"tier":"rebuild","type":"unsigned integer","value":180},
        {"control":"packages","default":[],"id":"packages","label":"Added in Settings","page":"apps",
         "path":["amane","packages"],"setIn":[],"tier":"rebuild","type":"list of string","value":[]}
    ],"host":"nixos-niri","revision":"5d056b8-dirty",
    "systemPackages":[{"name":"foot","version":"1.2","file":"modules/apps.nix"}],
    "missingPackages":["youtube-music"]}"#;

    fn snapshot() -> Parsed {
        parse_snapshot(SNAPSHOT).unwrap()
    }

    #[test]
    fn the_snapshot_says_what_each_setting_is_and_who_sets_it() {
        let snapshot = snapshot();
        assert_eq!(snapshot.entries.len(), 7);
        assert_eq!(snapshot.revision.as_deref(), Some("5d056b8-dirty"));

        let zone = snapshot.entry("timeZone").unwrap();
        assert_eq!(zone.control, Control::TimeZone);
        assert!(zone.managed());

        let tailscale = snapshot.entry("tailscale").unwrap();
        assert!(!tailscale.managed());
        assert_eq!(tailscale.set_elsewhere(), vec!["modules/services.nix"]);

        let boot = snapshot.entry("bootEntries").unwrap();
        assert_eq!(boot.control, Control::Int { min: 1, max: 50 });
        assert_eq!(boot.tier, Tier::Restart);
        assert_eq!(
            snapshot
                .entries
                .iter()
                .filter(|e| e.page == "storage")
                .count(),
            1
        );

        let ssh = snapshot.entry("openssh").unwrap();
        assert_eq!(ssh.unit.as_deref(), Some("sshd.service"));
        assert_eq!(ssh.on_demand.as_deref(), Some("sshdOnDemand"));
        assert!(!ssh.user_scope);
        let sunshine = snapshot.entry("sunshine").unwrap();
        assert!(sunshine.user_scope);
        // nothing sets it: ours to set
        assert!(sunshine.managed());

        let dim = snapshot.entry("dimAfter").unwrap();
        assert!(matches!(&dim.control, Control::Choice(choices) if choices.len() == 2));
        assert_eq!(show_as(&dim.control, &Value::from(0)), "Never");
        assert_eq!(show_as(&dim.control, &Value::from(7)), "7");
        assert_eq!(snapshot.system_packages[0].name, "foot");
        assert_eq!(snapshot.missing_packages, ["youtube-music"]);
    }

    #[test]
    fn a_value_is_checked_against_its_control() {
        let snapshot = snapshot();
        let boot = snapshot.entry("bootEntries").unwrap();
        assert!(boot.check_with(&Value::from(8), "").is_ok());
        assert!(boot.check_with(&Value::from(0), "").is_err());
        assert!(boot.check_with(&Value::from("ten"), "").is_err());

        let tailscale = snapshot.entry("tailscale").unwrap();
        assert!(tailscale.check_with(&Value::Bool(false), "").is_ok());
        assert!(tailscale.check_with(&Value::from(1), "").is_err());

        let zone = snapshot.entry("timeZone").unwrap();
        assert!(zone.check_with(&Value::from("../etc/passwd"), "").is_err());
        assert!(zone.check_with(&Value::from(""), "").is_err());

        let dim = snapshot.entry("dimAfter").unwrap();
        assert!(dim.check_with(&Value::from(60), "").is_ok());
        assert!(dim.check_with(&Value::from(61), "").is_err());

        let packages = snapshot.entry("packages").unwrap();
        assert!(
            packages
                .check_with(&serde_json::json!(["ripgrep", "python3Packages.black"]), "")
                .is_ok()
        );
        assert!(
            packages
                .check_with(&serde_json::json!(["ripgrep", "ripgrep"]), "")
                .is_err()
        );
        assert!(
            packages
                .check_with(&serde_json::json!(["rip grep"]), "")
                .is_err()
        );
        assert!(packages.check_with(&serde_json::json!([1]), "").is_err());
        assert!(packages.check_with(&Value::from("ripgrep"), "").is_err());
    }

    #[test]
    fn a_list_change_says_what_came_and_went() {
        let current = parse_map(r#"{"packages":["ripgrep","fd"],"dimAfter":0}"#).unwrap();
        let committed = parse_map(r#"{"packages":["htop","fd"],"dimAfter":180}"#).unwrap();
        assert_eq!(
            commit_message(&changes(&current, &committed), &snapshot().entries),
            "settings: Dim the screen after → Never, Added in Settings + ripgrep, − htop"
        );
        assert_eq!(strings(current.get("packages")), vec!["ripgrep", "fd"]);
    }

    #[test]
    fn the_changes_are_what_differs_from_head_and_read_as_words() {
        let current =
            parse_map(r#"{"timeZone":"Europe/Berlin","tailscale":false,"bootEntries":10}"#)
                .unwrap();
        let committed =
            parse_map(r#"{"timeZone":"Europe/Oslo","tailscale":true,"bootEntries":10}"#).unwrap();
        let found = changes(&current, &committed);
        assert_eq!(found.len(), 2);
        assert_eq!(
            commit_message(&found, &snapshot().entries),
            "settings: Tailscale off, Time zone → Europe/Berlin"
        );
        assert!(changes(&current, &current).is_empty());
    }

    #[test]
    fn a_missing_file_is_empty_and_a_broken_one_says_so() {
        assert!(parse_map("null").unwrap().is_empty());
        assert!(parse_map("[1]").is_err());
        assert!(parse_map("{").is_err());
    }
}
