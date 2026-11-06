//! About This Computer: the running system, whether it's the configuration,
//! the repo, and the settings the registry puts here.

use amane::{Center, Column, Row, Service, TextInput};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::color::Accent;
use amane_kit::form::FormRow;
use amane_kit::icons::{ICON_CHECK, ICON_INFO};
use amane_kit::layout::boxed;
use amane_kit::settings::{change_nixos, nixos};
use amane_kit::tokens::sys;

use super::generations::Generations;
use super::job::BuildJob;
use super::nixos::Snapshot;
use super::repo::{Repo, Running, Toplevel, short};
use super::{Status, controls, status};
use crate::View;
use crate::{group, groups};

/// Whether the running system is the configuration, with Apply when not.
fn status_row() -> FormRow {
    if matches!(status(), Status::Checking) {
        return FormRow::new(
            "settings.about.status",
            "Comparing with the running system…",
        )
        .loading();
    }
    let (glyph, accent, title, text, actions) = match status() {
        Status::Checking => (ICON_INFO, Accent::Gray, "Checking…", String::new(), false),
        Status::UpToDate => (
            ICON_CHECK,
            Accent::Green,
            "Up to date",
            String::from("The running system is this configuration"),
            false,
        ),
        Status::Unknown(why) => (ICON_INFO, Accent::Gray, "Unknown", why, false),
        Status::Pending(reasons) => (
            ICON_INFO,
            Accent::Orange,
            "Changes to apply",
            reasons
                .iter()
                .map(super::Reason::text)
                .collect::<Vec<_>>()
                .join(" · "),
            true,
        ),
    };

    let mut row = FormRow::new("settings.about.status", title)
        .icon(glyph, accent)
        .subtitle(text);
    if actions && !BuildJob::read().busy() {
        row = row
            .trailing(
                PushButton::new("settings.about.changes", "Show Changes")
                    .style(Style::Plain)
                    .on_press(|| crate::open(View::Changes))
                    .build(),
            )
            .trailing(
                PushButton::new("settings.about.apply", "Apply")
                    .style(Style::Filled)
                    .on_press(|| BuildJob::start_build(None))
                    .build(),
            );
    }
    row
}

fn refresh() {
    Snapshot::refresh();
    Repo::refresh();
    Running::refresh();
    Toplevel::refresh();
    Generations::refresh();
}

/// The generation running, and the one the next boot starts when an Apply
/// at Restart or a Try left them apart.
fn generation_text(running: Option<u32>, profile: Option<u32>) -> Option<String> {
    match (running, profile) {
        (Some(n), Some(next)) if next != n => Some(format!("{n} (next boot: {next})")),
        (Some(n), _) => Some(n.to_string()),
        (None, Some(next)) => Some(format!("next boot: {next}")),
        (None, None) => None,
    }
}

pub fn page() -> Column {
    let running = Running::read();
    let repo = Repo::read();
    let generations = Generations::read();
    let snapshot = Snapshot::read();
    let settings = nixos();

    let running_number = generations
        .list
        .iter()
        .find(|g| Some(&g.toplevel) == generations.current.as_ref())
        .map(|g| g.number);
    let generation = generation_text(running_number, generations.profile);
    let revision = match &running.revision {
        Some(rev) => {
            let subject = generations
                .list
                .iter()
                .find(|g| Some(&g.toplevel) == running.toplevel.as_ref())
                .and_then(|g| g.subject.clone());
            match subject {
                Some(subject) => format!("{} · {subject}", short(rev)),
                None => short(rev),
            }
        }
        None => String::from("not recorded"),
    };
    let head = match (&repo.head, &repo.subject) {
        (Some(head), Some(subject)) => format!("{} · {subject}", short(head)),
        (Some(head), None) => short(head),
        (None, _) => repo.error.clone().unwrap_or_default(),
    };

    let repo_path = settings.repo_path().to_string_lossy().into_owned();
    let host = settings.host_attr();
    const REPO: &str = "settings.system.repo";
    const HOST: &str = "settings.system.host";
    if TextInput::text(REPO).is_empty() {
        TextInput::set_text(REPO, &repo_path);
    }
    if TextInput::text(HOST).is_empty() {
        TextInput::set_text(HOST, &host);
    }

    let nixos_version = running.version.clone().map(|version| {
        format!(
            "{version}{}",
            running
                .nixpkgs
                .as_deref()
                .map_or(String::new(), |rev| format!(" · nixpkgs {}", short(rev)))
        )
    });
    // a value, or a spinner while it's still being read
    let info = |id: &str, title: &str, text: Option<String>| {
        let row = FormRow::new(format!("settings.about.{id}"), title);
        match text {
            Some(text) => row.value(text),
            None => row.loading(),
        }
    };

    let fields = Row::new(vec![
        boxed(field_box(
            REPO,
            text_field(REPO, "~/nixos-config").on_submit(|text| {
                let text = text.trim().to_string();
                change_nixos(|n| {
                    n.repo = (!text.is_empty()).then(|| text.clone().into());
                });
                refresh();
            }),
            260.0,
        )),
        boxed(field_box(
            HOST,
            text_field(HOST, "nixos-niri").on_submit(|text| {
                change_nixos(|n| n.host = text.trim().to_string());
                refresh();
            }),
            140.0,
        )),
    ])
    .gap(sys::space::gap::s())
    .align(Center);

    let evaluation = match (&snapshot.error, snapshot.loading) {
        (_, true) => String::from("Evaluating the configuration…"),
        (Some(error), _) => error.lines().last().unwrap_or("").to_string(),
        (None, _) => String::from("Reads the configuration and the running system again"),
    };

    groups(vec![
        group().row(status_row()),
        group()
            .row(info("nixos", "NixOS", nixos_version))
            .row(info("kernel", "Kernel", running.kernel.clone()))
            .row(info("generation", "Generation", generation))
            .row(FormRow::new("settings.about.built", "Built from").subtitle(revision))
            .row({
                let row = FormRow::new("settings.about.head", "Configuration at");
                if head.is_empty() {
                    row.loading()
                } else {
                    row.subtitle(head)
                }
            }),
        group()
            .row(
                FormRow::new("settings.about.repo", "Repository")
                    .subtitle("The flake repo and its nixosConfigurations attribute; Enter to set")
                    .detail(fields, amane_kit::macform::theme::control_height()),
            )
            .row(
                FormRow::new("settings.about.refresh-row", "Refresh")
                    .subtitle(evaluation)
                    .trailing(
                        PushButton::new("settings.about.refresh", "Refresh")
                            .on_press(refresh)
                            .build(),
                    ),
            ),
        group().rows(controls::rows("about")),
    ])
}

#[cfg(test)]
mod tests {
    use super::generation_text;

    #[test]
    fn the_generation_is_the_one_running() {
        assert_eq!(generation_text(Some(79), Some(79)).as_deref(), Some("79"));
        assert_eq!(
            generation_text(Some(79), Some(80)).as_deref(),
            Some("79 (next boot: 80)")
        );
        assert_eq!(generation_text(None, None), None);
    }
}

/// The page's sub-pages, by name: none yet.
pub fn sub(_name: &str) -> Option<(String, Box<dyn amane::Widget>)> {
    None
}
