//! The desktop: whether its widgets show, and how they look while windows
//! are open over them. Each control changes `desktop.json`, which the
//! shell follows at once.

use amane::{Column, Widget};
use amane_kit::buttons::Radio;
use amane_kit::form::FormRow;
use amane_kit::layout::boxed;
use amane_kit::settings::{WidgetStyle, change_desktop, desktop};

use crate::{group, groups, radios_height, switch_row};

pub fn page() -> Column {
    let (show, style) = {
        let now = desktop();
        (now.show_widgets, now.widget_style)
    };

    let styles = [
        ("Dim while windows are open", WidgetStyle::Automatic),
        ("Always full", WidgetStyle::Full),
        ("Always dimmed", WidgetStyle::Dim),
    ];
    let style_radios: Vec<Box<dyn Widget>> = styles
        .into_iter()
        .enumerate()
        .map(|(index, (label, choice))| {
            boxed(
                Radio::new(format!("settings.widget-style.{index}"), style == choice)
                    .label(label)
                    .on_select(move || change_desktop(|now| now.widget_style = choice))
                    .build(),
            )
        })
        .collect();

    groups(vec![
        group()
            .row(switch_row(
                "settings.show-widgets",
                "Show widgets on the desktop",
                show,
                |on| change_desktop(|now| now.show_widgets = on),
            ))
            .row(
                FormRow::new("settings.widget-style", "Widget style")
                    .detail(Column::new(style_radios), radios_height(3))
                    .disabled(!show),
            ),
    ])
}
