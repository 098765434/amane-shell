//! A new Bitwarden server, asked for over the IPC socket, is confirmed in the shell's own
//! alert before rbw is pointed at it. Any program of the session can write to the socket, and
//! the server is where the next unlock sends the master password, so the shell, not the
//! asker, has the last word.
//!
//! The confirming button is not the default one: Return, typed into whatever had the keyboard
//! a moment before, can't answer it.

use crate::services::vault::Vault;
use crate::surfaces::alert;
use emaki_kit::dialog::{Dialog, Icon};
use emaki_kit::icons::ICON_LOCK;

const NAME: &str = "emaki.alert.vault.server";

/// Asks before pointing rbw at `server` as `email`.
pub fn ask(server: String, email: String, forget: bool) {
    alert::show(dialog(server, email, forget));
}

fn dialog(server: String, email: String, forget: bool) -> Dialog {
    let host = host(&server).to_string();

    Dialog::new(NAME, "Use This Bitwarden Server?")
        .icon(Icon::Symbol(ICON_LOCK))
        .message(format!(
            "Your master password will be checked by {host} when you next unlock."
        ))
        .note(format!("{server} · {email}"))
        .button("Cancel", || {})
        .destructive("Use Server", move || {
            Vault::configure(server.clone(), email.clone(), forget);
        })
}

/// The host of `server`, as the user should read it.
pub fn host(server: &str) -> &str {
    let rest = server.split_once("://").map_or(server, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    authority.rsplit('@').next().unwrap_or(authority)
}

#[cfg(test)]
mod tests {
    use super::host;

    #[test]
    fn host_is_the_authority_without_credentials() {
        assert_eq!(host("https://vault.example.com/"), "vault.example.com");
        assert_eq!(
            host("https://user@evil.example:8443/x"),
            "evil.example:8443"
        );
        assert_eq!(host("http://localhost:8080"), "localhost:8080");
    }
}
