//! `passwords.json`: the vault, which apps it fills, and the browsers it knows.

use super::*;

/// An app the user filled from a vault entry and chose to remember, known by what the
/// compositor said runs it, never by the app id alone, which any app can claim.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", default)]
pub struct AppLink {
    /// The desktop entry the app's package provides, like "org.telegram.desktop".
    pub desktop_id: String,

    /// The package name of the program that ran, from its Nix store path, like
    /// "telegram-desktop".
    pub package: String,

    /// The vault entry, by its id, and its name then, for the settings page.
    pub entry_id: String,
    /// The vault entry's name.
    pub entry_name: String,
}

/// Bitwarden fill: where it offers itself, and the apps it remembers. The server, the
/// email and the auto-lock time are rbw's own settings, kept in rbw's config.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(crate = "amane::serde", default)]
pub struct Passwords {
    /// Offer to fill password fields in other apps (the shell as the input method).
    pub fill_in_apps: bool,

    /// Also offer on username and email fields, where an app has a remembered or
    /// matching entry.
    pub suggest_on_usernames: bool,

    /// Passwords in the launcher.
    #[serde(alias = "show_in_spotlight")] // lint-terms: allow (the old key, still read)
    pub show_in_launcher: bool,

    /// No popup while the screen is being shared or recorded.
    pub hide_while_sharing: bool,

    /// Lock the vault when the screen locks or the computer sleeps.
    pub lock_with_screen: bool,

    /// Apps never offered fill, by app id; browsers by default, whose Bitwarden extension
    /// checks the website itself.
    pub excluded_apps: Vec<String>,

    /// Apps tied to vault entries, for filling them.
    pub app_links: Vec<AppLink>,
}

impl Default for Passwords {
    fn default() -> Self {
        Self {
            fill_in_apps: true,
            suggest_on_usernames: false,
            show_in_launcher: true,
            hide_while_sharing: true,
            lock_with_screen: true,
            excluded_apps: BROWSERS.map(String::from).to_vec(),
            app_links: Vec::new(),
        }
    }
}

/// Browsers by their app ids: a page decides their window title, so fill never trusts
/// them; the Bitwarden extension knows the site.
pub const BROWSERS: [&str; 30] = [
    "firefox",
    "org.mozilla.firefox",
    "firefox-devedition",
    "firefox-nightly",
    "firefox-beta",
    "floorp",
    "one.ablaze.floorp",
    "thorium-browser",
    "mullvad-browser",
    "torbrowser",
    "org.torproject.torbrowser-launcher",
    "org.qutebrowser.qutebrowser",
    "org.kde.falkon",
    "org.kde.konqueror",
    "org.gnome.Epiphany",
    "Ladybird",
    "librewolf",
    "io.gitlab.librewolf-community",
    "zen",
    "zen-browser",
    "chromium",
    "chromium-browser",
    "org.chromium.Chromium",
    "google-chrome",
    "com.google.Chrome",
    "brave-browser",
    "com.brave.Browser",
    "vivaldi-stable",
    "microsoft-edge",
    "epiphany",
];

impl Passwords {
    /// Whether fill is never offered in the app with this id.
    pub fn excludes(&self, app_id: &str) -> bool {
        self.excluded_apps
            .iter()
            .any(|excluded| excluded.eq_ignore_ascii_case(app_id))
    }

    /// The entries remembered for an app, by its desktop entry and package.
    pub fn links_for<'a>(
        &'a self,
        desktop_id: &'a str,
        package: &'a str,
    ) -> impl Iterator<Item = &'a AppLink> + 'a {
        self.app_links
            .iter()
            .filter(move |link| link.desktop_id == desktop_id && link.package == package)
    }

    /// Remembers an entry for an app, once.
    pub fn remember(&mut self, link: AppLink) {
        let known = self.app_links.iter().any(|known| {
            known.desktop_id == link.desktop_id
                && known.package == link.package
                && known.entry_id == link.entry_id
        });

        if !known {
            self.app_links.push(link);
        }
    }
}

// which apps have which accounts is private
settings_file!(
    Passwords,
    crate::app_ids::SHELL,
    "passwords",
    passwords,
    change_passwords;
    const PRIVATE: bool = true;
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launcher_switch_reads_its_old_key_too() {
        let old: Passwords = serde_json::from_str(r#"{"show_in_spotlight": false}"#) // lint-terms: allow
            .expect("an old passwords.json");
        assert!(!old.show_in_launcher);

        let new: Passwords =
            serde_json::from_str(r#"{"show_in_launcher": false}"#).expect("passwords.json");
        assert!(!new.show_in_launcher);
    }
}
