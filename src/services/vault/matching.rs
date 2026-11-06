//! Which vault entries go with a verified app, best first. Only a verified app gets any:
//!
//! 1. **Remembered:** the entries the user filled into this app before, kept by its desktop
//!    entry and package.
//! 2. **Named:** an entry with the URI `app://<desktop id>`, or `androidapp://<app id>` (what
//!    Bitwarden's phone apps save, often the same reverse-DNS name).
//! 3. **Same site:** a reverse-DNS desktop id read as a domain (org.telegram.desktop is
//!    telegram.org) and an entry's website on that domain. Shown, never the first row.
//!
//! Window titles and app names are never matched: an app or a web page sets those itself.

use amane_kit::settings::Passwords;

use super::identity::Verified;
use super::rbw::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    Remembered,
    Named,
    SameSite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub entry: Entry,
    pub reason: Reason,
}

/// The entries for an app, best first, each once.
pub fn matches(app: &Verified, app_id: &str, entries: &[Entry], settings: &Passwords) -> Vec<Match> {
    let mut found: Vec<Match> = Vec::new();

    let mut add = |entry: &Entry, reason: Reason| {
        if !entry.is_login() || found.iter().any(|known| known.entry.id == entry.id) {
            return;
        }

        found.push(Match {
            entry: entry.clone(),
            reason,
        });
    };

    for link in settings.links_for(&app.desktop_id, &app.package) {
        if let Some(entry) = entries.iter().find(|entry| entry.id == link.entry_id) {
            add(entry, Reason::Remembered);
        }
    }

    let named = [
        format!("app://{}", app.desktop_id.to_ascii_lowercase()),
        format!("androidapp://{}", app_id.to_ascii_lowercase()),
        format!("androidapp://{}", app.desktop_id.to_ascii_lowercase()),
    ];
    for entry in entries {
        if entry
            .uris
            .iter()
            .any(|uri| named.contains(&uri.trim().trim_end_matches('/').to_ascii_lowercase()))
        {
            add(entry, Reason::Named);
        }
    }

    if let Some(domain) = domain_of(&app.desktop_id) {
        for entry in entries {
            if entry
                .uris
                .iter()
                .filter_map(|uri| host_of(uri))
                .any(|host| on_domain(&host, &domain))
            {
                add(entry, Reason::SameSite);
            }
        }
    }

    found
}

/// A reverse-DNS app id read as a domain: org.telegram.desktop is telegram.org. Needs at
/// least two parts, the first a top-level domain people use.
pub fn domain_of(desktop_id: &str) -> Option<String> {
    let parts: Vec<&str> = desktop_id.split('.').collect();

    if parts.len() < 3 {
        return None;
    }

    let top = parts[0].to_ascii_lowercase();
    if !matches!(
        top.as_str(),
        "org" | "com" | "net" | "io" | "de" | "no" | "uk" | "app" | "dev" | "me" | "eu"
    ) {
        return None;
    }

    let name = parts[1].to_ascii_lowercase();
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') {
        return None;
    }

    // github.io, gnome.org and the like host many projects: an app under them isn't their site
    if matches!(
        name.as_str(),
        "github" | "gitlab" | "codeberg" | "sourceforge" | "freedesktop" | "gnome" | "kde"
            | "flathub" | "mozilla" | "nixos" | "sr" | "srht" | "launchpad" | "googlecode"
    ) {
        return None;
    }

    Some(format!("{name}.{top}"))
}

/// An entry URI's host, lower case: a website's, never an app's.
pub fn host_of(uri: &str) -> Option<String> {
    let uri = uri.trim();

    let rest = match uri.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http") => rest,
        Some(_) => return None,
        // Bitwarden takes a bare host as a website
        None => uri,
    };

    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = host.split(':').next()?.trim_end_matches('.').to_ascii_lowercase();

    (!host.is_empty() && host.contains('.')).then_some(host)
}

/// A host on a domain: the domain itself or a part of it.
pub fn on_domain(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

#[cfg(test)]
mod tests {
    use amane_kit::settings::AppLink;

    use super::*;

    fn entry(id: &str, name: &str, uris: &[&str]) -> Entry {
        Entry {
            id: id.to_string(),
            name: name.to_string(),
            user: Some(String::from("me")),
            folder: None,
            kind: String::from("Login"),
            uris: uris.iter().map(|uri| uri.to_string()).collect(),
        }
    }

    fn telegram() -> Verified {
        Verified {
            desktop_id: String::from("org.telegram.desktop"),
            package: String::from("telegram-desktop"),
            name: String::from("Telegram"),
            browser: false,
        }
    }

    #[test]
    fn domains_from_reverse_dns() {
        assert_eq!(domain_of("org.telegram.desktop").as_deref(), Some("telegram.org"));
        assert_eq!(domain_of("com.discordapp.Discord").as_deref(), Some("discordapp.com"));
        assert_eq!(domain_of("firefox"), None);
        assert_eq!(domain_of("io.github.someone.App"), None);
        assert_eq!(domain_of("org.gnome.Calendar"), None);
        assert_eq!(domain_of("org.kde.konsole"), None);
        assert_eq!(domain_of("org.gnome.Epiphany"), None);
        assert_eq!(domain_of("xyz.evil.app"), None);
    }

    #[test]
    fn hosts_of_websites_only() {
        assert_eq!(host_of("https://web.telegram.org/k/").as_deref(), Some("web.telegram.org"));
        assert_eq!(host_of("telegram.org").as_deref(), Some("telegram.org"));
        assert_eq!(host_of("https://user@telegram.org:443").as_deref(), Some("telegram.org"));
        assert_eq!(host_of("androidapp://org.telegram.messenger"), None);
        assert_eq!(host_of("moz-extension://abc"), None);
        assert!(on_domain("web.telegram.org", "telegram.org"));
        assert!(!on_domain("eviltelegram.org", "telegram.org"));
        assert!(!on_domain("telegram.org.evil.com", "telegram.org"));
    }

    #[test]
    fn remembered_first_then_named_then_same_site() {
        let entries = vec![
            entry("site", "Telegram web", &["https://web.telegram.org"]),
            entry("named", "Telegram app", &["app://org.telegram.desktop"]),
            entry("kept", "Telegram", &[]),
            entry("other", "Bank", &["https://bank.example"]),
        ];

        let mut settings = Passwords::default();
        settings.remember(AppLink {
            desktop_id: String::from("org.telegram.desktop"),
            package: String::from("telegram-desktop"),
            entry_id: String::from("kept"),
            entry_name: String::from("Telegram"),
        });

        let found = matches(&telegram(), "org.telegram.desktop", &entries, &settings);
        let ids: Vec<&str> = found.iter().map(|found| found.entry.id.as_str()).collect();

        assert_eq!(ids, ["kept", "named", "site"]);
        assert_eq!(found[2].reason, Reason::SameSite);
    }

    #[test]
    fn a_link_for_another_package_is_not_used() {
        let entries = vec![entry("kept", "Telegram", &[])];

        let mut settings = Passwords::default();
        settings.remember(AppLink {
            desktop_id: String::from("org.telegram.desktop"),
            package: String::from("evil-telegram"),
            entry_id: String::from("kept"),
            entry_name: String::from("Telegram"),
        });

        assert!(matches(&telegram(), "org.telegram.desktop", &entries, &settings).is_empty());
    }
}
