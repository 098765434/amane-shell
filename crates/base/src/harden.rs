//! What a program holding secrets or reading untrusted data does to keep itself in: no core
//! dump of it (a crash would otherwise write its memory to disk); for one no other program needs
//! to look at, no other process of the user's reading its memory either; and its file access
//! confined to what it needs ([`confine`], with Landlock), so a bug in a decoder can't read or
//! change the rest of the user's files.

use std::path::Path;

/// No core dumps of this process or of what it starts: `RLIMIT_CORE` 0, which a child keeps.
pub fn no_core_dumps() {
    let none = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: setrlimit only reads the struct it's given
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &none) } != 0 {
        log::warn!("core dumps left on: {}", std::io::Error::last_os_error());
    }
}

/// [`no_core_dumps`], and the process not dumpable: no other process of the same user may
/// attach to it (ptrace) or read its memory (`/proc/<pid>/mem`). That also hides its
/// `/proc/<pid>/exe` and `environ`, so only for a program no other is meant to verify
/// that way: not the shell, emaki-pinentry or Drop, which check each other through those.
pub fn private_memory() {
    no_core_dumps();
    // SAFETY: PR_SET_DUMPABLE takes a plain integer
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        log::warn!("memory left readable: {}", std::io::Error::last_os_error());
    }
}

/// Confines the calling thread, and every thread and program it starts afterwards, to reading
/// (and running) what's under `readable` and reading and writing what's under `writable`; any
/// other file or folder can't be opened at all. Files already open stay usable. A path that
/// doesn't exist is left out. Best effort: a kernel without Landlock (before 5.13, or with it
/// off) leaves the thread as it was, and that is logged.
///
/// It also sets no-new-privileges, which Landlock needs: nothing started from the thread can
/// gain privileges through setuid (no sudo, no pkexec), so only for threads that start no such
/// program.
pub fn confine(readable: &[&Path], writable: &[&Path]) {
    use landlock::{
        ABI, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
        path_beneath_rules,
    };

    let abi = ABI::V5;

    let confined = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .and_then(|ruleset| ruleset.create())
        .and_then(|ruleset| {
            ruleset.add_rules(path_beneath_rules(readable, AccessFs::from_read(abi)))
        })
        .and_then(|ruleset| {
            ruleset.add_rules(path_beneath_rules(writable, AccessFs::from_all(abi)))
        })
        .and_then(|ruleset| ruleset.restrict_self());

    match confined {
        Ok(status) if status.ruleset == RulesetStatus::FullyEnforced => {}
        Ok(status) => log::warn!(
            "file access only partly confined ({:?}): the kernel's Landlock is older or off",
            status.ruleset
        ),
        Err(error) => log::warn!("file access not confined: {error}"),
    }
}

/// Whether this process may be dumped and attached to, as the kernel says.
pub fn dumpable() -> bool {
    // SAFETY: PR_GET_DUMPABLE takes no arguments
    unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) == 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_confined_thread_reads_only_what_it_was_given() {
        let folder = std::env::temp_dir().join(format!("emaki-confine-{}", std::process::id()));
        let allowed = folder.join("allowed");
        let other = folder.join("other");
        std::fs::create_dir_all(&allowed).expect("a folder");
        std::fs::create_dir_all(&other).expect("a folder");
        std::fs::write(other.join("secret"), "x").expect("a file");

        // on a thread of its own: confinement is the thread's and its children's
        let (readable, written, other_read) = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    confine(&[], &[&allowed]);
                    (
                        std::fs::read_dir(&allowed).is_ok(),
                        std::fs::write(allowed.join("file"), "y").is_ok(),
                        std::fs::read(other.join("secret")).is_ok(),
                    )
                })
                .join()
                .expect("the thread")
        });
        // this thread isn't confined
        let still_open = std::fs::read(other.join("secret")).is_ok();
        let _ = std::fs::remove_dir_all(&folder);

        assert!(readable && written && still_open);
        // only where Landlock is on is the rest closed
        if landlock_on() {
            assert!(
                !other_read,
                "a confined thread read outside what it was given"
            );
        }
    }

    fn landlock_on() -> bool {
        std::fs::read_to_string("/sys/kernel/security/lsm")
            .is_ok_and(|lsm| lsm.contains("landlock"))
    }

    #[test]
    fn core_dumps_go_off() {
        no_core_dumps();
        let mut limit = libc::rlimit {
            rlim_cur: 1,
            rlim_max: 1,
        };
        // SAFETY: getrlimit writes the struct it's given
        assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) }, 0);
        assert_eq!((limit.rlim_cur, limit.rlim_max), (0, 0));
    }
}
