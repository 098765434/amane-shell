//! The desktop: whether its widgets show, and how they look while windows
//! are open over them. Each control changes `desktop.json`, which the
//! shell follows at once.

use amane::Column;
use emaki_kit::buttons::PushButton;
use emaki_kit::form::FormRow;
use emaki_kit::settings::{WidgetStyle, change_desktop, desktop};
use emaki_kit::shell_ipc::{ShellCommand, Widgets};
use emaki_kit::style;
use emaki_kit::thumbnail_picker::ThumbnailPicker;

use crate::{group, groups, previews, switch_row};

pub fn page() -> Column {
    let (show, style) = {
        let now = desktop();
        (now.show_widgets, now.widget_style)
    };

    let styles = [
        ("Dim with windows", WidgetStyle::Automatic),
        ("Always full", WidgetStyle::Full),
        ("Always dimmed", WidgetStyle::Dim),
    ];
    let width = crate::page_width() - 2.0 * style::row_pad_h();
    let mut picker =
        ThumbnailPicker::new("settings.widget-style", previews::WIDTH, previews::HEIGHT)
            .width(width)
            .disabled(!show)
            .chosen(styles.iter().position(|(_, choice)| *choice == style))
            .on_select(move |index| {
                let choice = styles[index].1;
                change_desktop(|now| now.widget_style = choice);
            });
    for (label, choice) in styles {
        picker = picker.option(previews::widgets(choice), label);
    }
    let picker_height = picker.height();

    let edit = PushButton::new("settings.widgets.edit", "Edit Widgets…")
        .disabled(!show)
        .on_press(|| ShellCommand::Widgets(Widgets::Edit(None)).tell())
        .build();

    let mut list = vec![
        group()
            .row(switch_row(
                "settings.show-widgets",
                "Show widgets on the desktop",
                show,
                |on| change_desktop(|now| now.show_widgets = on),
            ))
            .row(
                FormRow::new("settings.widget-style", "While windows are open")
                    .detail(picker.build(), picker_height)
                    .child()
                    .disabled(!show),
            )
            .row(
                FormRow::new("settings.widgets.edit-row", "Add, move and remove widgets")
                    .trailing(edit)
                    .child()
                    .disabled(!show),
            ),
    ];
    list.extend(crate::screenshots::groups());
    groups(list)
}
