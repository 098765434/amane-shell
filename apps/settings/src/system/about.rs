//! About This Computer: the running system, whether it's the configuration,
//! the repo, and the settings the registry puts here.

use amane::{Center, Column, Row, Service, Size, TextInput, Widget};
use amane_kit::app::{field_box, text_field};
use amane_kit::buttons::{PushButton, Style};
use amane_kit::card::Card;
use amane_kit::icons::{ICON_CHECK, ICON_INFO};
use amane_kit::layout::{boxed, icon};
use amane_kit::material::{self, Level};
use amane_kit::settings::{change_nixos, nixos};
use amane_kit::tokens::{Styled, sys};

use super::apply_bar::Overlay;
use super::generations::Generations;
use super::job::BuildJob;
use super::nixos::Snapshot;
use super::repo::{Repo, Running, Toplevel, short};
use super::{Status, controls, status};
use crate::{Navigation, note, setting};

fn value(text: impl Into<String>) -> amane::Text {
    amane::Text::new(text)
        .style(sys::r#type::label())
        .color(material::label(Level::Primary))
        .elide()
}

/// Whether the running system is the configuration, with Apply when not.
fn banner() -> Box<dyn Widget> {
    let (glyph, text, actions): (char, String, bool) = match status() {
        Status::Checking => (ICON_INFO, String::from("Checking…"), false),
        Status::UpToDate => (
            ICON_CHECK,
            String::from("Up to date: the running system is this configuration."),
            false,
        ),
        Status::Unknown(why) => (ICON_INFO, why, false),
        Status::Pending(reasons) => (
            ICON_INFO,
            reasons
                .iter()
                .map(super::Reason::text)
                .collect::<Vec<_>>()
                .join(" · "),
            true,
        ),
    };

    let mut row: Vec<Box<dyn Widget>> = vec![
        boxed(icon(
            glyph,
            sys::size::icon::s(),
            material::label(Level::Secondary),
        )),
        boxed(value(text)),
    ];
    if actions && !BuildJob::read().busy() {
        row.push(boxed(
            PushButton::new("settings.about.changes", "Show Changes")
                .style(Style::Plain)
                .on_press(|| Navigation::write().overlay = Some(Overlay::Changes))
                .build(),
        ));
        row.push(boxed(
            PushButton::new("settings.about.apply", "Apply…")
                .style(Style::Filled)
                .on_press(|| BuildJob::start_build(None))
                .build(),
        ));
    }

    // a card fills its parent; this one is a row
    boxed(
        amane::Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(
                sys::size::row::height() + 2.0 * sys::space::inset::m(),
            ))
            .fill(amane::Color::TRANSPARENT)
            .child(
                Card::new(vec![boxed(
                    Row::new(row).gap(sys::space::gap::m()).align(Center),
                )])
                .padding(sys::space::inset::m())
                .build(),
            ),
    )
}

fn refresh() {
    Snapshot::refresh();
    Repo::refresh();
    Running::refresh();
    Toplevel::refresh();
    Generations::refresh();
}

pub fn page() -> Column {
    let running = Running::read();
    let repo = Repo::read();
    let generations = Generations::read();
    let snapshot = Snapshot::read();
    let settings = nixos();

    let generation = match (generations.profile, generations.list.len()) {
        (Some(n), total) if total > 0 => format!("{n} of {total}"),
        _ => String::from("…"),
    };
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
        (None, _) => repo.error.clone().unwrap_or_else(|| String::from("…")),
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

    let mut parts: Vec<Box<dyn Widget>> = vec![
        banner(),
        boxed(setting(
            "NixOS",
            value(format!(
                "{}{}",
                running.version.clone().unwrap_or_else(|| String::from("…")),
                running
                    .nixpkgs
                    .as_deref()
                    .map_or(String::new(), |rev| format!(" · nixpkgs {}", short(rev)))
            )),
        )),
        boxed(setting(
            "Kernel",
            value(running.kernel.clone().unwrap_or_else(|| String::from("…"))),
        )),
        boxed(setting("Generation", value(generation))),
        boxed(setting("Built from", value(revision))),
        boxed(setting("Configuration at", value(head))),
        boxed(setting(
            "Repository",
            Column::new(vec![
                boxed(
                    Row::new(vec![
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
                    .align(Center),
                ),
                boxed(note(
                    "The flake repo and its nixosConfigurations attribute; Enter to set.",
                )),
            ])
            .gap(sys::space::gap::xs()),
        )),
        boxed(
            Row::new(vec![
                boxed(
                    PushButton::new("settings.about.refresh", "Refresh")
                        .on_press(refresh)
                        .build(),
                ),
                boxed(note(match (&snapshot.error, snapshot.loading) {
                    (_, true) => String::from("Evaluating the configuration…"),
                    (Some(error), _) => error.lines().last().unwrap_or("").to_string(),
                    (None, _) => String::new(),
                })),
            ])
            .gap(sys::space::gap::m())
            .align(Center),
        ),
    ];

    let managed = controls::for_page("about");
    if !managed.is_empty() {
        parts.push(boxed(
            Column::new(managed)
                .gap(sys::space::gap::l())
                .width(Size::Parent),
        ));
    }

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}
