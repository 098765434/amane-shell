//! Services: a switch per service the registry lists, with whether its unit
//! is running (from `systemctl show`, every few seconds), "only when
//! needed" for the ones that are installed but not started at boot, and
//! Start/Stop for right now (systemd asks through polkit).

use std::collections::BTreeMap;
use std::thread;
use std::time::Duration;

use amane::{Center, Column, Row, Service, Size, Widget};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::layout::boxed;
use amane_kit::tokens::sys;

use super::controls::{self, Feedback};
use super::job::BuildJob;
use super::nixos::{Managed, Snapshot, run};
use super::storage::human;
use crate::{labelled, note};

/// A unit as systemd reports it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitState {
    pub active: String,
    pub sub: String,
    pub memory: Option<u64>,
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
    args.extend(["show", "-p", "Id,ActiveState,SubState,MemoryCurrent"]);
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
            let fresh = look();
            let mut now = Self::write();
            if fresh == *now {
                now.quiet();
            } else {
                *now = fresh;
            }
        });
    }

    pub fn get(&self, unit: &str, user: bool) -> Option<&UnitState> {
        if user { &self.user } else { &self.system }.get(unit)
    }
}

/// "Running · 79 MB", "Stopped", "Failed"; "—" while unknown.
pub fn status_text(state: Option<&UnitState>) -> String {
    let Some(state) = state else {
        return String::from("—");
    };
    match state.active.as_str() {
        "active" => match state.memory {
            Some(bytes) => format!("Running · {}", human(bytes)),
            None => String::from("Running"),
        },
        "activating" => String::from("Starting…"),
        "deactivating" => String::from("Stopping…"),
        "failed" => String::from("Failed"),
        _ => String::from("Stopped"),
    }
}

/// A Start or Stop under way, and what the last one said when it failed.
pub struct Actions {
    pub busy: Option<String>,
    pub error: Option<(String, String)>,
}

impl Service for Actions {
    fn new() -> Self {
        Self {
            busy: None,
            error: None,
        }
    }

    fn listen() {}
}

fn act(unit: String, user: bool, verb: &'static str) {
    {
        let mut now = Actions::write();
        if now.busy.is_some() {
            now.quiet();
            return;
        }
        now.busy = Some(unit.clone());
        now.error = None;
    }
    thread::spawn(move || {
        let mut args: Vec<&str> = Vec::new();
        if user {
            args.push("--user");
        }
        args.extend([verb, unit.as_str()]);
        let result = run("systemctl", &args, None);
        {
            let mut now = Actions::write();
            now.busy = None;
            if let Err(why) = result {
                now.error = Some((unit.clone(), why));
            }
        }
        Units::refresh();
    });
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

    let mut parts: Vec<Box<dyn Widget>> = vec![boxed(note(
        "Switches take effect after Apply. Start and Stop act now, and don't survive a restart.",
    ))];

    if snapshot.loading && entries.is_empty() {
        parts.push(boxed(note("Reading the configuration…")));
    } else if let Some(error) = &snapshot.error {
        parts.push(boxed(
            note(error.clone()).color(sys::color::status::critical::text()),
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

        let mut lines: Vec<Box<dyn Widget>> = Vec::new();

        let mut head: Vec<Box<dyn Widget>> = vec![boxed(labelled(
            &entry.label,
            controls::switch(entry, &managed).build(),
        ))];
        if entry.unit.is_some() {
            head.push(boxed(note(status_text(state))));
        }
        lines.push(boxed(
            Row::new(head).gap(sys::space::gap::m()).align(Center),
        ));
        lines.extend(controls::notes(entry, &managed, &feedback));

        if let Some(link) = entry.on_demand.as_deref().and_then(|id| snapshot.entry(id)) {
            lines.push(boxed(labelled(
                &link.label,
                controls::switch(link, &managed)
                    .disabled(!link.managed() || !enabled)
                    .build(),
            )));
            let pending = managed.current.get(&link.id) != managed.committed.get(&link.id);
            let refused = feedback
                .error
                .as_ref()
                .is_some_and(|(id, _)| *id == link.id);
            if pending || refused || !link.managed() {
                lines.extend(controls::notes(link, &managed, &feedback));
            }
        }

        if let Some(unit) = entry.unit.clone()
            && enabled
        {
            let running =
                state.is_some_and(|s| matches!(s.active.as_str(), "active" | "activating"));
            let (verb, label) = if running {
                ("stop", "Stop")
            } else {
                ("start", "Start")
            };
            let busy = actions.busy.as_deref() == Some(unit.as_str());
            let user = entry.user_scope;
            let unit2 = unit.clone();
            let mut row: Vec<Box<dyn Widget>> = vec![boxed(
                PushButton::new(format!("settings.services.{verb}.{}", entry.id), label)
                    .style(Style::Tonal)
                    .disabled(building || busy || !units.looked)
                    .on_press(move || act(unit2.clone(), user, verb))
                    .build(),
            )];
            if busy {
                row.push(boxed(note("Working…")));
            }
            lines.push(boxed(Row::new(row).gap(sys::space::gap::m()).align(Center)));
            if let Some((failed, why)) = &actions.error
                && *failed == unit
            {
                lines.push(boxed(
                    note(why.clone()).color(sys::color::status::critical::text()),
                ));
            }
        }

        parts.push(boxed(Column::new(lines).gap(sys::space::gap::xs())));
    }

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
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
            "Running · 82.7 MB"
        );
        assert_eq!(status_text(None), "—");
        assert!(parse_show("").is_empty());
    }
}
