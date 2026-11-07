//! Passwords and keys an app keeps, in the Secret Service (GNOME Keyring,
//! KWallet) through `secret-tool`, under `application <app> account <name>`.
//! Where no keyring answers, they go in `~/.config/<app>/secrets.json`,
//! readable only by its owner, and [`kept_in_file`] says so for a page to
//! warn.
//!
//! [`lookup`] and [`store`] wait on `secret-tool`, so they're for a thread
//! of their own. [`known`] is for drawing: it answers from what was last
//! found, and looks again on a thread when it doesn't know yet.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex, PoisonError};

crate::marker! {
    /// A secret was found, kept or forgotten: what shows whether one is
    /// kept draws again.
    pub SecretsChanged
}

/// Where a secret went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// The desktop's keyring, through `secret-tool`.
    Keyring,
    /// The keyring didn't answer: the app's own file, 0600.
    File,
}

/// Whether each secret is kept, by (app, name), as last found.
static KNOWN: LazyLock<Mutex<HashMap<(String, String), bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn remember(app: &str, name: &str, kept: bool) {
    KNOWN
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert((app.to_string(), name.to_string()), kept);
    SecretsChanged::changed();
}

/// The secret `name` of `app`, from the keyring or else the app's file.
pub fn lookup(app: &str, name: &str) -> Option<String> {
    let found = Command::new("secret-tool")
        .args(["lookup", "application", app, "account", name])
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .filter(|secret| !secret.is_empty())
        .or_else(|| file_secrets(app).remove(name));
    remember(app, name, found.is_some());
    found
}

/// Keeps `secret` as `name` of `app`, labelled `label` in the keyring;
/// an empty one forgets it.
pub fn store(app: &str, name: &str, label: &str, secret: &str) -> Kept {
    if secret.is_empty() {
        forget(app, name);
        return Kept::Keyring;
    }
    let kept = Command::new("secret-tool")
        .args([
            "store",
            "--label",
            label,
            "application",
            app,
            "account",
            name,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .and_then(|mut child| {
            child.stdin.take()?.write_all(secret.as_bytes()).ok()?;
            child.wait().ok().filter(|status| status.success())
        })
        .is_some();
    let kept = if kept {
        // a copy left in the file from before would outlive this one
        let mut file = file_secrets(app);
        if file.remove(name).is_some() {
            write_file_secrets(app, &file);
        }
        Kept::Keyring
    } else {
        let mut file = file_secrets(app);
        file.insert(name.to_string(), secret.to_string());
        write_file_secrets(app, &file);
        Kept::File
    };
    remember(app, name, true);
    kept
}

/// Forgets `name` of `app`, from the keyring and the file.
pub fn forget(app: &str, name: &str) {
    if let Err(error) = Command::new("secret-tool")
        .args(["clear", "application", app, "account", name])
        .stderr(Stdio::null())
        .status()
    {
        log::warn!("secret-tool clear: {error}");
    }
    let mut file = file_secrets(app);
    if file.remove(name).is_some() {
        write_file_secrets(app, &file);
    }
    remember(app, name, false);
}

/// Whether `name` of `app` is kept, for drawing: what was last found, or
/// None while a look on a thread finds out.
pub fn known(app: &str, name: &str) -> Option<bool> {
    SecretsChanged::watch();
    let key = (app.to_string(), name.to_string());
    let mut known = KNOWN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(kept) = known.get(&key) {
        return Some(*kept);
    }
    // looked for once: false until found, so it isn't asked every frame
    known.insert(key, false);
    drop(known);
    let (app, name) = (app.to_string(), name.to_string());
    std::thread::spawn(move || {
        lookup(&app, &name);
    });
    None
}

/// Whether any of `app`'s secrets are in its file, the keyring having
/// turned them down.
pub fn kept_in_file(app: &str) -> bool {
    SecretsChanged::watch();
    !file_secrets(app).is_empty()
}

fn secrets_file(app: &str) -> Option<PathBuf> {
    let config = crate::paths::config();
    Some(config.join(app).join("secrets.json"))
}

fn file_secrets(app: &str) -> BTreeMap<String, String> {
    secrets_file(app)
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_file_secrets(app: &str, secrets: &BTreeMap<String, String>) {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(file) = secrets_file(app) else {
        return;
    };
    if secrets.is_empty() {
        // no secrets and no file is the same
        let _ = std::fs::remove_file(file);
        return;
    }
    let Ok(text) = serde_json::to_string_pretty(secrets) else {
        return;
    };
    let write = || {
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let partial = file.with_extension("part");
        // a part file left by a failed save; create_new below needs it gone
        let _ = std::fs::remove_file(&partial);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&partial)
            .and_then(|mut out| out.write_all(text.as_bytes()))?;
        std::fs::rename(partial, &file)
    };
    if let Err(e) = write() {
        log::warn!("couldn't save {}: {e}", file.display());
    }
}
