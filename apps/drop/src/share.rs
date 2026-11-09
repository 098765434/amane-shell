//! Uploads, the sharing actions: 0x0.st, Imgur, an SFTP server
//! or an S3 bucket, each through `curl`. Secrets go to curl on its stdin
//! (`-K -`), never in its arguments, where `ps` would show them. Progress
//! comes from curl's progress bar; a cancel kills curl.

use std::fmt::Write as _;
use std::io::{self, Read, Write};

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;
use zeroize::Zeroizing;

use emaki_kit::settings::{DropSharing, UploadTo};

const APP: &str = emaki_kit::app_ids::DROP;

/// 0x0.st's address; `EMAKI_DROP_0X0_URL` points it elsewhere, for tests.
fn zero_x0() -> String {
    std::env::var("EMAKI_DROP_0X0_URL").unwrap_or_else(|_| String::from("https://0x0.st"))
}

/// How the link is found once curl is done.
#[derive(Clone, Debug, PartialEq)]
enum Link {
    /// What the server answered, trimmed (0x0.st).
    Answer,
    /// `data.link` in Imgur's JSON.
    Imgur,
    /// Known before: the public address the file lands at.
    Known(String),
    None,
}

/// One curl run: its arguments, what it's told on stdin, and where the
/// link comes from.
#[derive(Clone, Debug, PartialEq)]
struct Request {
    args: Vec<String>,
    // curl's config, with the secret in it: wiped when dropped
    config: Zeroizing<String>,
    link: Link,
}

/// A value in curl's config syntax: quoted, with `\` and `"` escaped.
fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for char in value.chars() {
        match char {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// A path's part made safe for a URL; `/` kept when `slashes`.
pub fn encode(text: &str, slashes: bool) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            b'/' if slashes => String::from("/"),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A file for curl's `-F name=@"file"`.
fn form_file(field: &str, file: &Path) -> String {
    let path = file.display().to_string();
    format!(
        "{field}=@\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

fn join(base: &str, rest: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        rest.trim_start_matches('/')
    )
}

/// What curl's run for `file` to `to` is; `secret` is the account's
/// password or key, looked up before.
fn request(
    file: &Path,
    to: UploadTo,
    sharing: &DropSharing,
    secret: Option<&str>,
) -> Result<Request, String> {
    if let Some(missing) = sharing.missing(to) {
        return Err(format!("{}: {}", to.title(), missing.to_lowercase()));
    }
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or("a file without a name")?;
    let mut args: Vec<String> = ["-f", "-#", "--connect-timeout", "20", "-K", "-"]
        .map(String::from)
        .to_vec();
    let mut config = Zeroizing::new(String::new());
    let link = match to {
        UploadTo::ZeroX0 => {
            args.extend([String::from("-F"), form_file("file", file), zero_x0()]);
            Link::Answer
        }
        UploadTo::Imgur => {
            if !emaki_kit::mime::is_image(file) && !emaki_kit::mime::is_video(file) {
                return Err(String::from("Imgur takes pictures and videos"));
            }
            let field = if emaki_kit::mime::is_video(file) {
                "video"
            } else {
                "image"
            };
            let _ = config.write_fmt(format_args!(
                "header = {}\n",
                quoted(&format!(
                    "Authorization: Client-ID {}",
                    sharing.imgur_client_id.trim()
                ))
            ));
            args.extend([
                String::from("-F"),
                form_file(field, file),
                String::from("https://api.imgur.com/3/image"),
            ]);
            Link::Imgur
        }
        UploadTo::Sftp => {
            let account = &sharing.sftp;
            let folder = account.folder.trim().trim_end_matches('/');
            // a folder not from the root is from the user's home
            let path = if folder.starts_with('/') {
                encode(folder, true)
            } else if folder.is_empty() {
                String::from("/~")
            } else {
                format!("/~/{}", encode(folder, true))
            };
            let url = format!(
                "sftp://{}:{}{}/{}",
                account.host.trim(),
                account.port,
                path,
                encode(&name, false)
            );
            match &account.key {
                Some(key) => {
                    let _ = config.write_fmt(format_args!(
                        "user = {}\n",
                        quoted(&format!("{}:", account.user))
                    ));
                    let _ = config.write_fmt(format_args!(
                        "key = {}\n",
                        quoted(&key.display().to_string())
                    ));
                    if let Some(passphrase) = secret {
                        let _ = config.write_fmt(format_args!("pass = {}\n", quoted(passphrase)));
                    }
                }
                None => {
                    let password = secret.ok_or("SFTP: no password kept")?;
                    let _ = config.write_fmt(format_args!(
                        "user = {}\n",
                        quoted(&format!("{}:{password}", account.user))
                    ));
                }
            }
            args.extend([
                String::from("--ftp-create-dirs"),
                String::from("-T"),
                file.display().to_string(),
                url,
            ]);
            if account.url.trim().is_empty() {
                Link::None
            } else {
                Link::Known(join(account.url.trim(), &encode(&name, false)))
            }
        }
        UploadTo::S3 => {
            let account = &sharing.s3;
            let secret = secret.ok_or("S3: no secret key kept")?;
            let region = match account.region.trim() {
                "" => "us-east-1",
                region => region,
            };
            let key = match account.prefix.trim().trim_matches('/') {
                "" => name.clone(),
                prefix => format!("{prefix}/{name}"),
            };
            let bucket = account.bucket.trim();
            let object = match account.endpoint.trim() {
                "" => format!(
                    "https://{bucket}.s3.{region}.amazonaws.com/{}",
                    encode(&key, true)
                ),
                endpoint => join(endpoint, &format!("{bucket}/{}", encode(&key, true))),
            };
            let _ = config.write_fmt(format_args!(
                "user = {}\n",
                quoted(&format!("{}:{secret}", account.key_id.trim()))
            ));
            args.extend([
                String::from("--aws-sigv4"),
                format!("aws:amz:{region}:s3"),
                String::from("-H"),
                String::from("x-amz-content-sha256: UNSIGNED-PAYLOAD"),
                String::from("-H"),
                format!("Content-Type: {}", emaki_kit::mime::of(file)),
            ]);
            if account.public {
                args.extend([String::from("-H"), String::from("x-amz-acl: public-read")]);
            }
            args.extend([
                String::from("-T"),
                file.display().to_string(),
                object.clone(),
            ]);
            Link::Known(if account.url.trim().is_empty() {
                object
            } else {
                join(account.url.trim(), &encode(&key, true))
            })
        }
    };
    Ok(Request { args, config, link })
}

/// The secret `to` logs in with, if it has one.
fn secret_for(to: UploadTo) -> Option<Zeroizing<String>> {
    match to {
        UploadTo::Sftp => emaki_kit::secrets::lookup(APP, "sftp"),
        UploadTo::S3 => emaki_kit::secrets::lookup(APP, "s3"),
        UploadTo::ZeroX0 | UploadTo::Imgur => None,
    }
}

/// Uploads `file` to `to`, telling how far along it is (0 to 1); the link
/// to it, when there is one.
pub fn upload(
    file: &Path,
    to: UploadTo,
    cancel: &AtomicBool,
    tell: &mut dyn FnMut(f32),
) -> io::Result<Option<String>> {
    if file.is_dir() {
        return Err(io::Error::other("folders can't be uploaded"));
    }
    let sharing = emaki_kit::settings::drop_sharing().clone();
    let secret = secret_for(to);
    let request = request(
        file,
        to,
        &sharing,
        secret.as_ref().map(|secret| secret.as_str()),
    )
    .map_err(io::Error::other)?;
    let answer = run_curl(&request, cancel, tell)?;
    match request.link {
        Link::Answer => Ok(Some(answer.trim().to_string()).filter(|link| !link.is_empty())),
        Link::Imgur => imgur_link(&answer)
            .map(Some)
            .ok_or_else(|| io::Error::other("Imgur gave no link")),
        Link::Known(link) => Ok(Some(link)),
        Link::None => Ok(None),
    }
}

fn imgur_link(answer: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(answer).ok()?;
    json.pointer("/data/link")?.as_str().map(String::from)
}

/// Text uploaded as a file: written where only this user reads, uploaded,
/// and removed.
pub fn upload_text(
    text: &str,
    to: UploadTo,
    cancel: &AtomicBool,
    tell: &mut dyn FnMut(f32),
) -> io::Result<Option<String>> {
    let folder = emaki_kit::paths::runtime().join("emaki-drop");
    std::fs::create_dir_all(&folder)?;
    let file = folder.join(format!("Text {}.txt", std::process::id()));
    // a new file only this user reads, never through a link left in its place
    emaki_kit::fs::Whole::new().private().write(&file, text)?;
    let result = upload(&file, to, cancel, tell);
    // a file of its own for this upload; a leftover is only space
    let _ = std::fs::remove_file(file);
    result
}

/// How far curl's progress bar says it is ("####   45.3%"), 0 to 1.
fn progress(piece: &str) -> Option<f32> {
    let percent = piece.trim().rsplit(' ').next()?.strip_suffix('%')?;
    percent
        .trim()
        .parse::<f32>()
        .ok()
        .map(|percent| (percent / 100.0).clamp(0.0, 1.0))
}

/// Runs curl, telling its progress; what the server answered.
fn run_curl(
    request: &Request,
    cancel: &AtomicBool,
    tell: &mut dyn FnMut(f32),
) -> io::Result<String> {
    let mut child = Command::new(emaki_kit::tools::path("curl"))
        .args(&request.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| io::Error::other(format!("couldn't run curl: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(request.config.as_bytes())?;
    }

    // the answer read alongside, so a full pipe never stalls curl
    let mut stdout = child.stdout.take().ok_or(io::ErrorKind::BrokenPipe)?;
    let answer = std::thread::spawn(move || {
        let mut text = String::new();
        // a pipe closed early leaves what was read
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let mut stderr = child.stderr.take().ok_or(io::ErrorKind::BrokenPipe)?;
    let (pieces, heard) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 512];
        let mut piece = Vec::new();
        while let Ok(read) = stderr.read(&mut buffer) {
            if read == 0 {
                break;
            }
            for &byte in &buffer[..read] {
                if byte == b'\r' || byte == b'\n' {
                    // the reader is gone: the upload has ended
                    let _ = pieces.send(String::from_utf8_lossy(&piece).into_owned());
                    piece.clear();
                } else {
                    piece.push(byte);
                }
            }
        }
        if !piece.is_empty() {
            // the reader is gone: the upload has ended
            let _ = pieces.send(String::from_utf8_lossy(&piece).into_owned());
        }
    });

    let mut said = String::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            // cancelled: curl may have ended already
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::ErrorKind::Interrupted.into());
        }
        match heard.recv_timeout(Duration::from_millis(100)) {
            Ok(piece) => {
                if let Some(done) = progress(&piece) {
                    tell(done);
                } else if piece.starts_with("curl:") {
                    said = piece;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = child.wait()?;
    let answer = answer.join().unwrap_or_default();
    if status.success() {
        return Ok(answer);
    }
    // curl's own words, "(22) The requested URL returned error: 403"
    let said = said.trim_start_matches("curl:").trim();
    Err(io::Error::other(if said.is_empty() {
        format!("curl failed ({status})")
    } else {
        said.to_string()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sharing() -> DropSharing {
        let mut sharing = DropSharing::default();
        sharing.sftp.host = String::from("example.org");
        sharing.sftp.user = String::from("me");
        sharing.sftp.folder = String::from("public_html/up");
        sharing.sftp.url = String::from("https://example.org/up/");
        sharing.s3.bucket = String::from("pics");
        sharing.s3.key_id = String::from("AKID");
        sharing.s3.prefix = String::from("/drop/");
        sharing
    }

    #[test]
    fn curls_bar_reads_as_progress() {
        assert_eq!(progress("######                    45.0%"), Some(0.45));
        assert_eq!(progress("#=#=#"), None);
        assert_eq!(progress("100.0%"), Some(1.0));
        assert_eq!(progress("curl: (6) Could not resolve host"), None);
    }

    #[test]
    fn secrets_go_on_stdin_quoted() {
        let request = request(
            Path::new("/tmp/My \"Trip\".jpg"),
            UploadTo::Sftp,
            &sharing(),
            Some("p\"w"),
        )
        .unwrap();
        assert!(request.config.contains(r#"user = "me:p\"w""#));
        assert!(!request.args.iter().any(|arg| arg.contains("p\"w")));
        assert_eq!(
            request.args.last().unwrap(),
            "sftp://example.org:22/~/public_html/up/My%20%22Trip%22.jpg"
        );
        assert_eq!(
            request.link,
            Link::Known(String::from("https://example.org/up/My%20%22Trip%22.jpg"))
        );
    }

    #[test]
    fn s3_goes_to_the_bucket_or_another_store() {
        let mut sharing = sharing();
        let aws = request(Path::new("/a/b c.png"), UploadTo::S3, &sharing, Some("s")).unwrap();
        let object = "https://pics.s3.us-east-1.amazonaws.com/drop/b%20c.png";
        assert_eq!(aws.args.last().unwrap(), object);
        assert_eq!(aws.link, Link::Known(String::from(object)));
        assert!(aws.args.contains(&String::from("aws:amz:us-east-1:s3")));
        assert!(aws.args.contains(&String::from("Content-Type: image/png")));

        sharing.s3.endpoint = String::from("https://id.r2.cloudflarestorage.com/");
        sharing.s3.region = String::from("auto");
        sharing.s3.url = String::from("https://cdn.example.org");
        let r2 = request(Path::new("/a/b.png"), UploadTo::S3, &sharing, Some("s")).unwrap();
        assert_eq!(
            r2.args.last().unwrap(),
            "https://id.r2.cloudflarestorage.com/pics/drop/b.png"
        );
        assert_eq!(
            r2.link,
            Link::Known(String::from("https://cdn.example.org/drop/b.png"))
        );
        // no secret, no upload
        assert!(request(Path::new("/a/b.png"), UploadTo::S3, &sharing, None).is_err());
    }

    #[test]
    fn imgur_takes_pictures_and_finds_the_link() {
        let mut sharing = sharing();
        assert!(request(Path::new("/a.png"), UploadTo::Imgur, &sharing, None).is_err());
        sharing.imgur_client_id = String::from("abc");
        assert!(request(Path::new("/a.pdf"), UploadTo::Imgur, &sharing, None).is_err());
        let ok = request(Path::new("/a.mp4"), UploadTo::Imgur, &sharing, None).unwrap();
        assert!(ok.args.contains(&String::from("video=@\"/a.mp4\"")));
        assert!(ok.config.contains("Client-ID abc"));
        assert_eq!(
            imgur_link(r#"{"data":{"link":"https://i.imgur.com/x.png"},"success":true}"#),
            Some(String::from("https://i.imgur.com/x.png"))
        );
    }
}
