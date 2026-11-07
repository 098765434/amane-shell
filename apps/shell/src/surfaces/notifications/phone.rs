//! **Open on Phone** on the phone's notifications. KDE Connect shows the
//! phone's notifications here as its own; the matching one in
//! `PhoneDevice::notifications()` carries the app's package (from
//! Android's notification key), and the button opens that app in a window
//! of its own (`amane-mirror --app`). It's offered once the phone's screen
//! was set up on the Phone page, or adb has the phone anyway. A text
//! message's notification also gets **Open in Messages**, which opens
//! `amane-messages` on that person's conversation.

use amane::{Notification, Service};
use amane_phone::{KdeConnect, PhoneNotification};

use crate::surfaces::quick_settings::tiles::phone_screen;

/// What the button says.
pub const LABEL: &str = "Open on Phone";

/// Whether `notification` came from KDE Connect.
fn from_kdeconnect(notification: &Notification) -> bool {
    notification.desktop_entry().contains("kdeconnect")
        || notification.app_name().eq_ignore_ascii_case("KDE Connect")
}

/// Whether the phone's notification is the one shown: its title is in the
/// summary or the body, and so is its text when it has one.
fn same(summary: &str, body: &str, phone: &PhoneNotification) -> bool {
    let shows = |part: &str| part.is_empty() || summary.contains(part) || body.contains(part);
    !phone.title.is_empty() && shows(&phone.title) && shows(&phone.text)
}

/// The phone's messaging apps: their notifications are text messages,
/// which Messages (`amane-messages`) shows here.
const MESSAGING: [&str; 2] = [
    "com.samsung.android.messaging",
    "com.google.android.apps.messaging",
];

/// What Open in Messages says.
pub const MESSAGES_LABEL: &str = "Open in Messages";

fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The buttons a phone's notification gets, as their labels and the
/// commands they run: Open in Messages for a text message, and Open on
/// Phone (its app in a window of its own) once the phone's screen can be
/// shown.
pub fn commands(notification: &Notification) -> Vec<(&'static str, String)> {
    if !from_kdeconnect(notification) {
        return Vec::new();
    }

    let kdeconnect = KdeConnect::read();
    for device in kdeconnect.paired() {
        if !device.connected() {
            continue;
        }
        let found = device.notifications().iter().find(|phone| {
            phone.package.is_some() && same(notification.summary(), notification.body(), phone)
        });
        let Some(found) = found else {
            continue;
        };
        let Some(package) = found.package.as_deref() else {
            continue;
        };

        let mut commands = Vec::new();
        if MESSAGING.contains(&package) && device.can_message() {
            commands.push((
                MESSAGES_LABEL,
                format!(
                    "{} --device {} --view messages --thread-of {}",
                    crate::surfaces::quick_settings::tiles::phone::MESSAGES_APP,
                    quoted(device.id()),
                    quoted(&found.title),
                ),
            ));
        }
        if phone_screen::set_up(device.id()) || phone_screen::ready(device) {
            commands.push((
                LABEL,
                format!(
                    "{} --device {} --app {} --name {}",
                    phone_screen::MIRROR_APP,
                    quoted(device.id()),
                    quoted(package),
                    quoted(&found.app),
                ),
            ));
        }
        return commands;
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone(title: &str, text: &str) -> PhoneNotification {
        PhoneNotification {
            id: "1".into(),
            app: "Signal".into(),
            package: Some("org.thoughtcrime.securesms".into()),
            title: title.into(),
            text: text.into(),
        }
    }

    #[test]
    fn matches_the_title_and_text_wherever_they_show() {
        assert!(same(
            "Signal: Ada",
            "See you at eight",
            &phone("Ada", "See you at eight")
        ));
        assert!(same("Ada", "", &phone("Ada", "")));
        assert!(!same(
            "Ada",
            "Something else",
            &phone("Ada", "See you at eight")
        ));
        assert!(!same("Signal", "", &phone("", "")));
    }
}
