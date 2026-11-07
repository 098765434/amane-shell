//! Services: a switch per service the registry lists, with whether its unit
//! is running (from `systemctl show`, every few seconds), "only when
//! needed" for the ones that are installed but not started at boot, and
//! Start/Stop for right now (systemd asks through polkit).

use super::busy::Busy;
use amane_kit::color::Accent;
use amane_kit::service::ServiceExt as _;
use std::collections::BTreeMap;
use std::thread;
use std::time::Duration;

use amane::{Column, Row, Service};
use amane_kit::app::Dialog;
use amane_kit::buttons::PushButton;
use amane_kit::form::FormRow;
use amane_kit::layout::{boxed, frame};
use amane_kit::style;

use super::controls::{self, Feedback};
use super::job::BuildJob;
use super::nixos::{Managed, Snapshot, run};
use crate::{group, groups};

/// A unit as systemd reports it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitState {
    pub active: String,
    pub sub: String,
    pub memory: Option<u64>,
    /// systemd knows the unit: a service switched on but not yet applied
    /// has none to start.
    pub loaded: bool,
}

/// The services' units, system and user, as of the last look.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Units {
    pub system: BTreeMap<String, UnitState>,
    pub user: BTreeMap<String, UnitState>,
    pub looked: bool,
}

/// The units to ask about: the registry's services' (by scope), and
/// hypridle's for Power & Lock.
fn names(user: bool) -> Vec<String> {
    let snapshot = Snapshot::read();
    let mut names: Vec<String> = snapshot
        .for_page("services")
        .into_iter()
        .filter(|entry| entry.user_scope == user)
        .filter_map(|entry| entry.unit.clone())
        .collect();
    if user {
        names.push(String::from("hypridle.service"));
    }
    names
}

fn show(user: bool, units: &[String]) -> BTreeMap<String, UnitState> {
    if units.is_empty() {
        return BTreeMap::new();
    }
    let mut args: Vec<&str> = Vec::new();
    if user {
        args.push("--user");
    }
    args.extend([
        "show",
        "-p",
        "Id,LoadState,ActiveState,SubState,MemoryCurrent",
    ]);
    args.extend(units.iter().map(String::as_str));
    run("systemctl", &args, None)
        .map(|text| parse_show(&text))
        .unwrap_or_default()
}

/// `systemctl show` for several units: a block of `Key=value` lines per
/// unit, blank lines between.
pub fn parse_show(text: &str) -> BTreeMap<String, UnitState> {
    text.split("\n\n")
        .filter_map(|block| {
            let mut id = None;
            let mut state = UnitState::default();
            for line in block.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                match key {
                    "Id" => id = Some(value.to_string()),
                    "ActiveState" => state.active = value.to_string(),
                    "SubState" => state.sub = value.to_string(),
                    "LoadState" => state.loaded = value == "loaded",
                    "MemoryCurrent" => state.memory = value.parse().ok(),
                    _ => {}
                }
            }
            Some((id?, state))
        })
        .collect()
}

fn look() -> Units {
    Units {
        system: show(false, &names(false)),
        user: show(true, &names(true)),
        looked: true,
    }
}

impl Service for Units {
    fn new() -> Self {
        Self::default()
    }

    fn interval() -> Duration {
        Duration::from_secs(5)
    }

    fn update(&mut self) -> bool {
        let fresh = look();
        let changed = fresh != *self;
        *self = fresh;
        changed
    }
}

impl Units {
    /// Looks again now, after a Start or Stop.
    pub fn refresh() {
        thread::spawn(|| {
            Self::replace(look());
        });
    }

    pub fn get(&self, unit: &str, user: bool) -> Option<&UnitState> {
        if user { &self.user } else { &self.system }.get(unit)
    }
}

/// How wide a service's status dot is, with the room around it.
const DOT: f32 = 10.0;

/// Green while running, red when it failed, grey otherwise.
fn status_dot(state: Option<&UnitState>) -> amane::Rectangle {
    use amane::{Rectangle, Size};
    let color = match state.map(|state| state.active.as_str()) {
        Some("active") => Accent::Green.color(),
        Some("activating" | "deactivating") => Accent::Yellow.color(),
        Some("failed") => Accent::Red.color(),
        _ => style::label_tertiary(),
    };
    frame(DOT, DOT)
        .align_child(amane::Center, amane::Center)
        .child(
            Rectangle::new()
                .width(Size::Fixed(8.0))
                .height(Size::Fixed(8.0))
                .radius(4.0)
                .fill(color),
        )
}

/// Units a remote session may be coming in through.
fn remote(unit: &str) -> bool {
    ["sshd", "tailscaled", "sunshine"]
        .iter()
        .any(|name| unit.trim_end_matches(".service") == *name)
}

/// A subtitle's parts, apart by " · ", leaving out the empty ones.
pub fn join_parts(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// "Running · 79 MB", "Stopped", "Failed"; "Off" for a unit systemd
/// doesn't have (a service switched off).
pub fn status_text(state: Option<&UnitState>) -> String {
    let Some(state) = state.filter(|state| state.loaded || !state.active.is_empty()) else {
        return String::from("Off");
    };
    match state.active.as_str() {
        "active" => match state.memory {
            Some(bytes) => format!("Running · {}", amane_kit::format::bytes(bytes)),
            None => String::from("Running"),
        },
        "activating" => String::from("Starting…"),
        "deactivating" => String::from("Stopping…"),
        "failed" => String::from("Failed"),
        _ => String::from("Stopped"),
    }
}

/// The start, stop or restart under way, and how the last one failed.
pub type Actions = Busy<UnitAction>;

/// What [`Actions`] runs.
pub enum UnitAction {}

fn act(unit: String, user: bool, verb: &'static str) {
    let name = unit.clone();
    let job = move || {
        let mut args: Vec<&str> = Vec::new();
        if user {
            args.push("--user");
        }
        args.extend([verb, name.as_str()]);
        run("systemctl", &args, None).map(drop)
    };
    Actions::run(unit, job, Units::refresh);
}

pub fn page() -> Column {
    let snapshot = Snapshot::read();
    let managed = Managed::read();
    let feedback = Feedback::read();
    let units = Units::read();
    let actions = Actions::read();
    let building = BuildJob::read().running();

    let entries = snapshot.for_page("services");
    let linked: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry.on_demand.as_deref())
        .collect();

    let mut services = group().footer(
        "Switches take effect after Apply. Start and Stop act now, and don't survive a restart.",
    );

    if snapshot.loading && entries.is_empty() {
        services = services
            .row(FormRow::new("settings.services.loading", "Reading the configuration…").loading());
    } else if let Some(error) = &snapshot.error {
        services = services.row(crate::problem_row(
            "settings.services.error",
            "The configuration couldn't be read",
            "Switches show the last configuration read; Refresh on About This Computer tries again.",
            error,
        ));
    }

    for entry in entries {
        if linked.contains(&entry.id.as_str()) {
            continue;
        }
        let enabled = managed
            .value(&entry.id, &entry.value)
            .as_bool()
            .unwrap_or(false);
        let state = entry
            .unit
            .as_deref()
            .and_then(|unit| units.get(unit, entry.user_scope));

        let (summary, mover) = controls::summary(entry, &managed, &feedback);
        let mut lines: Vec<String> = Vec::new();
        if entry.unit.is_some() && units.looked {
            lines.push(status_text(state));
        }
        if !summary.is_empty() {
            lines.push(summary);
        }

        let mut row = FormRow::new(
            format!("settings.services.row.{}", entry.id),
            entry.label.clone(),
        );
        // its state is still being asked; once known, a dot says it
        if entry.unit.is_some() && !units.looked {
            row = row.loading();
        } else if entry.unit.is_some() {
            row = row.leading(status_dot(state), DOT);
        }

        if let Some(unit) = entry.unit.clone()
            && enabled
            && state.is_some_and(|s| s.loaded)
        {
            let running =
                state.is_some_and(|s| matches!(s.active.as_str(), "active" | "activating"));
            let (verb, label) = if running {
                ("stop", "Stop")
            } else {
                ("start", "Start")
            };
            let busy = actions.is_busy(&unit);
            if busy {
                lines.push(String::from("Working…"));
            }
            if let Some((failed, why)) = &actions.error
                && *failed == unit
            {
                lines.push(why.clone());
            }
            let user = entry.user_scope;
            let unit2 = unit.clone();
            let label_of = entry.label.clone();
            row = row.trailing(
                PushButton::new(format!("settings.services.{verb}.{}", entry.id), label)
                    .disabled(building || busy || !units.looked)
                    .on_press(move || {
                        // stopping what a remote session rides on asks first
                        if verb == "stop" && remote(&unit2) {
                            let unit = unit2.clone();
                            Dialog::new(
                                "settings.services.stop-dialog",
                                format!("Stop {label_of}?"),
                            )
                            .message(
                                "Anyone connected through it, you included if you're not at \
                                 this computer, is cut off until it starts again.",
                            )
                            .button("Cancel", || {})
                            .destructive("Stop", move || act(unit.clone(), user, "stop"))
                            .open("main");
                        } else {
                            act(unit2.clone(), user, verb);
                        }
                    })
                    .build(),
            );
        }
        row = row
            .subtitle(join_parts(&lines))
            .trailing(controls::switch(entry, &managed).build());
        if let Some(mover) = mover {
            row = row.detail(
                Row::new(vec![boxed(mover.build())]),
                style::control_height(),
            );
        }
        services = services.row(row);

        if let Some(link) = entry.on_demand.as_deref().and_then(|id| snapshot.entry(id)) {
            let pending = managed.current.get(&link.id) != managed.committed.get(&link.id);
            let refused = feedback
                .error
                .as_ref()
                .is_some_and(|(id, _)| *id == link.id);
            let mut linked_row = FormRow::new(
                format!("settings.services.row.{}", link.id),
                "Start only when needed",
            )
            .subtitle("Starts when something connects, and stops when idle")
            .child()
            .trailing(
                controls::switch(link, &managed)
                    .disabled(!link.managed() || !enabled)
                    .build(),
            );
            if pending || refused || !link.managed() {
                let (summary, mover) = controls::summary(link, &managed, &feedback);
                linked_row = linked_row.subtitle(summary);
                if let Some(mover) = mover {
                    linked_row = linked_row.detail(
                        Row::new(vec![boxed(mover.build())]),
                        style::control_height(),
                    );
                }
            }
            services = services.row(linked_row);
        }
    }

    groups(vec![services])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemctl_show_blocks_become_unit_states() {
        let text = "Id=sshd.service\nActiveState=inactive\nSubState=dead\nMemoryCurrent=[not set]\n\n\
                    Id=tailscaled.service\nActiveState=active\nSubState=running\nMemoryCurrent=82731008\n";
        let units = parse_show(text);
        assert_eq!(units.len(), 2);
        assert_eq!(units["sshd.service"].memory, None);
        assert_eq!(status_text(units.get("sshd.service")), "Stopped");
        assert_eq!(
            status_text(units.get("tailscaled.service")),
            "Running · 83 MB"
        );
        assert_eq!(status_text(None), "Off");
        assert!(parse_show("").is_empty());
        assert!(!units["sshd.service"].loaded);
        let loaded = parse_show("Id=sshd.service\nLoadState=loaded\nActiveState=inactive\n");
        assert!(loaded["sshd.service"].loaded);
    }

    #[test]
    fn a_subtitle_leaves_out_empty_parts() {
        let parts = ["Running".to_string(), String::new(), " ".to_string()];
        assert_eq!(join_parts(&parts), "Running");
        assert_eq!(join_parts(&[]), "");
    }
}
