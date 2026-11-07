//! Default Apps: which app opens links, folders and each kind of file, and
//! which terminal the keys open. The kinds live in the freedesktop
//! `mimeapps.list` files that `xdg-open` reads, through amane's `MimeApps`,
//! which changes only the user's own `~/.config/mimeapps.list`. The
//! terminal is `xdg-terminal-exec`'s, in `~/.config/xdg-terminals.list`.
//!
//! Each role names one kind it's known by (a browser by `https` links) and
//! sets the kinds that go with it: a fixed set for links and mail, or every
//! kind of image, video or sound the chosen app says it opens.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use amane::{Apps, Column, DesktopApp, MimeApps, Rectangle, Service, TextInput, Widget};
use amane_kit::app::{Dialog, field_box, text_field};
use amane_kit::buttons::PushButton;
use amane_kit::color::Accent;
use amane_kit::dropdown::Dropdown;
use amane_kit::form::FormRow;
use amane_kit::icons::{ICON_MUSIC, ICON_TERMINAL};
use amane_kit::layout::boxed;
use amane_kit::menu::MenuItem;

use crate::{View, group, groups};

const ICON_WEB: char = '\u{e80b}';
const ICON_MAIL: char = '\u{e158}';
const ICON_FOLDER: char = '\u{e2c7}';
const ICON_TEXT: char = '\u{e745}';
const ICON_IMAGE: char = '\u{e3f4}';
const ICON_MOVIE: char = '\u{e02c}';
const ICON_PDF: char = '\u{e415}';
const ICON_ARCHIVE: char = '\u{eb2c}';
const ICON_CALENDAR: char = '\u{ebcc}';
const ICON_FILE_TYPES: char = '\u{e873}';

/// The kind a File Types search is typed into.
const TYPE_FIELD: &str = "settings.defaults.type";

/// What else a role sets, beside the kind it's known by.
enum Kinds {
    /// These, whether or not the app lists them.
    Fixed(&'static [&'static str]),
    /// Those of these the app says it opens.
    Listed(&'static [&'static str]),
    /// Every kind under this prefix the app says it opens, but these.
    Prefix(&'static str, &'static [&'static str]),
}

struct Role {
    id: &'static str,
    title: &'static str,
    glyph: char,
    accent: Accent,
    /// The kind the role is known by: its default is the role's app.
    key: &'static str,
    kinds: Kinds,
}

const ROLES: [Role; 10] = [
    Role {
        id: "browser",
        title: "Web browser",
        glyph: ICON_WEB,
        accent: Accent::Blue,
        key: "x-scheme-handler/https",
        kinds: Kinds::Fixed(&[
            "x-scheme-handler/http",
            "text/html",
            "application/xhtml+xml",
        ]),
    },
    Role {
        id: "mail",
        title: "Mail",
        glyph: ICON_MAIL,
        accent: Accent::Blue,
        key: "x-scheme-handler/mailto",
        kinds: Kinds::Listed(&["message/rfc822"]),
    },
    Role {
        id: "files",
        title: "Folders",
        glyph: ICON_FOLDER,
        accent: Accent::Cyan,
        key: "inode/directory",
        kinds: Kinds::Fixed(&[]),
    },
    Role {
        id: "text",
        title: "Text editor",
        glyph: ICON_TEXT,
        accent: Accent::Gray,
        key: "text/plain",
        kinds: Kinds::Prefix(
            "text/",
            &["text/html", "text/calendar", "text/vcard", "text/x-vcard"],
        ),
    },
    Role {
        id: "images",
        title: "Images",
        glyph: ICON_IMAGE,
        accent: Accent::Green,
        key: "image/png",
        kinds: Kinds::Prefix("image/", &[]),
    },
    Role {
        id: "video",
        title: "Video",
        glyph: ICON_MOVIE,
        accent: Accent::Purple,
        key: "video/mp4",
        kinds: Kinds::Prefix("video/", &[]),
    },
    Role {
        id: "music",
        title: "Music",
        glyph: ICON_MUSIC,
        accent: Accent::Pink,
        key: "audio/mpeg",
        kinds: Kinds::Prefix("audio/", &[]),
    },
    Role {
        id: "pdf",
        title: "PDF documents",
        glyph: ICON_PDF,
        accent: Accent::Red,
        key: "application/pdf",
        kinds: Kinds::Fixed(&[]),
    },
    Role {
        id: "archives",
        title: "Archives",
        glyph: ICON_ARCHIVE,
        accent: Accent::Brown,
        key: "application/zip",
        kinds: Kinds::Listed(&[
            "application/x-tar",
            "application/x-compressed-tar",
            "application/x-xz-compressed-tar",
            "application/x-bzip2-compressed-tar",
            "application/x-zstd-compressed-tar",
            "application/gzip",
            "application/x-7z-compressed",
            "application/vnd.rar",
            "application/x-rar",
            "application/zstd",
        ]),
    },
    Role {
        id: "calendar",
        title: "Calendar invitations",
        glyph: ICON_CALENDAR,
        accent: Accent::Orange,
        key: "text/calendar",
        kinds: Kinds::Fixed(&[]),
    },
];

impl Role {
    /// The kinds choosing `app` makes it the default for.
    fn kinds_for(&self, app: &DesktopApp) -> Vec<String> {
        let mut kinds = vec![String::from(self.key)];
        let extra: Vec<String> = match &self.kinds {
            Kinds::Fixed(list) => list.iter().map(|kind| String::from(*kind)).collect(),
            Kinds::Listed(list) => list
                .iter()
                .filter(|kind| app.mime_types().iter().any(|own| own == *kind))
                .map(|kind| String::from(*kind))
                .collect(),
            Kinds::Prefix(prefix, but) => app
                .mime_types()
                .iter()
                .filter(|kind| kind.starts_with(prefix) && !but.contains(&kind.as_str()))
                .cloned()
                .collect(),
        };
        for kind in extra {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        kinds
    }
}

fn failed(error: String) {
    Dialog::new("settings.defaults.failed", "The default didn't change")
        .message(error)
        .button("OK", || {})
        .open("main");
}

/// A dropdown of `apps` showing `chosen`, or `unset` when it isn't among
/// them; picking one runs `pick` with it.
fn app_dropdown(
    id: String,
    apps: Vec<DesktopApp>,
    chosen: Option<&str>,
    unset: &str,
    pick: impl Fn(&DesktopApp) + 'static,
) -> Rectangle {
    let selected = chosen.and_then(|id| apps.iter().position(|app| app.id() == id));
    let items = apps
        .iter()
        .map(|app| MenuItem::new(String::from(app.name())))
        .collect();
    Dropdown::items(id, items, selected)
        .width(220.0)
        .fallback(unset)
        .on_select(move |index| {
            if let Some(app) = apps.get(index) {
                pick(app);
            }
        })
        .build()
}

/// The role's dropdown, or none when no app opens its kind.
fn role_dropdown(role: &'static Role) -> Option<Rectangle> {
    let apps = MimeApps::handlers(role.key);
    if apps.is_empty() {
        return None;
    }
    let chosen = MimeApps::default_app(role.key).map(|app| String::from(app.id()));
    Some(app_dropdown(
        format!("settings.defaults.{}", role.id),
        apps,
        chosen.as_deref(),
        "Not chosen",
        move |app| {
            if let Err(error) = MimeApps::set_default(&role.kinds_for(app), app.id()) {
                failed(error);
            }
        },
    ))
}

pub fn page() -> Column {
    let scanned = !Apps::read().all().is_empty();
    let mut roles = group().header("Open With");
    if !scanned {
        roles = roles.row(FormRow::new("settings.defaults.reading", "Looking for apps…").loading());
    } else {
        for role in &ROLES {
            let row = FormRow::new(format!("settings.defaults.{}-row", role.id), role.title)
                .icon(role.glyph, role.accent);
            roles = roles.row(match role_dropdown(role) {
                Some(dropdown) => row.trailing(dropdown),
                None => row.value("No app opens these"),
            });
        }
    }
    roles = roles.footer(
        "Images, video, music and text take every kind of file the app says it opens. Links, \
         files and folders opened from any app follow these.",
    );

    let mut list = vec![roles];
    if let Some(terminal) = terminal() {
        list.push(terminal);
    }
    let own = MimeApps::read().own().len();
    list.push(
        group().row(
            FormRow::new("settings.defaults.file-types", "File Types")
                .icon(ICON_FILE_TYPES, Accent::Gray)
                .subtitle("An app for any kind of file, and the ones you've chosen")
                .value(if own == 0 {
                    String::new()
                } else {
                    format!("{own} chosen")
                })
                .on_press(|| crate::open(View::Sub("file-types"))),
        ),
    );
    groups(list)
}

/// The page's sub-page: File Types.
pub fn sub(name: &str) -> Option<(String, Box<dyn Widget>)> {
    match name {
        "file-types" => Some((String::from("File Types"), boxed(file_types()))),
        _ => None,
    }
}

/// The kind `typed` names: a type as it is (`text/markdown`), else the
/// kind of a file with that extension (`md`, `.md` or `notes.md`).
fn kind_of(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    if typed.contains('/') {
        return Some(typed.to_lowercase());
    }
    let name = if typed.contains('.') && !typed.starts_with('.') {
        typed.to_string()
    } else {
        format!("file.{}", typed.trim_start_matches('.'))
    };
    Apps::mime_type(&name)
}

fn file_types() -> Column {
    let typed = TextInput::text(TYPE_FIELD);
    let mut search = group().header("Choose an App").row(
        FormRow::new("settings.defaults.type-row", "Kind of file")
            .subtitle("An extension, like md or png, or a type, like text/markdown")
            .trailing(field_box(TYPE_FIELD, text_field(TYPE_FIELD, "md"), 200.0)),
    );
    if let Some(kind) = kind_of(&typed) {
        search = search.row(kind_row("settings.defaults.found", &kind).child());
    } else if !typed.trim().is_empty() {
        search = search.row(
            FormRow::new(
                "settings.defaults.unknown",
                "No kind of file has that extension",
            )
            .child(),
        );
    }

    let own = MimeApps::read().own().to_vec();
    let mut chosen = group()
        .header("Chosen by You")
        .footer("Reset gives a kind back to the app the system picks.");
    if own.is_empty() {
        chosen = chosen.row(FormRow::new(
            "settings.defaults.none-chosen",
            "None yet: every kind opens with the system's choice",
        ));
    }
    for (kind, _) in own {
        let reset_kind = kind.clone();
        let reset = PushButton::new(format!("settings.defaults.reset.{kind}"), "Reset")
            .on_press(move || {
                if let Err(error) = MimeApps::reset(std::slice::from_ref(&reset_kind)) {
                    failed(error);
                }
            })
            .build();
        chosen =
            chosen.row(kind_row(&format!("settings.defaults.own.{kind}"), &kind).trailing(reset));
    }
    groups(vec![search, chosen])
}

/// `kind` with a dropdown of the apps that open it.
fn kind_row(id: &str, kind: &str) -> FormRow {
    let apps = MimeApps::handlers(kind);
    let row = FormRow::new(id, kind);
    if apps.is_empty() {
        return row.value("No app opens this");
    }
    let chosen = MimeApps::default_app(kind).map(|app| String::from(app.id()));
    let set = String::from(kind);
    row.trailing(app_dropdown(
        format!("{id}.app"),
        apps,
        chosen.as_deref(),
        "Not chosen",
        move |app| {
            if let Err(error) = MimeApps::set_default(std::slice::from_ref(&set), app.id()) {
                failed(error);
            }
        },
    ))
}

// ───── terminal ─────────────────────────────────────────────────────────────

/// Whether `xdg-terminal-exec` is on the PATH, looked for once.
fn terminal_exec() -> bool {
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| {
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| dir.join("xdg-terminal-exec").is_file())
        })
    })
}

fn terminals_file() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_default()
        .join("xdg-terminals.list")
}

/// The ids `xdg-terminals.list` lists, most wanted first.
fn terminals_listed() -> Vec<String> {
    fs::read_to_string(terminals_file())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect()
}

/// `id` first, then the others as they were.
fn choose_terminal(id: &str) -> Result<(), String> {
    let mut lines = vec![String::from(id)];
    lines.extend(terminals_listed().into_iter().filter(|line| line != id));
    let file = terminals_file();
    let temp = file.with_extension("list.new");
    fs::write(&temp, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", temp.display()))?;
    fs::rename(&temp, &file).map_err(|e| format!("{}: {e}", file.display()))
}

/// The terminal row, while `xdg-terminal-exec` is installed.
fn terminal() -> Option<amane_kit::form::Group> {
    if !terminal_exec() {
        return None;
    }
    let apps: Vec<DesktopApp> = Apps::read()
        .list()
        .iter()
        .filter(|app| app.categories().iter().any(|c| c == "TerminalEmulator"))
        .cloned()
        .collect();
    if apps.is_empty() {
        return None;
    }
    let listed = terminals_listed();
    let chosen = listed
        .iter()
        .find(|id| apps.iter().any(|app| app.id() == id.as_str()))
        .cloned();
    let dropdown = app_dropdown(
        String::from("settings.defaults.terminal"),
        apps,
        chosen.as_deref(),
        "The first one found",
        |app| {
            if let Err(error) = choose_terminal(app.id()) {
                failed(error);
            }
        },
    );
    Some(
        group()
            .row(
                FormRow::new("settings.defaults.terminal-row", "Terminal")
                    .icon(ICON_TERMINAL, Accent::Gray)
                    .trailing(dropdown),
            )
            .footer("What Super+T and Super+Return open, and apps that ask for a terminal."),
    )
}
