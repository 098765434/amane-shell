//! About: the running system, whether it's the configuration, the repo,
//! and Emaki Settings' own version and settings files.

use amane::{Column, Service};
use emaki_kit::app::{commit_on_blur, field_box, text_field};
use emaki_kit::buttons::{PushButton, Style};
use emaki_kit::color::Accent;
use emaki_kit::form::FormRow;
use emaki_kit::icons::{ICON_CHECK, ICON_INFO};
use emaki_kit::layout::boxed;
use emaki_kit::settings::{change_nixos, nixos};

use super::generations::Generations;
use super::job::BuildJob;
use super::nixos::Snapshot;
use super::repo::{Repo, Running, Toplevel, short};
use super::{Status, status};
use crate::View;
use crate::about::{HERO, card, tidy_path};
use crate::{group, groups};

/// Whether the running system is the configuration, with Apply when not.
fn status_row() -> FormRow {
    let (glyph, accent, title, text, actions) = match status() {
        Status::Checking => {
            return FormRow::new(
                "settings.about.status",
                "Comparing with the running system…",
            )
            .loading();
        }
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

    let nixos_version = running.version.clone().map(|version| {
        format!(
            "{version}{}",
            running
                .nixpkgs
                .as_deref()
                .map_or(String::new(), |rev| format!(" · packages {}", short(rev)))
        )
    });
    // a value, or a spinner while it's still being read
    let info = |id: &str, title: &str, text: Option<String>| {
        let id = format!("settings.about.{id}");
        match text {
            Some(text) => FormRow::property(id, title, text),
            None => FormRow::new(id, title).loading(),
        }
    };

    groups(vec![
        group().custom(hero(running.version.as_deref()), HERO),
        group().row(status_row()),
        group()
            .row(info("nixos", "NixOS", nixos_version))
            .row(info("kernel", "Kernel", running.kernel.clone()))
            .row(info("generation", "System version", generation))
            .row(FormRow::property(
                "settings.about.built",
                "Built from",
                revision,
            ))
            .row({
                let id = "settings.about.head";
                if head.is_empty() {
                    FormRow::new(id, "Latest saved change").loading()
                } else {
                    FormRow::property(id, "Latest saved change", head)
                }
            }),
        group().row(
            FormRow::new("settings.about.advanced-row", "Configuration folder")
                .value(tidy_path(std::path::Path::new(&settings_repo())))
                .on_press(|| crate::open(View::Sub("advanced"))),
        ),
        group()
            .header("Emaki")
            .row(FormRow::property(
                "settings.about.emaki",
                "Emaki Settings",
                format!("Version {}", env!("CARGO_PKG_VERSION")),
            ))
            .row(
                FormRow::new("settings.about.files-link", "Settings files")
                    .value(format!("{} files", crate::about::file_count()))
                    .on_press(|| crate::open(View::Sub("files"))),
            ),
    ])
}

/// This computer's name, as the kernel has it.
fn host_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_default()
}

fn settings_repo() -> String {
    nixos().repo_path().to_string_lossy().into_owned()
}

/// The computer's card: its glyph, its name and the NixOS it runs.
fn hero(version: Option<&str>) -> amane::Rectangle {
    use amane::Text;
    use emaki_kit::style;
    use emaki_kit::tokens::Styled;
    let name = host_name();
    let under = match version {
        Some(version) => boxed(
            Text::new(format!("NixOS {version}"))
                .style(style::subtitle_font())
                .color(style::label_secondary()),
        ),
        None => boxed(emaki_kit::spinner::Spinner::new(14.0).build()),
    };
    card(
        Accent::Blue.color(),
        emaki_kit::icons::ICON_COMPUTER,
        if name.is_empty() {
            String::from("This computer")
        } else {
            name
        },
        under,
    )
}

/// The Advanced sub-page: where the configuration is, and reading it again.
fn advanced() -> Column {
    let snapshot = Snapshot::read();
    let settings = nixos();
    let repo_path = settings.repo_path().to_string_lossy().into_owned();
    let host = settings.host_attr();
    const REPO: &str = "settings.system.repo";
    const HOST: &str = "settings.system.host";
    emaki_kit::app::fill_once(REPO, || repo_path.clone());
    emaki_kit::app::fill_once(HOST, || host.clone());
    let keep_repo = |text: String| {
        let text = text.trim().to_string();
        let now = nixos().repo_path().to_string_lossy().into_owned();
        if text != now {
            change_nixos(|n| {
                n.repo = (!text.is_empty()).then(|| text.clone().into());
            });
            refresh();
        }
    };
    let keep_host = |text: String| {
        let text = text.trim().to_string();
        if text != nixos().host_attr() {
            change_nixos(|n| n.host = text.clone());
            refresh();
        }
    };
    commit_on_blur(REPO, keep_repo);
    commit_on_blur(HOST, keep_host);

    let evaluation = match (&snapshot.error, snapshot.loading) {
        (_, true) => String::from("Reading the configuration…"),
        (Some(_), _) => String::from("The configuration couldn't be read"),
        (None, _) => String::from("Reads the configuration and the running system again"),
    };
    let mut refresh_row =
        FormRow::new("settings.about.refresh-row", "Read the configuration again")
            .subtitle(evaluation)
            .trailing(
                PushButton::new("settings.about.refresh", "Refresh")
                    .disabled(snapshot.loading)
                    .on_press(refresh)
                    .build(),
            );
    if snapshot.loading {
        refresh_row = refresh_row.loading();
    }

    let mut rows = vec![
        FormRow::new("settings.about.repo", "Configuration folder")
            .subtitle("The git folder with the flake this computer is built from")
            .trailing(field_box(
                REPO,
                text_field(REPO, "~/nixos-config").on_submit(keep_repo),
                crate::FIELD_W,
            )),
        FormRow::new("settings.about.host", "Machine name in the flake")
            .subtitle("Which of its nixosConfigurations is this computer")
            .trailing(field_box(
                HOST,
                text_field(HOST, "nixos-niri").on_submit(keep_host),
                crate::MENU_W_NARROW,
            )),
        refresh_row,
    ];
    if let Some(error) = &snapshot.error {
        rows.push(crate::problem_row(
            "settings.about.error",
            "The configuration couldn't be read",
            "Settings that come from it can't be changed until it can.",
            error,
        ));
    }
    groups(vec![group().rows(rows).footer(
        "Changes here are kept when you press Return or click away.",
    )])
}

/// The page's sub-pages: Advanced.
/// About's sub-pages: the configuration folder, and where the shell's settings files are.
pub fn sub(name: &str) -> Option<(String, Box<dyn amane::Widget>)> {
    match name {
        "advanced" => Some((
            String::from("Configuration folder"),
            Box::new(advanced()) as Box<dyn amane::Widget>,
        )),
        "files" => crate::about::sub(name),
        _ => None,
    }
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
