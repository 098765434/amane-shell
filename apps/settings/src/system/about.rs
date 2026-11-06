//! About This Computer: the running system, whether it's the configuration,
//! the repo, and the settings the registry puts here.

use amane::{Center, Column, Row, Service, TextInput};
use amane_kit::app::{commit_on_blur, field_box, text_field};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::color::Accent;
use amane_kit::form::FormRow;
use amane_kit::icons::{ICON_CHECK, ICON_INFO};
use amane_kit::layout::boxed;
use amane_kit::settings::{change_nixos, nixos};

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
    let _ = (&snapshot, &settings);

    groups(vec![
        group().custom(hero(running.version.as_deref()), HERO),
        group().row(status_row()),
        group()
            .row(info("nixos", "NixOS", nixos_version))
            .row(info("kernel", "Kernel", running.kernel.clone()))
            .row(info("generation", "Generation", generation))
            .row(FormRow::new("settings.about.built", "Running configuration").subtitle(revision))
            .row({
                let row = FormRow::new("settings.about.head", "Latest in the folder");
                if head.is_empty() {
                    row.loading()
                } else {
                    row.subtitle(head)
                }
            }),
        group().row(
            FormRow::new("settings.about.advanced-row", "Configuration folder")
                .value(tidy(&settings_repo()))
                .on_press(|| crate::open(View::Sub("advanced"))),
        ),
        group().rows(controls::rows("about")),
    ])
}

/// How tall the computer's card is.
const HERO: f32 = 104.0;

/// This computer's name, as the kernel has it.
fn host_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|name| name.trim().to_string())
        .unwrap_or_default()
}

fn settings_repo() -> String {
    nixos().repo_path().to_string_lossy().into_owned()
}

/// A path with the home folder as `~`.
fn tidy(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

/// The computer's card: its glyph, its name and the NixOS it runs, as
/// macOS's About This Mac opens.
fn hero(version: Option<&str>) -> amane::Rectangle {
    use amane::{Color, Rectangle, Size, Text};
    use amane_kit::macform::theme as mac;
    use amane_kit::tokens::Styled;
    let tile = Rectangle::new()
        .width(Size::Fixed(60.0))
        .height(Size::Fixed(60.0))
        .radius(14.0)
        .fill(Color::from("#0091ff"))
        .align_child(Center, Center)
        .child(amane_kit::layout::icon(
            amane_kit::icons::ICON_COMPUTER,
            36.0,
            Color::WHITE,
        ));
    let name = host_name();
    let mut lines: Vec<Box<dyn amane::Widget>> = vec![boxed(
        Text::new(if name.is_empty() {
            String::from("This computer")
        } else {
            name
        })
        .style(mac::page_title_font())
        .color(mac::label_primary()),
    )];
    lines.push(match version {
        Some(version) => boxed(
            Text::new(format!("NixOS {version}"))
                .style(mac::subtitle_font())
                .color(mac::label_secondary()),
        ),
        None => boxed(amane_kit::spinner::Spinner::new(14.0).build()),
    });
    Rectangle::new()
        .width(Size::Parent)
        .height(Size::Fixed(HERO))
        .fill(Color::TRANSPARENT)
        .padding(amane::Padding {
            left: mac::row_pad_h(),
            ..amane::Padding::default()
        })
        .align_child(amane::Start, Center)
        .child(
            Row::new(vec![boxed(tile), boxed(Column::new(lines).gap(2.0))])
                .gap(16.0)
                .align(Center),
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
    if TextInput::text(REPO).is_empty() {
        TextInput::set_text(REPO, &repo_path);
    }
    if TextInput::text(HOST).is_empty() {
        TextInput::set_text(HOST, &host);
    }
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
    let mut refresh_row = FormRow::new("settings.about.refresh-row", "Read Again")
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
                240.0,
            )),
        FormRow::new("settings.about.host", "Machine name in the flake")
            .subtitle("Which of its nixosConfigurations is this computer")
            .trailing(field_box(
                HOST,
                text_field(HOST, "nixos-niri").on_submit(keep_host),
                160.0,
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
pub fn sub(name: &str) -> Option<(String, Box<dyn amane::Widget>)> {
    (name == "advanced").then(|| {
        (
            String::from("Configuration"),
            Box::new(advanced()) as Box<dyn amane::Widget>,
        )
    })
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
