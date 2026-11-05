//! **Open on Phone** on the phone's notifications. KDE Connect shows the
//! phone's notifications here as its own; the matching one in
//! `PhoneDevice::notifications()` carries the app's package (from
//! Android's notification key), and the button opens that app in a window
//! of its own (`amane-mirror --app`). It's offered once the phone's screen
//! was set up on the Phone page, or adb has the phone anyway.

use amane::{KdeConnect, Notification, PhoneNotification, Service};

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

/// The command that opens `notification`'s app on the phone, if it's one
/// of the phone's and its screen can be shown.
pub fn open_command(notification: &Notification) -> Option<String> {
    if !from_kdeconnect(notification) {
        return None;
    }

    let kdeconnect = KdeConnect::read();
    for device in kdeconnect.paired() {
        if !device.connected()
            || !(phone_screen::set_up(device.id()) || phone_screen::ready(device))
        {
            continue;
        }
        let found = device.notifications().iter().find(|phone| {
            phone.package.is_some() && same(notification.summary(), notification.body(), phone)
        });
        if let Some(found) = found {
            let package = found.package.as_deref()?;
            return Some(format!(
                "{} --device '{}' --app '{}' --name '{}'",
                phone_screen::MIRROR_APP,
                device.id(),
                package,
                found.app.replace('\'', r"'\''"),
            ));
        }
    }
    None
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
