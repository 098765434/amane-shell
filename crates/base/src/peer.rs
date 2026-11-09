//! Who is at the other end of a unix socket, as the kernel says: its user and process from
//! SO_PEERCRED, a pidfd from SO_PEERPIDFD so a recycled pid can't pass for it, and what the
//! process runs from /proc, down to the Nix store package it comes from. The shell and
//! emaki-pinentry check each other with these, and the daemons their clients.

use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

// Linux 6.5's SO_PEERPIDFD, which libc doesn't name yet
const SO_PEERPIDFD: libc::c_int = 77;

/// The process at the other end, held by a pidfd while it's looked at.
pub struct Peer {
    /// Its process id, from SO_PEERCRED.
    pub pid: u32,
    /// Its user id, from SO_PEERCRED.
    pub uid: u32,
    pidfd: Option<OwnedFd>,
}

impl Peer {
    /// The process at the other end of `stream`, with a pidfd for it when the kernel has
    /// SO_PEERPIDFD (6.5 and later). Fails when the socket has no peer process.
    pub fn of(stream: &UnixStream) -> io::Result<Self> {
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

        // SAFETY: getsockopt writes at most `length` bytes into the ucred it's given
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut credentials).cast(),
                &mut length,
            )
        };

        if result != 0 {
            return Err(io::Error::last_os_error());
        }

        if credentials.pid <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "peer has no process",
            ));
        }

        let mut pidfd: libc::c_int = -1;
        let mut length = std::mem::size_of::<libc::c_int>() as libc::socklen_t;

        // SAFETY: as above, into one int
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                SO_PEERPIDFD,
                (&raw mut pidfd).cast(),
                &mut length,
            )
        };

        // SAFETY: on success the kernel gave us a new fd that nothing else owns
        let pidfd = (result == 0 && pidfd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(pidfd) });

        Ok(Self {
            pid: credentials.pid as u32,
            uid: credentials.uid,
            pidfd,
        })
    }

    /// Whether the process checked is still the one that connected: its pidfd still refers to
    /// a live process, so its pid wasn't given to another in between. Call it after reading
    /// /proc. Without a pidfd (a kernel before 6.5) this can't be told and is false.
    pub fn still_there(&self) -> bool {
        let Some(pidfd) = &self.pidfd else {
            return false;
        };

        // SAFETY: signal 0 only checks; the fd is ours and open
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                pidfd.as_raw_fd(),
                0,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };

        result == 0
    }
}

/// What a process runs.
pub fn exe(pid: u32) -> Option<PathBuf> {
    fs::read_link(format!("/proc/{pid}/exe")).ok()
}

/// A process's parent.
pub fn parent(pid: u32) -> Option<u32> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;

    status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:"))
        .and_then(|ppid| ppid.trim().parse().ok())
        .filter(|ppid| *ppid > 0)
}

/// One variable of a process's environment, as it was started with.
pub fn environment(pid: u32, name: &str) -> Option<Option<String>> {
    let environ = fs::read(format!("/proc/{pid}/environ")).ok()?;

    let found = environ.split(|byte| *byte == 0).find_map(|pair| {
        let pair = std::str::from_utf8(pair).ok()?;
        let (key, value) = pair.split_once('=')?;

        (key == name).then(|| String::from(value))
    });

    Some(found)
}

/// Whether an executable is one of `names`, from the store package `package` (a
/// `/nix/store/<hash>-<name>` path, see [`store_package`]). Anyone can put a program of any
/// name in the Nix store with `nix build`, so being in the store proves nothing; being in the
/// one package this build was made with, or ships in, does. Without a package, only a debug
/// build trusts anything: its own target directory or the store, so the shell and pinentry
/// can be tried from a checkout.
pub fn is_program(exe: &Path, names: &[&str], package: Option<&Path>) -> bool {
    let Some(name) = exe.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    // nix's wrappers run the real program as .name-wrapped
    let bare = name
        .strip_prefix('.')
        .and_then(|name| name.strip_suffix("-wrapped"))
        .unwrap_or(name);

    if !names.contains(&bare) {
        return false;
    }

    match package {
        Some(package) => store_package(exe) == Some(package),
        // a checkout's debug build: its own target directory, or (as before) the store
        None => {
            cfg!(debug_assertions)
                && (exe.starts_with("/nix/store/")
                    || exe.components().any(|part| part.as_os_str() == "target"))
        }
    }
}

/// The store package a path is in, `/nix/store/<hash>-<name>`; none outside the store.
pub fn store_package(path: &Path) -> Option<&Path> {
    let rest = path.strip_prefix("/nix/store/").ok()?;
    let first = rest.components().next()?.as_os_str().to_str()?;

    // a store path's name is a 32-letter hash, a dash, then the package's name
    let (hash, name) = first.split_at_checked(32)?;
    if !name.starts_with('-') || !hash.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }

    let length = "/nix/store/".len() + first.len();
    Some(Path::new(path.to_str()?.get(..length)?))
}

/// The store package this process runs from, which the shell and emaki-pinentry share: the
/// other end must come from the very same build.
pub fn own_package() -> Option<PathBuf> {
    let exe = fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    store_package(&exe).map(Path::to_path_buf)
}

/// This process's user id.
pub fn current_uid() -> u32 {
    // SAFETY: getuid can't fail
    unsafe { libc::getuid() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_are_known_by_name_and_package() {
        let package = Path::new("/nix/store/0123456789abcdfghijklmnpqrsvwxyz-rbw-1.15.0");
        let other = Path::new("/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-rbw-1.15.0");
        let at = |package: &Path, name: &str| package.join("bin").join(name);

        assert!(is_program(
            &at(package, "rbw-agent"),
            &["rbw-agent"],
            Some(package)
        ));
        assert!(is_program(
            &at(package, ".emaki-wrapped"),
            &["emaki"],
            Some(package)
        ));
        assert!(!is_program(
            &at(package, "rbw-agent-evil"),
            &["rbw-agent"],
            Some(package)
        ));

        // a program of the same name that someone else built
        assert!(!is_program(
            &at(other, "rbw-agent"),
            &["rbw-agent"],
            Some(package)
        ));

        assert!(!is_program(
            Path::new("/home/me/bin/rbw-agent"),
            &["rbw-agent"],
            Some(package)
        ));
        assert!(!is_program(
            Path::new("/tmp/rbw-agent"),
            &["rbw-agent"],
            Some(package)
        ));
        assert_eq!(
            is_program(&at(package, "rbw-agent"), &["rbw-agent"], None),
            cfg!(debug_assertions)
        );
        assert!(!is_program(
            Path::new("/tmp/rbw-agent"),
            &["rbw-agent"],
            None
        ));
    }

    #[test]
    fn store_packages_are_the_hash_and_name() {
        let exe = Path::new("/nix/store/0123456789abcdfghijklmnpqrsvwxyz-emaki-0.1.0/bin/emaki");

        assert_eq!(
            store_package(exe),
            Some(Path::new(
                "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-emaki-0.1.0"
            ))
        );
        assert_eq!(
            store_package(Path::new("/nix/store/short-emaki/bin/emaki")),
            None
        );
        assert_eq!(store_package(Path::new("/usr/bin/emaki")), None);
    }

    #[test]
    fn our_own_process_is_readable() {
        let me = std::process::id();

        assert!(exe(me).is_some());
        assert!(parent(me).is_some());
        assert!(
            environment(me, "EMAKI_SURELY_UNSET_VARIABLE").is_some_and(|value| value.is_none())
        );
    }

    #[test]
    fn a_socket_pair_knows_its_peer() {
        let (one, _two) = UnixStream::pair().unwrap();

        let peer = Peer::of(&one).unwrap();

        assert_eq!(peer.pid, std::process::id());
        assert_eq!(peer.uid, current_uid());
        assert!(peer.still_there());
    }
}
