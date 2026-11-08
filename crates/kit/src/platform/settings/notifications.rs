//! `notifications.json`: what each app's notifications may do.

use super::*;

/// What an app's notifications may do.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case")]
pub enum AppRule {
    /// A banner, then the list in the notification center.
    #[default]
    Allow,
    /// Straight into the notification center, with no banner.
    CenterOnly,
    /// Nowhere; a critical one still shows, as through Do Not Disturb.
    Off,
}

impl AppRule {
    /// Every rule, in the order a menu lists them.
    pub const ALL: [Self; 3] = [Self::Allow, Self::CenterOnly, Self::Off];

    /// What a menu calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Allow => "Banners and List",
            Self::CenterOnly => "List Only",
            Self::Off => "Off",
        }
    }
}

/// The notification rules, by app key (see [`notification_app_key`]); an
/// app that isn't here is allowed.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct NotificationRules {
    /// Each app's rule, by its key.
    pub apps: BTreeMap<String, AppRule>,
}

impl NotificationRules {
    /// The rule for the app keyed `key`.
    pub fn rule(&self, key: &str) -> AppRule {
        self.apps.get(key).copied().unwrap_or_default()
    }
}

/// What a notification's app is known by: its desktop entry, else its app
/// name, lowercased. The center groups by it, and the rules are keyed by it.
pub fn notification_app_key(desktop_entry: &str, app_name: &str) -> String {
    let key = if desktop_entry.is_empty() {
        app_name
    } else {
        desktop_entry
    };
    key.to_lowercase()
}

settings_file!(
    NotificationRules,
    crate::app_ids::SHELL,
    "notifications",
    notification_rules,
    change_notification_rules
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_app_is_known_by_its_entry_else_its_name() {
        assert_eq!(
            notification_app_key("firefox", "Firefox Nightly"),
            "firefox"
        );
        assert_eq!(notification_app_key("", "Slack"), "slack");
        assert_eq!(
            notification_app_key("Org.Gnome.Nautilus", ""),
            "org.gnome.nautilus"
        );
    }

    #[test]
    fn rules_read_as_kebab_case_and_default_to_allow() {
        let rules: NotificationRules =
            serde_json::from_str(r#"{"apps":{"slack":"center-only","spam":"off"}}"#).unwrap();
        assert_eq!(rules.rule("slack"), AppRule::CenterOnly);
        assert_eq!(rules.rule("spam"), AppRule::Off);
        assert_eq!(rules.rule("firefox"), AppRule::Allow);
    }
}
