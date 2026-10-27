//! History: the system generations, and Restore to go back to one.

use amane::{Center, Column, Row, Service, Size, Widget};
use amane_kit::app::Dialog;
use amane_kit::buttons::{PushButton, Style};
use amane_kit::layout::boxed;
use amane_kit::list::{List, ListItem, trailing_text, with_row_width};
use amane_kit::material::{self, Level};
use amane_kit::tokens::sys;

use super::generations::Generations;
use super::job::{BuildJob, Mode};
use super::repo::short;
use crate::{Navigation, PAGE_WIDTH, note};

pub fn page() -> Column {
    let generations = Generations::read();
    let selected = Navigation::read().selected_generation;
    let running = BuildJob::read().running();

    let items = || {
        generations
            .list
            .iter()
            .map(|g| {
                let mut marks = Vec::new();
                if Some(&g.toplevel) == generations.current.as_ref() {
                    marks.push("current");
                }
                if Some(&g.toplevel) == generations.booted.as_ref() {
                    marks.push("booted");
                }
                let number = g.number;
                ListItem::new(
                    format!("settings.history.{number}"),
                    format!("{} · {}", g.when(), g.title()),
                )
                .selected(selected == Some(number))
                .trailing(trailing_text(
                    marks.join(", "),
                    material::label(Level::Secondary),
                ))
                .on_press(move || Navigation::write().selected_generation = Some(number))
            })
            .collect::<Vec<ListItem>>()
    };

    let mut parts: Vec<Box<dyn Widget>> = Vec::new();

    if let Some(g) = selected.and_then(|n| generations.find(n)) {
        let is_current = Some(&g.toplevel) == generations.current.as_ref();
        let toplevel = g.toplevel.clone();
        let toplevel2 = g.toplevel.clone();
        let number = g.number;
        parts.push(boxed(
            Column::new(vec![
                boxed(note(format!(
                    "Generation {number} · NixOS {}{}",
                    g.version,
                    g.revision.as_deref().map_or(String::new(), |r| format!(" · {}", short(r)))
                ))),
                boxed(
                    Row::new(vec![
                        boxed(
                            PushButton::new("settings.history.try", "Try")
                                .style(Style::Tonal)
                                .disabled(running || is_current)
                                .on_press(move || {
                                    BuildJob::restore(toplevel2.clone(), Mode::Test);
                                })
                                .build(),
                        ),
                        boxed(
                            PushButton::new("settings.history.restore", "Restore…")
                                .style(Style::Filled)
                                .disabled(running || is_current)
                                .on_press(move || {
                                    let toplevel = toplevel.clone();
                                    Dialog::new(
                                        "settings.history.restore-dialog",
                                        format!("Go back to generation {number}?"),
                                    )
                                    .message(
                                        "A new generation is made from it, so the boot menu keeps both. Try first runs it without recording it.",
                                    )
                                    .button("Cancel", || {})
                                    .main_button("Restore", move || {
                                        BuildJob::restore(toplevel.clone(), Mode::Switch);
                                    })
                                    .open("main");
                                })
                                .build(),
                        ),
                    ])
                    .gap(sys::space::gap::s())
                    .align(Center),
                ),
            ])
            .gap(sys::space::gap::s()),
        ));
    } else {
        parts.push(boxed(note(
            "Select a generation to try it or go back to it.",
        )));
    }

    parts.push(boxed(with_row_width(PAGE_WIDTH, || {
        List::new(items())
            .empty(if generations.loading {
                "Reading the generations…"
            } else {
                "No generations found."
            })
            .build()
    })));

    Column::new(parts)
        .gap(sys::space::gap::l())
        .width(Size::Parent)
}
