//! The questions a drop asks under the tiles: where to move or copy,
//! what to call a renamed file, whether to replace.

use super::*;

/// A question's title, its detail, a field if it has one, and its answers.
type Asked = (
    String,
    String,
    Option<Box<dyn Widget>>,
    Vec<Box<dyn Widget>>,
);

pub(super) fn ask(prompt: Prompt, width: f32) -> Rectangle {
    let primary = material::label(Level::Primary);
    let secondary = material::label(Level::Secondary);
    let (title, detail, field, buttons): Asked = match prompt {
        Prompt::Conflict { job, names } => {
            let folder = match &job {
                Job::Copy { folder, .. } | Job::Move { folder, .. } => name(folder),
                _ => String::new(),
            };
            let title = match names.as_slice() {
                [one] => format!("“{one}” is already in {folder}"),
                many => format!("{} items are already in {folder}", many.len()),
            };
            let (keep, replace) = (job.clone(), job);
            (
                title,
                String::from("Keep both, with a number after the new one's name, or replace?"),
                None,
                vec![
                    boxed(button("emaki-drop.stop", "Stop", Style::Plain, dismiss)),
                    boxed(button(
                        "emaki-drop.replace",
                        "Replace",
                        Style::Destructive,
                        move || {
                            dismiss();
                            tasks::run(replace.clone(), false);
                        },
                    )),
                    boxed(button(
                        "emaki-drop.keep",
                        "Keep Both",
                        Style::Filled,
                        move || {
                            dismiss();
                            tasks::run(keep.clone(), true);
                        },
                    )),
                ],
            )
        }
        Prompt::Phone {
            what,
            devices,
            tile,
        } => {
            let mut buttons: Vec<Box<dyn Widget>> = vec![boxed(button(
                "emaki-drop.cancel",
                "Cancel",
                Style::Plain,
                dismiss,
            ))];
            for (id, device_name) in devices.iter().take(3) {
                let (what, device, device_name) = (what.clone(), id.clone(), device_name.clone());
                buttons.push(boxed(
                    PushButton::new(format!("emaki-drop.phone.{id}"), &device_name)
                        .icon(ICON_PHONE)
                        .style(Style::Filled)
                        .on_press(move || {
                            dismiss();
                            tasks::start(Job::Phone {
                                what: what.clone(),
                                device: device.clone(),
                                device_name: device_name.clone(),
                                tile,
                            });
                        })
                        .build(),
                ));
            }
            let (title, detail) = if devices.is_empty() {
                (
                    String::from("No phone is connected"),
                    String::from(
                        "Pair one with KDE Connect in Quick Settings › Phone, on the same network.",
                    ),
                )
            } else {
                (
                    String::from("Send to which phone?"),
                    String::from("Choose one for this tile while editing to skip this."),
                )
            };
            (title, detail, None, buttons)
        }
        Prompt::Rename {
            files,
            folder,
            tile,
        } => {
            let title = match files.as_slice() {
                [one] => format!("Rename “{}”", name(one)),
                many => format!("Rename {} items", many.len()),
            };
            let detail = format!("Moved into {}", name(&folder));
            let (typed, moved) = ((files.clone(), folder.clone()), (files, folder));
            let input = text_field(RENAME, "Name")
                .focused()
                .on_submit(move |text| rename(&typed.0, &typed.1, &text, tile));
            (
                title,
                detail,
                Some(boxed(field_box(RENAME, input, width))),
                vec![
                    boxed(button("emaki-drop.cancel", "Cancel", Style::Plain, dismiss)),
                    boxed(button(
                        "emaki-drop.rename",
                        "Move",
                        Style::Filled,
                        move || {
                            rename(&moved.0, &moved.1, &TextInput::text(RENAME), tile);
                        },
                    )),
                ],
            )
        }
    };
    let mut rows: Vec<Box<dyn Widget>> = vec![
        boxed(label(&title, sys::r#type::label(), primary, width)),
        boxed(label(&detail, sys::r#type::caption(), secondary, width)),
    ];
    rows.extend(field);
    rows.push(boxed(Row::new(buttons).gap(6.0).align(Center)));
    frame(width, PROMPT)
        .align_child(Start, Center)
        .child(Column::new(rows).gap(8.0))
}

pub(super) fn button(
    id: &'static str,
    title: &str,
    style: Style,
    press: impl Fn() + 'static,
) -> Rectangle {
    PushButton::new(id, title)
        .style(style)
        .on_press(press)
        .build()
}

pub(super) fn rename(files: &[PathBuf], folder: &Path, typed: &str, tile: usize) {
    if typed.trim().is_empty() {
        return;
    }
    dismiss();
    tasks::start(Job::Rename {
        moves: tasks::renamed(files, folder, typed),
        tile,
    });
}

/// Puts the question away.
pub(super) fn dismiss() {
    state::change(|ui| ui.prompt = None);
}
