//! The bar under a page while there's something to apply: the edits
//! waiting, the build's progress, what the built system changes, and how
//! it went. Its Changes and Log views take the page's place.

use amane::{Center, Column, Row, ScrollArea, Service, Size, Text, Widget};
use amane_kit::app::Dialog;
use amane_kit::buttons::{PushButton, Style};
use amane_kit::card::Card;
use amane_kit::layout::boxed;
use amane_kit::list::{List, ListItem, trailing_text, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::slider::Slider;
use amane_kit::tokens::{Styled, sys};

use super::job::{BuildJob, Kind, Mode, Phase, Target};
use super::nixos::{Managed, Snapshot};
use crate::{Navigation, PAGE_WIDTH, note};

/// A view that takes the page's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Changes,
    Log,
}

fn button(id: &str, label: &str, style: Style, press: impl Fn() + 'static) -> Box<dyn Widget> {
    boxed(
        PushButton::new(format!("settings.apply.{id}"), label)
            .style(style)
            .on_press(press)
            .build(),
    )
}

fn show(overlay: Overlay) -> impl Fn() + 'static {
    move || Navigation::write().overlay = Some(overlay)
}

fn line(text: impl Into<String>) -> Text {
    Text::new(text)
        .style(sys::r#type::label())
        .color(material::label(Level::Primary))
        .elide()
}

/// The bar, or nothing while there's nothing waiting and no job.
pub fn apply_bar() -> Option<Box<dyn Widget>> {
    let job = BuildJob::read();
    let managed = Managed::read();
    let snapshot = Snapshot::read();
    let changes = managed.summary(&snapshot);

    let (text, buttons): (Box<dyn Widget>, Vec<Box<dyn Widget>>) = match &job.phase {
        Phase::Idle if changes.is_empty() => return None,
        Phase::Idle => (
            boxed(line(format!(
                "{} waiting · {}",
                match changes.len() {
                    1 => String::from("1 change"),
                    n => format!("{n} changes"),
                },
                changes.join(" · ")
            ))),
            vec![
                button("discard", "Discard", Style::Plain, || {
                    Dialog::new("settings.apply.discard-dialog", "Discard the changes?")
                        .message("The settings go back to what was last applied.")
                        .button("Cancel", || {})
                        .destructive("Discard", Managed::discard)
                        .open("main");
                }),
                button("try", "Try", Style::Tonal, || {
                    BuildJob::start_build(Some(Mode::Test));
                }),
                button("apply", "Apply…", Style::Filled, || {
                    BuildJob::start_build(None)
                }),
            ],
        ),
        Phase::Building(progress) => {
            let what = if progress.evaluating {
                String::from("Evaluating…")
            } else {
                format!(
                    "{}{} · {}",
                    progress.current.as_deref().unwrap_or("Building"),
                    if progress.expected > 0 {
                        format!(" ({}/{})", progress.done, progress.expected)
                    } else {
                        String::new()
                    },
                    job.elapsed()
                )
            };
            (
                boxed(
                    Row::new(vec![
                        boxed(Slider::new(progress.percent(), 160.0).inert(true).build()),
                        boxed(line(what)),
                    ])
                    .gap(sys::space::gap::m())
                    .align(Center),
                ),
                vec![
                    button("log", "Show Log", Style::Plain, show(Overlay::Log)),
                    button("cancel", "Cancel", Style::Destructive, BuildJob::cancel),
                ],
            )
        }
        Phase::Built(built) => {
            let changed = built.diff.len();
            let mut text = format!(
                "Ready: {}",
                match changed {
                    0 => String::from("nothing changes"),
                    1 => String::from("1 package changes"),
                    n => format!("{n} packages change"),
                }
            );
            if built.needs_restart {
                text.push_str(" · needs a restart");
            }
            let mut buttons = vec![
                button(
                    "changes",
                    "Show Changes",
                    Style::Plain,
                    show(Overlay::Changes),
                ),
                button("drop", "Cancel", Style::Plain, BuildJob::dismiss),
            ];
            if job.target == Target::Head {
                buttons.push(button("try-built", "Try", Style::Tonal, || {
                    BuildJob::activate(Mode::Test);
                }));
            }
            if built.needs_restart {
                buttons.push(button("boot", "Apply at Restart", Style::Tonal, || {
                    BuildJob::activate(Mode::Boot);
                }));
            }
            buttons.push(button("switch", "Apply Now", Style::Filled, || {
                BuildJob::activate(Mode::Switch);
            }));
            (boxed(line(text)), buttons)
        }
        Phase::Activating { .. } => (
            boxed(line("Applying… authenticate when asked.")),
            vec![button("log", "Show Log", Style::Plain, show(Overlay::Log))],
        ),
        Phase::Done {
            mode,
            generation,
            commit,
        } => {
            let text = match (mode, commit) {
                (Mode::Test, _) => String::from("Tried · Apply to keep it"),
                (_, Some(Err(why))) => format!("Applied, but not committed: {why}"),
                (_, Some(Ok(message))) => format!(
                    "Applied{} · {message}",
                    generation.map_or(String::new(), |n| format!(" · generation {n}"))
                ),
                (Mode::Boot, None) => String::from("Applied at the next restart"),
                (_, None) => format!(
                    "Applied{}",
                    generation.map_or(String::new(), |n| format!(" · generation {n}"))
                ),
            };
            (
                boxed(line(text)),
                vec![
                    button("log", "Show Log", Style::Plain, show(Overlay::Log)),
                    button("dismiss", "Dismiss", Style::Tonal, BuildJob::dismiss),
                ],
            )
        }
        Phase::Failed { stage, message } => (
            boxed(
                line(format!(
                    "{} failed: {}",
                    stage.name(),
                    message.lines().last().unwrap_or("")
                ))
                .color(sys::color::status::critical::text()),
            ),
            vec![
                button("log", "Show Log", Style::Plain, show(Overlay::Log)),
                button("dismiss", "Dismiss", Style::Tonal, BuildJob::dismiss),
            ],
        ),
    };

    // a card fills its parent; the bar is two rows
    Some(boxed(
        amane::Rectangle::new()
            .width(Size::Parent)
            .height(Size::Fixed(
                2.0 * sys::size::row::height()
                    + sys::space::gap::s()
                    + 2.0 * sys::space::inset::m(),
            ))
            .fill(amane::Color::TRANSPARENT)
            .child(
                Card::new(vec![
                    text,
                    boxed(
                        Row::new(buttons)
                            .gap(sys::space::gap::s())
                            .width(Size::Parent)
                            .justify(amane::End),
                    ),
                ])
                .gap(sys::space::gap::s())
                .padding(sys::space::inset::m())
                .build(),
            ),
    ))
}

fn back() -> Box<dyn Widget> {
    button("back", "Back", Style::Tonal, || {
        Navigation::write().overlay = None
    })
}

/// The built system beside the running one.
pub fn changes_view() -> Column {
    with_row_width(PAGE_WIDTH, changes)
}

fn changes() -> Column {
    let job = BuildJob::read();
    let managed = Managed::read();
    let snapshot = Snapshot::read();

    let mut parts: Vec<Box<dyn Widget>> = vec![back()];

    let edits = managed.summary(&snapshot);
    if !edits.is_empty() {
        parts.push(boxed(
            List::new(
                edits
                    .iter()
                    .enumerate()
                    .map(|(i, edit)| {
                        ListItem::new(format!("settings.changes.edit.{i}"), edit).bare(true)
                    })
                    .collect(),
            )
            .header("Settings")
            .build(),
        ));
    }

    if let Phase::Built(built) = &job.phase {
        for (kind, header) in [
            (Kind::Changed, "Updates"),
            (Kind::Added, "Added"),
            (Kind::Removed, "Removed"),
        ] {
            let items: Vec<ListItem> = built
                .diff
                .iter()
                .filter(|line| line.kind() == kind)
                .enumerate()
                .map(|(i, line)| {
                    let versions = match kind {
                        Kind::Changed => format!("{} → {}", line.from, line.to),
                        Kind::Added => line.to.clone(),
                        Kind::Removed => line.from.clone(),
                    };
                    let detail = match &line.size {
                        Some(size) => format!("{versions} · {size}"),
                        None => versions,
                    };
                    ListItem::new(format!("settings.changes.{header}.{i}"), &line.name)
                        .bare(true)
                        .trailing(trailing_text(detail, material::label(Level::Secondary)))
                })
                .collect();
            if !items.is_empty() {
                parts.push(boxed(List::new(items).header(header).build()));
            }
        }
        if built.diff.is_empty() {
            parts.push(boxed(note(if built.raw_diff.trim().is_empty() {
                String::from("No package changes.")
            } else {
                built.raw_diff.clone()
            })));
        }
        if built.needs_restart {
            parts.push(boxed(note(
                "The kernel, its modules or the initrd change: a restart is needed for those.",
            )));
        }
    } else {
        parts.push(boxed(note(
            "What the build changes shows here once it's built.",
        )));
    }

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}

/// The build log.
pub fn log_view() -> Column {
    let job = BuildJob::read();
    let text = job.log.iter().cloned().collect::<Vec<_>>().join("\n");
    let copy = text.clone();

    Column::new(vec![
        boxed(
            Row::new(vec![
                back(),
                button("copy", "Copy Log", Style::Plain, move || {
                    amane::Clipboard::copy(copy.clone());
                }),
            ])
            .gap(sys::space::gap::s()),
        ),
        boxed(
            ScrollArea::new(
                "settings.scroll.log",
                Column::new(vec![boxed(
                    Text::new(if text.is_empty() {
                        String::from("Nothing yet.")
                    } else {
                        text
                    })
                    .style(sys::r#type::caption())
                    .font("monospace")
                    .color(material::label(Level::Primary))
                    .wrap(),
                )])
                .width(Size::Parent),
            )
            .width(Size::Parent)
            .height(Size::Parent),
        ),
    ])
    .gap(sys::space::gap::m())
    .width(Size::Parent)
    .height(Size::Parent)
}
