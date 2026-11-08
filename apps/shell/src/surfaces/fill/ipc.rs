//! The passwords IPC (`passwords …`), and the answer to `passwords status`.

use super::*;

/// `emaki` IPC, for keybinds and Emaki Settings' Passwords page:
///
/// - `passwords suggest`: the popup for the field that has the keyboard, even a username or
///   terminal field;
/// - `passwords status`: the vault's state, one `key=value` a line, never a secret;
/// - `passwords unlock | sign-in | register | sync | lock`;
/// - `passwords configure <server> <email> [forget]`, `passwords lock-timeout <seconds>`.
///
/// Unlocking from here is still the shell's own rbw command, so its dialog may ask.
pub fn run(command: Command) -> String {
    match command {
        Command::Suggest => {
            let generation = FocusedField::read().generation();

            with(|state| {
                state.forced = Some(generation);
                state.dismissed = None;
            });
            FillChanged::changed();

            Vault::look_now();
        }
        Command::Status => return status(),
        Command::Unlock => Vault::unlock(),
        Command::SignIn => Vault::sign_in(),
        Command::Register => Vault::register(),
        Command::Sync => Vault::sync(),
        Command::Lock => Vault::lock(),
        Command::Configure {
            server,
            email,
            forget,
        } => {
            if !valid_server(&server) || !valid_email(&email) {
                return String::from(
                    "error: the server must be https:// (or http://localhost), and the email an address",
                );
            }

            Vault::configure(server, email, forget);
        }
        Command::LockTimeout(seconds) => Vault::set_lock_timeout(seconds),
    }

    String::from("ok")
}

/// A server rbw may talk to: https, or plain http only to this computer.
pub fn valid_server(server: &str) -> bool {
    let lower = server.to_ascii_lowercase();

    let rest = if let Some(rest) = lower.strip_prefix("https://") {
        rest
    } else if let Some(rest) = lower.strip_prefix("http://") {
        let authority = rest.split('/').next().unwrap_or("");
        let host = match authority.strip_prefix('[') {
            Some(inner) => inner.split(']').next().map(|host| format!("[{host}]")),
            None => authority.split(':').next().map(String::from),
        }
        .unwrap_or_default();
        if !matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") {
            return false;
        }
        rest
    } else {
        return false;
    };

    !rest.is_empty()
        && !server
            .chars()
            .any(|letter| letter.is_whitespace() || letter.is_control())
}

pub(super) fn valid_email(email: &str) -> bool {
    email.len() <= 254
        && email
            .split_once('@')
            .is_some_and(|(name, domain)| !name.is_empty() && domain.contains('.'))
        && !email
            .chars()
            .any(|letter| letter.is_whitespace() || letter.is_control())
}

// the vault's state for the Passwords page, without anything secret
pub(super) fn status() -> String {
    let vault = Vault::read();
    let field = FocusedField::read();

    let config = vault.config().cloned().unwrap_or_default();

    let mut lines = vec![
        format!("status={:?}", vault.status()),
        format!(
            "entries={}",
            vault
                .entries()
                .iter()
                .filter(|entry| entry.is_login())
                .count()
        ),
        format!(
            "busy={}",
            vault
                .busy()
                .map(|busy| format!("{busy:?}"))
                .unwrap_or_default()
        ),
        format!(
            "error={}",
            vault
                .error()
                .map(|error| error.describe())
                .unwrap_or_default()
        ),
        format!("server={}", config.base_url),
        format!("email={}", config.email),
        format!("lock_timeout={}", config.lock_timeout),
        format!("pinentry_ok={}", config.pinentry == vault::pinentry_path()),
        format!(
            "input_method={}",
            if !settings::passwords().fill_in_apps {
                "off"
            } else if field.available() {
                "on"
            } else {
                "unavailable"
            }
        ),
        format!("secure_fill={}", field.can_fill()),
    ];

    if let Some(synced) = vault
        .synced()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
    {
        lines.push(format!("synced={}", synced.as_secs()));
    }

    lines.join("\n")
}
