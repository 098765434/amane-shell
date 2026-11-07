//! Finding files for the launcher's Files: by name and by what's in them
//! through GNOME's LocalSearch index when it runs (`tinysparql`), else by
//! name alone, walking the home folder with `fd`. A search runs off the UI
//! thread a moment after the last key, the newest replacing any before
//! it, and what it found is kept for the view to read.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::services::run_now;

amane_kit::marker! {
    /// A search finished: what asked for its hits draws again.
    pub Found
}

/// How long after the last key a search starts.
const DEBOUNCE: Duration = Duration::from_millis(120);
/// How many files a search gives at most, and how many recent ones.
const LIMIT: usize = 60;
const RECENT: usize = 20;
/// How recent a file is to be listed before anything is typed, without
/// the index (fd).
const RECENT_DAYS: &str = "7d";
/// How long whether the index runs is trusted before asking again.
const RECHECK: Duration = Duration::from_secs(300);
const LOCALSEARCH: &str = "org.freedesktop.LocalSearch3";

/// A kind of file to narrow a search to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Pdf,
    Image,
    Document,
    Spreadsheet,
    Presentation,
    Folder,
    Audio,
    Video,
    Archive,
}

impl FileKind {
    pub const ALL: [Self; 9] = [
        Self::Pdf,
        Self::Image,
        Self::Document,
        Self::Folder,
        Self::Spreadsheet,
        Self::Presentation,
        Self::Audio,
        Self::Video,
        Self::Archive,
    ];

    /// What its chip says.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Image => "Images",
            Self::Document => "Documents",
            Self::Spreadsheet => "Spreadsheets",
            Self::Presentation => "Presentations",
            Self::Folder => "Folders",
            Self::Audio => "Music",
            Self::Video => "Movies",
            Self::Archive => "Archives",
        }
    }

    /// The file name endings that are it, for searching by name.
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Pdf => &["pdf"],
            Self::Image => &[
                "png", "jpg", "jpeg", "gif", "webp", "svg", "heic", "avif", "bmp", "tiff",
            ],
            Self::Document => &["txt", "md", "odt", "doc", "docx", "rtf", "tex", "epub"],
            Self::Spreadsheet => &["ods", "xls", "xlsx", "csv"],
            Self::Presentation => &["odp", "ppt", "pptx", "key"],
            Self::Folder => &[],
            Self::Audio => &["mp3", "flac", "ogg", "opus", "wav", "m4a", "aac"],
            Self::Video => &["mp4", "mkv", "webm", "mov", "avi"],
            Self::Archive => &["zip", "tar", "gz", "xz", "zst", "7z", "rar", "bz2"],
        }
    }

    /// A SPARQL test on `?mime` for it, for searching the index.
    fn sparql(self) -> String {
        let starts = |prefix: &str| format!("STRSTARTS(?mime, \"{prefix}\")");
        let any = |mimes: &[&str]| {
            let quoted: Vec<String> = mimes.iter().map(|mime| format!("\"{mime}\"")).collect();
            format!("?mime IN ({})", quoted.join(", "))
        };
        match self {
            Self::Pdf => any(&["application/pdf"]),
            Self::Image => starts("image/"),
            Self::Audio => starts("audio/"),
            Self::Video => starts("video/"),
            Self::Folder => any(&["inode/directory"]),
            Self::Document => format!(
                "({} || {})",
                starts("text/"),
                any(&[
                    "application/vnd.oasis.opendocument.text",
                    "application/msword",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    "application/rtf",
                    "application/epub+zip",
                ])
            ),
            Self::Spreadsheet => any(&[
                "application/vnd.oasis.opendocument.spreadsheet",
                "application/vnd.ms-excel",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "text/csv",
            ]),
            Self::Presentation => any(&[
                "application/vnd.oasis.opendocument.presentation",
                "application/vnd.ms-powerpoint",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ]),
            Self::Archive => any(&[
                "application/zip",
                "application/x-tar",
                "application/gzip",
                "application/x-xz",
                "application/zstd",
                "application/x-7z-compressed",
                "application/vnd.rar",
                "application/x-bzip2",
            ]),
        }
    }

    /// The kind a file named `name` is, by its ending.
    pub fn of(name: &str) -> Option<Self> {
        let ext = name.rsplit_once('.')?.1.to_lowercase();
        Self::ALL
            .into_iter()
            .find(|kind| kind.extensions().contains(&ext.as_str()))
    }
}

/// What a search is for: `text` in a name (or in a file, with the index),
/// of `kind`, under `folder`. Nothing typed lists the recent files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub text: String,
    pub kind: Option<FileKind>,
    pub folder: Option<PathBuf>,
}

/// A file found.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub path: PathBuf,
    pub name: String,
    pub dir: bool,
    pub size: u64,
    /// When it last changed, in seconds since the epoch.
    pub modified: u64,
}

struct State {
    // what the last search to finish was for, and found
    done: Option<(Request, Vec<Hit>)>,
    // the newest search asked for
    asked: Option<Request>,
    // whether the index runs, and when that was last asked
    indexed: Option<(bool, Instant)>,
}

static STATE: Mutex<State> = Mutex::new(State {
    done: None,
    asked: None,
    indexed: None,
});
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// What `request` finds: the files, and whether they're for it (rather
/// than for the search before, while this one runs). Asks for it to run
/// if it hasn't.
pub fn find(request: &Request) -> (Vec<Hit>, bool) {
    Found::watch();
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((done, hits)) = &state.done
        && done == request
    {
        return (hits.clone(), true);
    }
    let stale = state
        .done
        .as_ref()
        .map(|(_, hits)| hits.clone())
        .unwrap_or_default();
    if state.asked.as_ref() != Some(request) {
        state.asked = Some(request.clone());
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        let request = request.clone();
        std::thread::spawn(move || run(request, generation));
    }
    (stale, false)
}

fn run(request: Request, generation: u64) {
    std::thread::sleep(DEBOUNCE);
    if GENERATION.load(Ordering::SeqCst) != generation {
        return;
    }
    let hits = if index_runs() {
        search_index(&request)
    } else {
        search_names(&request)
    };
    if GENERATION.load(Ordering::SeqCst) != generation {
        return;
    }
    STATE.lock().unwrap_or_else(|e| e.into_inner()).done = Some((request, hits));
    Found::changed();
}

/// Whether LocalSearch answers, asked every few minutes.
fn index_runs() -> bool {
    let known = STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .indexed
        .filter(|(_, at)| at.elapsed() < RECHECK);
    if let Some((on, _)) = known {
        return on;
    }
    let on = Command::new("tinysparql")
        .args(["query", "-b", LOCALSEARCH, "-q", "ASK { }"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    STATE.lock().unwrap_or_else(|e| e.into_inner()).indexed = Some((on, Instant::now()));
    on
}

fn search_index(request: &Request) -> Vec<Hit> {
    let (query, args) = sparql(request);
    let mut command = vec![
        String::from("query"),
        String::from("-b"),
        String::from(LOCALSEARCH),
        String::from("-q"),
        query,
    ];
    for (name, value) in args {
        command.push(String::from("-a"));
        command.push(format!("{name}:s:{value}"));
    }
    let Some(out) = run_now("tinysparql", &command) else {
        return Vec::new();
    };
    let mut hits: Vec<Hit> = parse_rows(&out)
        .into_iter()
        .filter_map(|path| hit(&path))
        .collect();
    hits.dedup_by(|a, b| a.path == b.path);
    if request.text.is_empty() {
        hits.truncate(RECENT);
    }
    hits
}

/// The SPARQL that finds `request` in LocalSearch's index, and its
/// arguments: files whose name has the text in it, or whose content
/// matches it; newest first, each as its URL.
pub fn sparql(request: &Request) -> (String, Vec<(&'static str, String)>) {
    let mut args = Vec::new();
    let mut filters = Vec::new();
    let found = if request.text.is_empty() {
        String::new()
    } else {
        args.push(("q", request.text.to_lowercase()));
        // FTS5 reads a few characters as its own: as a phrase they're text
        let phrase = format!("\"{}\"", request.text.replace('"', ""));
        args.push(("fts", phrase));
        String::from(
            "{ ?f nfo:fileName ?n . FILTER(CONTAINS(LCASE(?n), ~q)) } \
             UNION { ?c fts:match ~fts ; nie:isStoredAs ?f } ",
        )
    };
    if let Some(kind) = request.kind {
        filters.push(kind.sparql());
    } else if request.text.is_empty() {
        // recent files, not the folders they're in
        filters.push(String::from("?mime != \"inode/directory\""));
    }
    if let Some(folder) = &request.folder {
        args.push(("folder", file_url(folder)));
        filters.push(String::from("STRSTARTS(?url, ~folder)"));
    }
    let filter = if filters.is_empty() {
        String::new()
    } else {
        format!("FILTER({}) ", filters.join(" && "))
    };
    let limit = if request.text.is_empty() {
        RECENT * 2
    } else {
        LIMIT
    };
    let query = format!(
        "SELECT DISTINCT ?url WHERE {{ {found}?f a nfo:FileDataObject ; nie:url ?url ; \
         nfo:fileLastModified ?mod . ?f nie:interpretedAs ?ie . ?ie nie:mimeType ?mime . \
         {filter}}} ORDER BY DESC(?mod) LIMIT {limit}"
    );
    (query, args)
}

/// The paths in tinysparql's printout of one column of URLs.
pub fn parse_rows(out: &str) -> Vec<PathBuf> {
    out.lines()
        .filter_map(|line| line.strip_prefix("  "))
        .filter_map(|url| url.trim().strip_prefix("file://"))
        .map(|path| PathBuf::from(percent_decode(path)))
        .collect()
}

fn search_names(request: &Request) -> Vec<Hit> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let base = request.folder.clone().unwrap_or(home);
    let recent = request.text.is_empty();
    let args = fd_args(request, &base);
    let Some(out) = run_now("fd", &args) else {
        return Vec::new();
    };
    let mut hits: Vec<Hit> = out
        .lines()
        .filter_map(|line| hit(Path::new(line.trim_end_matches('/'))))
        .collect();
    if recent {
        hits.sort_by_key(|hit| std::cmp::Reverse(hit.modified));
        hits.truncate(RECENT);
    }
    hits
}

/// fd's arguments for `request`, looking under `base`: the text as it is,
/// in any case, or the files changed lately when there's none.
pub fn fd_args(request: &Request, base: &Path) -> Vec<String> {
    let mut args = vec![
        String::from("--absolute-path"),
        String::from("--ignore-case"),
        String::from("--max-results"),
    ];
    if request.text.is_empty() {
        args.extend([
            String::from("300"),
            String::from("--changed-within"),
            String::from(RECENT_DAYS),
        ]);
    } else {
        args.extend([LIMIT.to_string(), String::from("--fixed-strings")]);
    }
    match request.kind {
        Some(FileKind::Folder) => args.extend([String::from("--type"), String::from("d")]),
        Some(kind) => {
            args.extend([String::from("--type"), String::from("f")]);
            for ext in kind.extensions() {
                args.extend([String::from("--extension"), ext.to_string()]);
            }
        }
        None if request.text.is_empty() => args.extend([String::from("--type"), String::from("f")]),
        None => {}
    }
    args.push(String::from("--"));
    args.push(if request.text.is_empty() {
        String::from(".")
    } else {
        request.text.clone()
    });
    args.push(base.to_string_lossy().into_owned());
    args
}

fn hit(path: &Path) -> Option<Hit> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs());
    Some(Hit {
        path: path.to_path_buf(),
        name: path.file_name()?.to_string_lossy().into_owned(),
        dir: meta.is_dir(),
        size: meta.len(),
        modified,
    })
}

/// `path` as a `file://` URL, its bytes outside the safe ones escaped, as
/// LocalSearch keeps them.
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                url.push(byte as char)
            }
            other => url.push_str(&format!("%{other:02X}")),
        }
    }
    url
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(byte) = text
                .get(at + 1..at + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Opens `path` in the app it opens with.
pub fn open(path: &Path) {
    let path = path.to_string_lossy().into_owned();
    std::thread::spawn(move || {
        run_now("xdg-open", &[path]);
    });
}

/// Shows `path` in the file manager, picked out in its folder
/// (`org.freedesktop.FileManager1`).
pub fn reveal(path: &Path) {
    let url = file_url(path);
    std::thread::spawn(move || {
        run_now(
            "gdbus",
            &[
                String::from("call"),
                String::from("--session"),
                String::from("--dest"),
                String::from("org.freedesktop.FileManager1"),
                String::from("--object-path"),
                String::from("/org/freedesktop/FileManager1"),
                String::from("--method"),
                String::from("org.freedesktop.FileManager1.ShowItems"),
                format!("['{url}']"),
                String::new(),
            ],
        );
    });
}

/// Seconds since the epoch.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(text: &str, kind: Option<FileKind>) -> Request {
        Request {
            text: text.to_string(),
            kind,
            folder: None,
        }
    }

    #[test]
    fn a_file_is_its_kind_by_its_ending() {
        assert_eq!(FileKind::of("Report.PDF"), Some(FileKind::Pdf));
        assert_eq!(FileKind::of("photo.jpeg"), Some(FileKind::Image));
        assert_eq!(FileKind::of("notes"), None);
        assert_eq!(FileKind::of("main.rs"), None);
    }

    #[test]
    fn the_index_is_asked_by_name_and_content() {
        let (query, args) = sparql(&request("Budget \"Q3\"", None));
        assert!(query.contains("fts:match ~fts"));
        assert!(query.contains("CONTAINS(LCASE(?n), ~q)"));
        assert!(query.contains(&format!("LIMIT {LIMIT}")));
        assert_eq!(args[0], ("q", String::from("budget \"q3\"")));
        // quotes inside the phrase would end it
        assert_eq!(args[1], ("fts", String::from("\"Budget Q3\"")));
    }

    #[test]
    fn nothing_typed_asks_for_recent_files() {
        let (query, args) = sparql(&request("", None));
        assert!(!query.contains("fts:match"));
        assert!(query.contains("?mime != \"inode/directory\""));
        assert!(query.contains("ORDER BY DESC(?mod)"));
        assert!(args.is_empty());
    }

    #[test]
    fn a_kind_and_a_folder_narrow_the_query() {
        let mut asked = request("x", Some(FileKind::Pdf));
        asked.folder = Some(PathBuf::from("/home/me/My Docs"));
        let (query, args) = sparql(&asked);
        assert!(query.contains("?mime IN (\"application/pdf\")"));
        assert!(query.contains("STRSTARTS(?url, ~folder)"));
        assert!(args.contains(&("folder", String::from("file:///home/me/My%20Docs"))));
    }

    #[test]
    fn rows_come_back_as_paths() {
        let out = "Results:\n  file:///home/me/My%20Doc.pdf\n  file:///home/me/notes.txt\n\n";
        assert_eq!(
            parse_rows(out),
            vec![
                PathBuf::from("/home/me/My Doc.pdf"),
                PathBuf::from("/home/me/notes.txt")
            ]
        );
        assert!(parse_rows("Results:\n  None\n").is_empty());
    }

    #[test]
    fn fd_looks_for_the_text_or_whats_recent() {
        let base = Path::new("/home/me");
        let args = fd_args(&request("budget", Some(FileKind::Pdf)), base);
        assert!(args.contains(&String::from("--fixed-strings")));
        assert!(args.windows(2).any(|pair| pair == ["--extension", "pdf"]));
        assert_eq!(&args[args.len() - 2..], ["budget", "/home/me"]);
        let recent = fd_args(&request("", None), base);
        assert!(
            recent
                .windows(2)
                .any(|pair| pair == ["--changed-within", RECENT_DAYS])
        );
        assert_eq!(&recent[recent.len() - 2..], [".", "/home/me"]);
        let folders = fd_args(&request("src", Some(FileKind::Folder)), base);
        assert!(folders.windows(2).any(|pair| pair == ["--type", "d"]));
    }
}
