//! tailscaled's LocalAPI, over its socket: the same JSON the `tailscale`
//! command prints, with no program started for it. Any local user may read
//! it; changes still go through the command, which checks the operator grant.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// Where tailscaled listens on Linux.
const SOCKET: &str = "/var/run/tailscale/tailscaled.sock";

// tailscaled checks the Host header; any request from this machine uses this one
const HOST: &str = "local-tailscaled.sock";

// a read that hangs past this counts as the daemon gone
const READ_TIMEOUT: Duration = Duration::from_secs(5);

// the notices the change stream sends: 0 is state, prefs and netmap changes,
// without the engine's traffic counters every few seconds
const WATCH: &str = "/localapi/v0/watch-ipn-bus?mask=0";

/// Whether the socket is there to talk to.
pub fn available() -> bool {
    Path::new(SOCKET).exists()
}

/// The body of `GET path`, or `None` when the daemon doesn't answer 200.
pub fn get(path: &str) -> Option<String> {
    let mut stream = connect(path).ok()?;
    stream.set_read_timeout(Some(READ_TIMEOUT)).ok()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;
    body(&response).map(|body| String::from_utf8_lossy(body).into_owned())
}

/// Follows the change stream, calling `notice` once per notice, until the
/// daemon closes it. Whether anything came.
pub fn watch(mut notice: impl FnMut()) -> bool {
    let Ok(stream) = connect(WATCH) else {
        return false;
    };
    let mut reader = BufReader::new(stream);

    // the status line and headers, then one JSON object per line
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || !ok(&line) {
        return false;
    }
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return false,
            Ok(_) if line.trim().is_empty() => break,
            Ok(_) => {}
        }
    }

    let mut heard = false;
    for line in reader.lines() {
        let Ok(line) = line else {
            break;
        };
        if !line.trim().is_empty() {
            heard = true;
            notice();
        }
    }
    heard
}

// HTTP/1.0, so the answer comes whole or as a stream that ends with the
// connection, never chunked
fn connect(path: &str) -> io::Result<UnixStream> {
    let mut stream = UnixStream::connect(SOCKET)?;
    write!(stream, "GET {path} HTTP/1.0\r\nHost: {HOST}\r\n\r\n")?;
    Ok(stream)
}

fn ok(status_line: &str) -> bool {
    status_line.split_whitespace().nth(1) == Some("200")
}

fn body(response: &[u8]) -> Option<&[u8]> {
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?;
    let status = std::str::from_utf8(&response[..split]).ok()?;
    ok(status.lines().next()?).then_some(&response[split + 4..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_body_of_a_good_answer() {
        let answer = b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{\"a\":1}";
        assert_eq!(body(answer), Some(&b"{\"a\":1}"[..]));
        let refused = b"HTTP/1.0 403 Forbidden\r\n\r\naccess denied";
        assert_eq!(body(refused), None);
        assert_eq!(body(b"garbage"), None);
    }
}
