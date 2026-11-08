//! Notifications: what each app that has sent one may do. Each choice
//! changes `notifications.json`, which the shell follows at once.

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
            .width(200.0)
            .build();
            FormRow::new(format!("settings.notifications.app.{key}"), &label)
                .leading(app_icon::tile(&key, &label, icon_side()), icon_side())
                .trailing(menu)
        })
        .collect();

    let apps = group().header("Apps");
    let apps = if rows.is_empty() {
        apps.footer("Apps show here once they have sent a notification.")
    } else {
        apps.rows(rows).footer(
            "List Only skips the banner. Off hides them altogether. Critical notifications, \
             like a low battery, show whatever is chosen.",
        )
    };
    groups(vec![apps])
}
