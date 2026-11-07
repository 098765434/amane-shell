//! Who is at the other end of a unix socket, as the kernel says: its user and process from
//! SO_PEERCRED, a pidfd from SO_PEERPIDFD so a recycled pid can't pass for it, and what the
//! process runs from /proc. Both ends check each other with these.

use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

// Linux 6.5's SO_PEERPIDFD, which libc doesn't name yet
const SO_PEERPIDFD: libc::c_int = 77;

/// The socket the shell answers on: in the runtime directory, which only this user can enter.
pub fn socket_path() -> Option<PathBuf> {
    let runtime = amane_base::paths::private_runtime()?;
    Some(
        runtime
            .join(amane_base::app_ids::SHELL)
            .join("pinentry.sock"),
    )
}

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

/// Whether an executable is one of `names`, where only root can put files: the Nix store. A
/// program run from the system profile counts, since `exe` is the resolved path, which is in the
/// store. A debug build also takes its own target directory, so the shell and pinentry can be
/// tried from a checkout.
pub fn is_program(exe: &Path, names: &[&str]) -> bool {
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

    if exe.starts_with("/nix/store/") {
        return true;
    }

    cfg!(debug_assertions) && exe.components().any(|part| part.as_os_str() == "target")
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
    fn programs_are_known_by_name_and_place() {
        let store = |name: &str| PathBuf::from(format!("/nix/store/abc-rbw-1.15.0/bin/{name}"));

        assert!(is_program(&store("rbw-agent"), &["rbw-agent"]));
        assert!(is_program(&store(".amane-shell-wrapped"), &["amane-shell"]));
        assert!(!is_program(&store("rbw-agent-evil"), &["rbw-agent"]));
        assert!(!is_program(
            Path::new("/home/me/bin/rbw-agent"),
            &["rbw-agent"]
        ));
        assert!(!is_program(Path::new("/tmp/rbw-agent"), &["rbw-agent"]));
    }

    #[test]
    fn our_own_process_is_readable() {
        let me = std::process::id();

        assert!(exe(me).is_some());
        assert!(parent(me).is_some());
        assert!(
            environment(me, "AMANE_SURELY_UNSET_VARIABLE").is_some_and(|value| value.is_none())
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
