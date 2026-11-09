//! rbw, the Bitwarden client the vault runs: its commands by their store path, never through
//! PATH, with no secret in its arguments or environment, and with core dumps off for rbw and
//! the agent it may start. What it prints on failure is read for what went wrong and then
//! dropped, never logged, since it can hold an entry's name.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use serde_json::Value;
use zeroize::Zeroizing;

use super::secret::{self, Secret};

// set by the nix build to the store's rbw, so no PATH can swap it
const RBW: &str = match option_env!("EMAKI_RBW") {
    Some(path) => path,
    None => "/run/current-system/sw/bin/rbw",
};

/// The store package of the rbw the shell runs, whose rbw-agent is the only one trusted to
/// ask for the master password; none when rbw isn't in the store (a checkout's debug build).
pub fn package() -> Option<std::path::PathBuf> {
    let rbw = std::fs::canonicalize(RBW).ok()?;
    emaki_kit::peer::store_package(&rbw).map(std::path::Path::to_path_buf)
}

/// Where the vault is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    /// rbw isn't there, or has no email set.
    #[default]
    NotConfigured,
    /// Set up, but no copy of the vault yet: it needs a login.
    LoggedOut,
    Locked,
    Unlocked,
}

/// What went wrong, read from rbw's error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Missing,
    Locked,
    Cancelled,
    WrongPassword,
    NeedsLogin,
    NeedsRegister,
    Network,
    NotFound,
    Failed,
}

impl Error {
    pub fn describe(self) -> &'static str {
        match self {
            Error::Missing => "rbw isn't installed",
            Error::Locked => "The vault is locked",
            Error::Cancelled => "Cancelled",
            Error::WrongPassword => "The password was wrong",
            Error::NeedsLogin => "Sign in first",
            Error::NeedsRegister => "This device needs registering with the server",
            Error::Network => "The server couldn't be reached",
            Error::NotFound => "The entry is gone",
            Error::Failed => "rbw failed",
        }
    }

    // from what rbw printed; the text is only searched, never kept
    fn read(stderr: &[u8]) -> Self {
        let text = String::from_utf8_lossy(stderr).to_ascii_lowercase();

        if text.contains("pinentry cancelled") || text.contains("cancelled") {
            Error::Cancelled
        } else if text.contains("rbw register") || text.contains("register") {
            Error::NeedsRegister
        } else if text.contains("incorrect password") || text.contains("invalid password") {
            Error::WrongPassword
        } else if text.contains("not logged in") || text.contains("rbw login") {
            Error::NeedsLogin
        } else if text.contains("locked") {
            Error::Locked
        } else if text.contains("couldn't find") || text.contains("no entry found") {
            Error::NotFound
        } else if text.contains("error sending request")
            || text.contains("dns")
            || text.contains("connection")
            || text.contains("timed out")
        {
            Error::Network
        } else {
            Error::Failed
        }
    }
}

/// An entry as the list shows it: never its password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub user: Option<String>,
    pub folder: Option<String>,
    pub kind: String,
    pub uris: Vec<String>,
}

impl Entry {
    pub fn is_login(&self) -> bool {
        self.kind == "Login"
    }
}

/// rbw's settings that the vault shows, from its own config file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub email: String,
    pub base_url: String,
    pub lock_timeout: u64,
    pub pinentry: String,
}

// rbw names its folders after RBW_PROFILE, as rbw-<profile>
fn folder_name() -> String {
    match std::env::var("RBW_PROFILE") {
        Ok(profile) if !profile.is_empty() => format!("rbw-{profile}"),
        _ => String::from("rbw"),
    }
}

pub fn config() -> Option<Config> {
    let path = emaki_kit::paths::config()
        .join(folder_name())
        .join("config.json");

    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;

    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };

    Some(Config {
        email: text("email"),
        base_url: text("base_url"),
        lock_timeout: value
            .get("lock_timeout")
            .and_then(Value::as_u64)
            .unwrap_or(3600),
        pinentry: text("pinentry"),
    })
}

/*
 * whether rbw has a copy of the vault: its encrypted database, named after the server and
 * email, in the cache folder only this user can enter
 */
fn has_database(config: &Config) -> bool {
    let folder = emaki_kit::paths::cache();

    let server = if config.base_url.is_empty() {
        String::from("https://api.bitwarden.com")
    } else {
        config.base_url.clone()
    };

    let name = format!(
        "{}:{}.json",
        emaki_kit::percent::encode(&server),
        config.email
    );

    folder.join(folder_name()).join(name).is_file()
}

pub fn status() -> Status {
    let Some(config) = config().filter(|config| !config.email.is_empty()) else {
        return Status::NotConfigured;
    };

    if !std::path::Path::new(RBW).exists() {
        return Status::NotConfigured;
    }

    if !has_database(&config) {
        return Status::LoggedOut;
    }

    match run(&["unlocked"]) {
        Ok(_) => Status::Unlocked,
        Err(_) => Status::Locked,
    }
}

/// Whether the agent holds the vault unlocked now: asked before a secret is, so a lock since
/// the last look never brings up a password prompt the shell didn't start.
pub fn unlocked() -> bool {
    run(&["unlocked"]).is_ok()
}

/// Every entry, without a secret; only works unlocked.
pub fn list() -> Result<Vec<Entry>, Error> {
    let output = run(&["list", "--raw"])?;

    let value: Value = serde_json::from_slice(&output).map_err(|_| Error::Failed)?;

    let entries = value
        .as_array()
        .ok_or(Error::Failed)?
        .iter()
        .filter_map(|entry| {
            let text = |key: &str| entry.get(key).and_then(Value::as_str).map(String::from);

            Some(Entry {
                id: text("id").filter(|id| is_id(id))?,
                name: text("name").unwrap_or_default(),
                user: text("user").filter(|user| !user.is_empty()),
                folder: text("folder").filter(|folder| !folder.is_empty()),
                kind: text("type").unwrap_or_default(),
                uris: entry
                    .get("uris")
                    .and_then(Value::as_array)
                    .map(|uris| {
                        uris.iter()
                            .filter_map(|uri| match uri {
                                Value::String(uri) => Some(uri.clone()),
                                Value::Object(object) => {
                                    object.get("uri").and_then(Value::as_str).map(String::from)
                                }
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect();

    Ok(entries)
}

/// An entry's password.
pub fn password(id: &str) -> Result<Secret, Error> {
    secret_of(&["get", "--", id])
}

/// An entry's current two-factor code.
pub fn totp(id: &str) -> Result<Secret, Error> {
    secret_of(&["code", "--", id])
}

pub fn unlock() -> Result<(), Error> {
    run(&["unlock"]).map(drop)
}

pub fn login() -> Result<(), Error> {
    run(&["login"]).map(drop)
}

pub fn register() -> Result<(), Error> {
    run(&["register"]).map(drop)
}

pub fn sync() -> Result<(), Error> {
    run(&["sync"]).map(drop)
}

pub fn lock() -> Result<(), Error> {
    run(&["lock"]).map(drop)
}

pub fn stop_agent() -> Result<(), Error> {
    run(&["stop-agent"]).map(drop)
}

pub fn purge() -> Result<(), Error> {
    run(&["purge"]).map(drop)
}

/// Sets one of rbw's settings: only the ones the Passwords page has.
pub fn config_set(key: &str, value: &str) -> Result<(), Error> {
    if !matches!(key, "email" | "base_url" | "lock_timeout" | "pinentry") {
        return Err(Error::Failed);
    }

    // after `--`, so a value starting with `-` is never read as an option
    run(&["config", "set", "--", key, value]).map(drop)
}

/// A vault entry's id: a UUID, so an id can never be read as one of rbw's options.
pub fn is_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(at, byte)| match at {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(RBW);

    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // no core dump of rbw, or of the agent it starts, which holds the unlocked keys
    // SAFETY: setrlimit is async-signal-safe and touches nothing of the parent's
    unsafe {
        command.pre_exec(|| {
            let none = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            libc::setrlimit(libc::RLIMIT_CORE, &none);
            Ok(())
        });
    }

    command
}

fn run(args: &[&str]) -> Result<Zeroizing<Vec<u8>>, Error> {
    let output = command(args).output().map_err(|_| Error::Missing)?;

    // stdout may be a secret's neighbour; wiped either way
    let stdout = Zeroizing::new(output.stdout);

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(Error::read(&output.stderr))
    }
}

// a secret read straight into one buffer that wipes itself, never grown
fn secret_of(args: &[&str]) -> Result<Secret, Error> {
    if !args.last().is_some_and(|id| is_id(id)) {
        return Err(Error::NotFound);
    }

    let mut child = command(args).spawn().map_err(|_| Error::Missing)?;

    let mut buffer = Zeroizing::new(vec![0u8; secret::LIMIT + 1]);
    let mut filled = 0;

    if let Some(mut stdout) = child.stdout.take() {
        while filled < buffer.len() {
            match stdout.read(&mut buffer[filled..]) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(_) => break,
            }
        }
    }

    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        // what could be read is enough; the exit status decides
        let _ = pipe.read_to_end(&mut stderr);
    }

    let status = child.wait().map_err(|_| Error::Failed)?;

    if !status.success() {
        return Err(Error::read(&stderr));
    }

    if filled > secret::LIMIT {
        return Err(Error::Failed);
    }

    Secret::from_output(&buffer[..filled]).ok_or(Error::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_uuids() {
        assert!(is_id("24dba236-4578-4629-a552-d505008ea0e4"));
        assert!(!is_id("--raw"));
        assert!(!is_id("24dba236-4578-4629-a552-d505008ea0e"));
        assert!(!is_id("24dba236x4578-4629-a552-d505008ea0e4"));
    }

    #[test]
    fn database_names_match_rbws() {
        assert_eq!(
            emaki_kit::percent::encode("https://vault.tail8c36e9.ts.net"),
            "https%3A%2F%2Fvault.tail8c36e9.ts.net"
        );
    }

    #[test]
    fn errors_are_read_not_kept() {
        assert_eq!(
            Error::read(b"rbw unlock: failed to read password from pinentry: pinentry cancelled"),
            Error::Cancelled
        );
        assert_eq!(Error::read(b"rbw get: agent is locked"), Error::Locked);
        assert_eq!(Error::read(b"rbw login: failed to log in to bitwarden instance: unregistered device, run rbw register"), Error::NeedsRegister);
        assert_eq!(Error::read(b"something else"), Error::Failed);
    }

    #[test]
    fn a_secret_needs_a_real_id() {
        assert_eq!(password("--help").unwrap_err(), Error::NotFound);
    }
}
