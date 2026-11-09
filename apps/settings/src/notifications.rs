//! Notifications: the sound one plays ([`crate::sound::alerts`]), and what
//! each app that has sent one may do. Each choice changes
//! `notifications.json`, which the shell follows at once.

use amane::Column;
use emaki_kit::app_icon;
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::FormRow;
use emaki_kit::settings::{AppRule, change_notification_rules, notification_rules, notifiers_seen};

use crate::system::apps::icon_side;
use crate::{group, groups};

pub fn page() -> Column {
    let rules = notification_rules().clone();

    let rows: Vec<FormRow> = notifiers_seen()
        .into_iter()
        .map(|(key, name)| {
            let label = if name.is_empty() { key.clone() } else { name };
            let changed = key.clone();
            let menu = Dropdown::choice(
                format!("settings.notifications.app.{key}.rule"),
                AppRule::ALL.map(|rule| (rule.label(), rule)),
                rules.rule(&key),
                move |rule| {
                    change_notification_rules(|now| {
                        if rule == AppRule::Allow {
                            now.apps.remove(&changed);
                        } else {
                            now.apps.insert(changed.clone(), rule);
                        }
                    });
                },
            )
            .width(crate::MENU_W)
            .build();
            FormRow::new(format!("settings.notifications.app.{key}"), &label)
                .leading(app_icon::tile(&key, &label, icon_side()), icon_side())
                .trailing(menu)
        })
        .collect();

    let apps = if rows.is_empty() {
        group().header("Apps").row(crate::empty_row(
            "settings.notifications.none",
            "Apps show here once they've sent a notification, each with its own choice.",
        ))
    } else {
        group().header("Apps").rows(rows)
    };
    groups(vec![
        crate::sound::alerts(),
        apps.footer(
            "List Only skips the banner. Off hides them altogether. Critical notifications, \
         like a low battery, show whatever is chosen.",
        ),
    ])
}
