//! History: the system generations, Restore to go back to one, and how
//! many the boot menu keeps.

use amane::{Center, Column, Row, Service};
use emaki_kit::app::Dialog;
use emaki_kit::buttons::{PushButton, Style};
use emaki_kit::form::FormRow;
use emaki_kit::layout::boxed;
use emaki_kit::style;
use emaki_kit::tokens::sys;

use super::generations::Generations;
use super::job::{BuildJob, Mode};
use crate::{Navigation, group, groups};

pub fn page() -> Column {
    let generations = Generations::read();
    let selected = Navigation::read().selected_generation;
    let running = BuildJob::read().running();

    let rows = generations.list.iter().map(|g| {
        let mut marks = vec![g.when()];
        if Some(&g.toplevel) == generations.current.as_ref() {
            marks.push(String::from("running"));
        }
        if Some(&g.toplevel) == generations.booted.as_ref() {
            marks.push(String::from("started at boot"));
        }
        let number = g.number;
        let chosen = selected == Some(number);
        let mut row = FormRow::new(format!("settings.history.{number}"), g.title())
            .subtitle(marks.join(" · "))
            .value(format!("Version {number}"))
            .selected(chosen)
            .on_press(move || {
                let mut nav = Navigation::write();
                // pressing the chosen one again puts its buttons away
                nav.selected_generation =
                    (nav.selected_generation != Some(number)).then_some(number);
            });
        // the chosen one shows what can be done with it, under it
        if chosen {
            let is_current = Some(&g.toplevel) == generations.current.as_ref();
            let actions = actions(g.toplevel.clone(), number, running || is_current);
            row = row.detail(actions, style::control_height());
            // buttons that can't be pressed say why
            if is_current {
                row = row.subtitle(format!("{} · nothing to try or restore", marks.join(" · ")));
            } else if running {
                row = row.subtitle(format!(
                    "{} · once the build under way is done",
                    marks.join(" · ")
                ));
            }
        }
        row
    });

    let list = group()
        .header("Versions")
        .rows(rows)
        .footer("Choose a version to try it until the next restart, or to go back to it for good.");
    let list = if list.has_rows() {
        list
    } else {
        list.row({
            let row = FormRow::new(
                "settings.history.empty",
                if generations.loading {
                    "Reading the versions…"
                } else {
                    "No versions found"
                },
            );
            if generations.loading {
                row.loading()
            } else {
                row
            }
        })
    };

    groups(vec![
        list,
        group()
            .rows(super::controls::rows("history"))
            .footer("Each version kept is one more entry to start from in the boot menu."),
    ])
}

/// Try and Restore for generation `number`.
fn actions(toplevel: std::path::PathBuf, number: u32, disabled: bool) -> Row {
    let tried = toplevel.clone();
    Row::new(vec![
        boxed(
            PushButton::new("settings.history.try", "Try…")
                .disabled(disabled)
                .on_press(move || {
                    let toplevel = tried.clone();
                    Dialog::new(
                        "settings.history.try-dialog",
                        format!("Try version {number}?"),
                    )
                    .message(
                        "It runs until the next restart, which starts the current one again. \
                         Nothing is recorded.",
                    )
                    .button("Cancel", || {})
                    .main_button("Try", move || {
                        BuildJob::restore(toplevel.clone(), Mode::Test);
                    })
                    .open("main");
                })
                .build(),
        ),
        boxed(
            PushButton::new("settings.history.restore", "Restore…")
                .style(Style::Filled)
                .disabled(disabled)
                .on_press(move || {
                    let toplevel = toplevel.clone();
                    Dialog::new(
                        "settings.history.restore-dialog",
                        format!("Go back to version {number}?"),
                    )
                    .message(
                        "A new version is made from it, so the boot menu keeps both: to come \
                         back, pick today's version here and restore it. Try runs it without \
                         recording it.",
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
    .align(Center)
}
