//! Bitwarden, through rbw: whether the vault is locked, its entries without their secrets, and
//! the commands Emaki Settings and the fill popup run. See `docs/user/features/passwords.md`.
//!
//! - [`rbw`]: the rbw commands, by store path, with no secret in an argument.
//! - [`secret`]: a password or code on its way into a field, wiped when dropped.
//! - [`pinentry`]: the shell's end of emaki-pinentry, the master password in the shell's
//!   own dialog, only for an unlock the shell started.
//! - [`identity`] and [`matching`]: which app owns a field, checked, and its entries.
//!
//! The entries are kept only while unlocked, in memory, and never with a password; a
//! password is fetched from rbw when it's filled, and dropped right after.

pub mod deliver;
pub mod identity;
pub mod matching;
pub mod pinentry;
pub mod rbw;
pub mod secret;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use amane::{FieldOwner, Service};

pub use identity::Identity;
pub use rbw::{Entry, Error, Status};

// how soon after the last look a view's look_soon() looks again
use emaki_kit::sync::Locked as _;
const LOOK_AGAIN: Duration = Duration::from_secs(5);

// past the agent's lock timeout, so the vault has locked by itself when it's looked at
const LOCK_SLACK: Duration = Duration::from_secs(5);

/// What the vault is busy with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    Unlocking,
    SigningIn,
    Registering,
    Syncing,
    Locking,
    Saving,
}

#[derive(Default)]
pub struct Vault {
    status: Status,
    entries: Arc<Vec<Entry>>,
    busy: Option<Busy>,
    error: Option<Error>,
    synced: Option<SystemTime>,
    config: Option<rbw::Config>,
}

// a nudge for the watcher to look again at once
static LOOK: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

// when the watcher last looked
static LOOKED: Mutex<Option<Instant>> = Mutex::new(None);

impl Service for Vault {
    fn new() -> Self {
        Self::default()
    }

    /*
     * rbw says nothing when the vault locks or unlocks, so it's looked at when asked (a fill, the
     * launcher's passwords, a command here) and, while unlocked, once the agent's lock timeout
     * could have locked it; never on a clock of its own
     */
    fn listen() {
        watch_lock();

        loop {
            look();
            *LOOKED.locked() = Some(Instant::now());

            let timeout = {
                let vault = Vault::read();
                (vault.status == Status::Unlocked).then(|| {
                    let lock = vault.config.as_ref().map_or(3600, |c| c.lock_timeout);
                    Duration::from_secs(lock) + LOCK_SLACK
                })
            };

            let (asked, nudge) = &LOOK;
            let asked = asked.locked();
            let mut asked = match timeout {
                Some(timeout) => {
                    nudge
                        .wait_timeout_while(asked, timeout, |asked| !*asked)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                None => nudge
                    .wait_while(asked, |asked| !*asked)
                    .unwrap_or_else(PoisonError::into_inner),
            };
            *asked = false;
        }
    }
}

impl Vault {
    pub fn status(&self) -> Status {
        self.status
    }

    pub fn entries(&self) -> Arc<Vec<Entry>> {
        self.entries.clone()
    }

    pub fn busy(&self) -> Option<Busy> {
        self.busy
    }

    /// What went wrong with the last command, until the next one.
    pub fn error(&self) -> Option<Error> {
        self.error
    }

    /// When the entries were last read from an unlocked vault.
    pub fn synced(&self) -> Option<SystemTime> {
        self.synced
    }

    /// rbw's settings: the server, the email and the auto-lock time.
    pub fn config(&self) -> Option<&rbw::Config> {
        self.config.as_ref()
    }

    /// Looks at the vault again unless it was looked at a moment ago, for a view about to show
    /// its entries, like the launcher's passwords.
    pub fn look_soon() {
        let recent = LOOKED.locked().is_some_and(|at| at.elapsed() < LOOK_AGAIN);
        if !recent {
            Self::look_now();
        }
    }

    /// Looks at the vault again now, as after the popup opens.
    pub fn look_now() {
        let (asked, nudge) = &LOOK;
        *asked.locked() = true;
        nudge.notify_one();
    }

    /// Unlocks: the shell's dialog asks for the master password.
    pub fn unlock() {
        command(Busy::Unlocking, true, rbw::unlock);
    }

    /// Signs in to the server set, then unlocks.
    pub fn sign_in() {
        command(Busy::SigningIn, true, rbw::login);
    }

    /// Registers this device with the server, which bitwarden.com asks for; the dialog asks
    /// for the account's API key.
    pub fn register() {
        command(Busy::Registering, true, rbw::register);
    }

    /// Fetches the vault from the server again.
    pub fn sync() {
        command(Busy::Syncing, true, rbw::sync);
    }

    pub fn lock() {
        command(Busy::Locking, false, rbw::lock);
    }

    /*
     * sets the server and email; a change of either stops the agent, and with `forget` also
     * removes the old server's copy of the vault
     */
    pub fn configure(server: String, email: String, forget: bool) {
        command(Busy::Saving, false, move || {
            let current = rbw::config().unwrap_or_default();

            let changed = current.base_url != server || current.email != email;

            if changed {
                rbw::stop_agent().ok();

                if forget {
                    rbw::purge().ok();
                }
            }

            rbw::config_set("base_url", &server)?;
            rbw::config_set("email", &email)?;

            // the shell's pinentry, from the system profile, so a rebuild doesn't break it
            rbw::config_set("pinentry", &pinentry_path())
        });
    }

    /// How long the vault stays unlocked while unused, in seconds.
    pub fn set_lock_timeout(seconds: u64) {
        let seconds = seconds.clamp(60, 3600);

        command(Busy::Saving, false, move || {
            rbw::config_set("lock_timeout", &seconds.to_string())?;

            // the agent reads it when it starts
            rbw::stop_agent()
        });
    }
}

/*
 * the pinentry rbw is told to run: the one in the profile the shell is installed in (home
 * manager's per-user profile, or the system's), a path that stays when the shell is rebuilt.
 * A shell run from a checkout uses the pinentry built beside it
 */
pub fn pinentry_path() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();

    let profiles = [
        format!("/etc/profiles/per-user/{user}/bin/emaki-pinentry"),
        format!("{home}/.nix-profile/bin/emaki-pinentry"),
        String::from("/run/current-system/sw/bin/emaki-pinentry"),
    ];

    if let Some(found) = profiles
        .into_iter()
        .find(|path| !user.is_empty() && std::path::Path::new(path).exists())
    {
        return found;
    }

    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join("emaki-pinentry")))
        .filter(|path| path.exists())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("emaki-pinentry"))
}

// a command on a thread of its own, then a fresh look
fn command(busy: Busy, asks: bool, run: impl FnOnce() -> Result<(), Error> + Send + 'static) {
    {
        let mut vault = Vault::write();

        if vault.busy.is_some() {
            return;
        }

        vault.busy = Some(busy);
        vault.error = None;
    }

    thread::spawn(move || {
        if asks {
            pinentry::expect();
        }

        let result = run();

        // the dialog may come up for this command only, not for what asks after it
        if asks {
            pinentry::done_expecting();
        }

        {
            let mut vault = Vault::write();
            vault.busy = None;
            vault.error = result.err();
        }

        look();
    });
}

// reads the vault's state, and its entries once it's unlocked
fn look() {
    let status = rbw::status();
    let config = rbw::config();

    let (was, had) = {
        let vault = Vault::read();
        (vault.status, !vault.entries.is_empty())
    };

    let entries = match status {
        Status::Unlocked if was != Status::Unlocked || !had => rbw::list().ok().map(Arc::new),
        Status::Unlocked => None,
        _ => Some(Arc::new(Vec::new())),
    };

    let mut vault = Vault::write();

    let changed = vault.status != status || vault.config != config || entries.is_some();

    vault.status = status;
    vault.config = config;

    if let Some(entries) = entries {
        if status == Status::Unlocked {
            vault.synced = Some(SystemTime::now());
        }

        vault.entries = entries;
    }

    if !changed {
        vault.quiet();
    }
}

/// Locks the vault now, as when the screen locks.
pub fn lock_now() {
    if rbw::lock().is_ok() {
        look();
    }
}

// the vault locks with the screen and before the computer sleeps, when asked to
fn watch_lock() {
    let lock = || {
        if wants_lock() {
            // rbw takes a moment, and logind's thread has others to tell
            thread::spawn(lock_now);
        }
    };
    crate::services::power::logind::follow(lock, lock);
}

fn wants_lock() -> bool {
    emaki_kit::settings::passwords().lock_with_screen
}

/// Which app owns each field seen, once checked; the check runs on a thread, as nix-store
/// can take a moment the first time.
#[derive(Default)]
pub struct Identities {
    known: HashMap<IdentityKey, (Identity, std::time::Instant)>,
    checking: Vec<IdentityKey>,
}

// how long a check is believed: an app installed, removed or shadowed since is seen again
const IDENTITY_FOR: Duration = Duration::from_secs(600);

// the most owners kept, and checked at once
const IDENTITIES: usize = 256;
const CHECKING: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentityKey {
    exe: Option<PathBuf>,
    app_id: String,
    sandboxed: bool,
}

impl Service for Identities {
    fn new() -> Self {
        Self::default()
    }

    fn listen() {}
}

impl Identities {
    /// The owner's identity, once checked; asking starts the check.
    pub fn of(owner: &FieldOwner) -> Option<Identity> {
        let key = IdentityKey {
            exe: owner.exe.clone(),
            app_id: owner.app_id.clone(),
            sandboxed: owner.sandboxed,
        };

        {
            let identities = Identities::read();

            if let Some((identity, at)) = identities.known.get(&key)
                && at.elapsed() < IDENTITY_FOR
            {
                return Some(identity.clone());
            }

            // an app changing its id over and over gets no thread for each
            if identities.checking.contains(&key) || identities.checking.len() >= CHECKING {
                return None;
            }
        }

        Identities::write().checking.push(key.clone());

        let owner = owner.clone();
        thread::spawn(move || {
            let identity = identity::check(&owner);

            let mut identities = Identities::write();
            identities.checking.retain(|checking| *checking != key);

            if identities.known.len() >= IDENTITIES {
                identities
                    .known
                    .retain(|_, (_, at)| at.elapsed() < IDENTITY_FOR);
            }
            if identities.known.len() >= IDENTITIES {
                identities.known.clear();
            }

            identities
                .known
                .insert(key, (identity, std::time::Instant::now()));
        });

        None
    }
}
