//! Album art for what's playing: the file a player names, or a copy of the
//! art it links to on the web, fetched once into
//! `$XDG_CACHE_HOME/emaki/art/`. Quick Settings' Now Playing card and
//! the bar's Now Playing activity both show it.
//!
//! The URL comes from whatever is playing, a web page among them, so a fetch
//! goes only to a public address (never this computer or the local network),
//! checked before and pinned for the fetch, with no redirects, http(s) only,
//! and at most 10 MB.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

emaki_kit::marker! {
    /// Art fetched from the web landed in the cache: what showed none draws
    /// again.
    Arrived
}

thread_local! {
    // the web art being fetched, so each is asked for once
    static FETCHING: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    // the web art already in the cache, by URL, so a frame doesn't look on disk
    static FOUND: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// The file holding the art at `url`: the file itself for a `file://`
/// URL, else a copy in the cache, fetched in the background the first
/// time. Called while drawing: what asked draws again when it lands.
pub fn path(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    if let Some(path) = url.strip_prefix("file://") {
        return Some(emaki_kit::percent::decode(path));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    // asked on every frame of the card and the bar's Now Playing: a file found once is kept
    Arrived::watch();
    if let Some(found) = FOUND.with_borrow(|found| found.get(url).cloned()) {
        return Some(found);
    }
    let path = cached(url);
    if path.exists() {
        let found = path.to_string_lossy().into_owned();
        FOUND.with_borrow_mut(|known| {
            // a few players' art at most; a long session's list is let go now and then
            if known.len() > 64 {
                known.clear();
            }
            known.insert(url.to_string(), found.clone());
        });
        return Some(found);
    }
    let fresh = FETCHING.with_borrow_mut(|fetching| fetching.insert(url.to_string()));
    if fresh {
        let url = url.to_string();
        std::thread::spawn(move || {
            if let Some(dir) = path.parent() {
                // a folder that can't be made fails the fetch below, which is shown as no art
                let _ = std::fs::create_dir_all(dir);
            }
            let Some((host, port, address)) = public_address(&url) else {
                log::info!("not fetching art from {url}: not a public web address");
                return;
            };
            let address = match address {
                IpAddr::V6(v6) => format!("[{v6}]"),
                IpAddr::V4(v4) => v4.to_string(),
            };
            let part = path.with_extension("part");
            let fetched = std::process::Command::new(emaki_kit::tools::path("curl"))
                .args(["-fsS", "--max-time", "15", "--max-filesize", "10M"])
                .args(["--proto", "=http,https", "--max-redirs", "0"])
                // the address checked is the one fetched from, whatever DNS says next
                .arg("--resolve")
                .arg(format!("{host}:{port}:{address}"))
                .arg("-o")
                .arg(&part)
                .arg("--")
                .arg(&url)
                .status()
                .is_ok_and(|status| status.success());
            if fetched && std::fs::rename(&part, &path).is_ok() {
                prune(path.parent());
                Arrived::changed();
            } else {
                // a part file that was never written is gone already
                let _ = std::fs::remove_file(&part);
            }
        });
    }
    None
}

/// The host, port and a public address of an http(s) URL; none for a URL with credentials in
/// it, or a host that is, or resolves to, this computer or a private network.
fn public_address(url: &str) -> Option<(String, u16, IpAddr)> {
    let (scheme, rest) = url.split_once("://")?;
    let default_port = match scheme {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(bracketed) => {
            let (host, after) = bracketed.split_once(']')?;
            let port = match after.strip_prefix(':') {
                Some(port) => port.parse().ok()?,
                None if after.is_empty() => default_port,
                None => return None,
            };
            (host, port)
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().ok()?),
            None => (authority, default_port),
        },
    };
    if host.is_empty() {
        return None;
    }
    let addresses: Vec<IpAddr> = (host, port)
        .to_socket_addrs()
        .ok()?
        .map(|address| address.ip())
        .collect();
    // every address the name has must be public, so none of them is a way in
    if addresses.is_empty() || !addresses.iter().all(|address| is_public(*address)) {
        return None;
    }
    Some((host.to_string(), port, addresses[0]))
}

/// Whether `address` is on the internet: not this computer, a private or link-local network,
/// or anything else that isn't routed.
fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || a == 0
                // shared address space (carrier NAT, Tailscale)
                || (a == 100 && (64..128).contains(&b))
                // benchmarking, and reserved
                || (a == 198 && (b == 18 || b == 19))
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // unique local, link-local
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                // documentation
                || first == 0x2001 && v6.segments()[1] == 0x0db8)
        }
    }
}

// the art cache keeps the newest few hundred, so a long session doesn't fill the disk
const KEPT: usize = 200;

fn prune(folder: Option<&Path>) {
    let Some(entries) = folder.and_then(|folder| std::fs::read_dir(folder).ok()) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    if files.len() <= KEPT {
        return;
    }
    files.sort();
    for (_, old) in &files[..files.len() - KEPT] {
        let _ = std::fs::remove_file(old);
    }
}

/// Where the art at `url` is kept: `$XDG_CACHE_HOME/emaki/art/<hash>`.
fn cached(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let base = emaki_kit::paths::cache();
    base.join(emaki_kit::app_ids::SHELL)
        .join("art")
        .join(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_are_fetched_from() {
        for local in [
            "http://127.0.0.1/a.jpg",
            "http://localhost:631/",
            "https://[::1]/x",
            "http://192.168.1.1/x",
            "http://10.0.0.5:8080/x",
            "http://169.254.169.254/latest/meta-data",
            "http://100.100.100.100/x",
            "https://[fd00::1]/x",
            "https://[::ffff:127.0.0.1]/x",
            "http://user:pw@1.1.1.1/x",
            "ftp://1.1.1.1/x",
        ] {
            assert!(public_address(local).is_none(), "{local}");
        }
        let (host, port, address) = public_address("https://1.1.1.1/art.jpg").unwrap();
        assert_eq!((host.as_str(), port), ("1.1.1.1", 443));
        assert!(address.is_ipv4());
        assert_eq!(
            public_address("http://8.8.8.8:8080/x").map(|a| a.1),
            Some(8080)
        );
    }

    #[test]
    fn web_art_is_cached_by_its_url() {
        let a = cached("https://x/1.jpg");
        assert_eq!(a, cached("https://x/1.jpg"));
        assert_ne!(a, cached("https://x/2.jpg"));
        assert!(a.ends_with(a.file_name().unwrap()));
        assert!(a.to_string_lossy().contains("emaki/art"));
    }
}
