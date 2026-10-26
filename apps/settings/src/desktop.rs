//! The desktop: whether its widgets show, and how they look while windows
//! are open over them. Each control changes `desktop.json`, which the
//! shell follows at once.

use amane::{Column, Widget};
use amane_kit::buttons::{Radio, Switch};
use amane_kit::layout::boxed;
use amane_kit::settings::{WidgetStyle, change_desktop, desktop};
use amane_kit::tokens::sys;

use crate::setting;

pub fn page() -> Column {
    let (show, style) = {
        let now = desktop();
        (now.show_widgets, now.widget_style)
    };

    let shown = Switch::new("settings.show-widgets", show)
        .on_toggle(|on| change_desktop(|now| now.show_widgets = on))
        .build();

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

    Column::new(vec![
        boxed(setting("Show widgets on the desktop", shown)),
        boxed(setting("Widget style", Column::new(style_radios))),
    ])
    .gap(sys::space::gap::l())
}
