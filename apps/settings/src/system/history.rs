//! History: the system generations, and Restore to go back to one.

use amane::{Column, Service};
use amane_kit::app::Dialog;
use amane_kit::buttons::{PushButton, Style};
use amane_kit::form::FormRow;

use super::generations::Generations;
use super::job::{BuildJob, Mode};
use super::repo::short;
use crate::{Navigation, group, groups};

pub fn page() -> Column {
    let generations = Generations::read();
    let selected = Navigation::read().selected_generation;
    let running = BuildJob::read().running();

    let rows = generations.list.iter().map(|g| {
        let mut marks = vec![g.when()];
        if Some(&g.toplevel) == generations.current.as_ref() {
            marks.push(String::from("current"));
        }
        if Some(&g.toplevel) == generations.booted.as_ref() {
            marks.push(String::from("booted"));
        }
        let number = g.number;
        FormRow::new(format!("settings.history.{number}"), g.title())
            .subtitle(marks.join(" · "))
            .value(format!("#{number}"))
            .selected(selected == Some(number))
            .on_press(move || Navigation::write().selected_generation = Some(number))
    });

    let chosen = match selected.and_then(|n| generations.find(n)) {
        Some(g) => {
            let is_current = Some(&g.toplevel) == generations.current.as_ref();
            let toplevel = g.toplevel.clone();
            let toplevel2 = g.toplevel.clone();
            let number = g.number;
            FormRow::new("settings.history.chosen", format!("Generation {number}"))
                .subtitle(format!(
                    "NixOS {}{}",
                    g.version,
                    g.revision
                        .as_deref()
                        .map_or(String::new(), |r| format!(" · {}", short(r)))
                ))
                .trailing(
                    PushButton::new("settings.history.try", "Try")
                        .disabled(running || is_current)
                        .on_press(move || {
                            BuildJob::restore(toplevel2.clone(), Mode::Test);
                        })
                        .build(),
                )
                .trailing(
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
                )
        }
        None => FormRow::new("settings.history.chosen", "No generation selected")
            .subtitle("Select one below to try it or go back to it"),
    };

    let list = group().header("Generations").rows(rows);
    let list = if list.is_empty() {
        list.row(FormRow::new(
            "settings.history.empty",
            if generations.loading {
                "Reading the generations…"
            } else {
                "No generations found"
            },
        ))
    } else {
        list
    };

    groups(vec![group().row(chosen), list])
}
