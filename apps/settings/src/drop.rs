//! Drop: emaki-drop's grids, what an upload's link does, and the accounts
//! it uploads to. The grids are `~/.config/emaki-drop/grids.json` and the
//! accounts `sharing.json`, both followed by the app at once; passwords
//! and keys go in the keyring (`emaki_kit::secrets`), never in a file
//! unless no keyring answers.

use amane::{Center, Column, FileDialog, Row, TextInput, Widget};
use emaki_kit::app::{Dialog, commit_on_blur, field_box, text_field};
use emaki_kit::buttons::{IconButton, PushButton, Style};
use emaki_kit::dialog::Field;
use emaki_kit::dropdown::Dropdown;
use emaki_kit::form::FormRow;
use emaki_kit::icons::ICON_CLOSE;
use emaki_kit::layout::boxed;
use emaki_kit::secrets;
use emaki_kit::settings::{
    DropSharing, UploadTo, change_drop_grids, change_drop_sharing, drop_grids, drop_sharing,
};

use crate::{group, groups, switch_row};

const APP: &str = "emaki-drop";

/// `expand_less` and `expand_more`, to move a grid up or down.
const ICON_EARLIER: char = '\u{e5ce}';
const ICON_LATER: char = '\u{e5cf}';

/// The rename dialog's field.
const RENAME: &str = "settings.drop.rename.field";

const FIELD_WIDTH: f32 = 240.0;

pub fn page() -> Column {
    let sharing = drop_sharing().clone();
    let mut pages = vec![grids(), after(&sharing)];
    pages.extend(accounts(&sharing));
    groups(pages)
}

// ───── grids ─────────────────────────────────────────────────────────────────

fn grids() -> emaki_kit::form::Group {
    let grids = drop_grids().clone();
    let last = grids.grids.len().saturating_sub(1);
    let many = grids.grids.len() > 1;
    let rows: Vec<FormRow> = grids
        .grids
        .iter()
        .enumerate()
        .map(|(index, grid)| {
            let id = format!("settings.drop.grid.{index}");
            let mut trailing: Vec<Box<dyn Widget>> = vec![
                boxed(
                    PushButton::new(format!("{id}.rename"), "Rename…")
                        .on_press(move || rename(index))
                        .build(),
                ),
                boxed(
                    IconButton::new(format!("{id}.earlier"), ICON_EARLIER)
                        .disabled(index == 0)
                        .on_press(move || {
                            change_drop_grids(|grids| grids.grids.swap(index, index - 1));
                        })
                        .build(),
                ),
                boxed(
                    IconButton::new(format!("{id}.later"), ICON_LATER)
                        .disabled(index == last)
                        .on_press(move || {
                            change_drop_grids(|grids| grids.grids.swap(index, index + 1));
                        })
                        .build(),
                ),
            ];
            if many {
                let name = grid.name.clone();
                trailing.push(boxed(
                    IconButton::new(format!("{id}.delete"), ICON_CLOSE)
                        .style(Style::Destructive)
                        .on_press(move || delete(index, &name))
                        .build(),
                ));
            }
            let tiles = emaki_kit::format::count(grid.tiles.len(), "tile", "tiles");
            FormRow::new(id, &grid.name)
                .subtitle(tiles)
                .trailing(Row::new(trailing).gap(4.0).align(Center))
        })
        .collect();

    let columns = Dropdown::new(
        "settings.drop.columns",
        ["3", "4", "5", "6"].map(String::from).to_vec(),
        grids.columns() - 3,
    )
    .width(80.0)
    .on_select(|index| change_drop_grids(|grids| grids.columns = index as u8 + 3));
    let add = PushButton::new("settings.drop.add-grid", "Add Grid")
        .on_press(|| {
            change_drop_grids(|grids| {
                let name = format!("Grid {}", grids.grids.len() + 1);
                grids.grids.push(emaki_kit::settings::DropGrid {
                    name,
                    tiles: Vec::new(),
                });
            });
        })
        .build();
    let open = PushButton::new("settings.drop.open", "Open Drop")
        .on_press(|| amane::spawn("emaki-drop open"))
        .build();

    group()
        .header("Grids")
        .rows(rows)
        .row(FormRow::new("settings.drop.columns", "Tiles a row").trailing(columns.build()))
        .row(
            FormRow::new("settings.drop.add", "Grids and their tiles")
                .subtitle("Tiles are added and removed in the grid, with Edit")
                .trailing(Row::new(vec![boxed(add), boxed(open)]).gap(6.0)),
        )
        .footer(
            "Drop's item in the status bar opens its grid, as does Super+Shift+D; files dragged \
             over the item open it at once. Each grid has a Drop Bar of its own.",
        )
}

fn rename(index: usize) {
    let now = drop_grids()
        .grids
        .get(index)
        .map(|grid| grid.name.clone())
        .unwrap_or_default();
    TextInput::set_text(RENAME, &now);
    Dialog::new("settings.drop.rename", "Rename the grid")
        .field(Field {
            id: RENAME,
            placeholder: String::from("Name"),
            secret: false,
        })
        .button("Cancel", || {})
        .main_button("Rename", move || {
            let name = TextInput::text(RENAME).trim().to_string();
            if !name.is_empty() {
                change_drop_grids(|grids| {
                    if let Some(grid) = grids.grids.get_mut(index) {
                        grid.name = name;
                    }
                });
            }
        })
        .open("main");
}

fn delete(index: usize, name: &str) {
    Dialog::new("settings.drop.delete", format!("Delete “{name}”?"))
        .message("Its tiles go with it. The files on its Drop Bar stay where they are.")
        .button("Cancel", || {})
        .destructive("Delete", move || {
            change_drop_grids(|grids| {
                if grids.grids.len() > 1 && index < grids.grids.len() {
                    grids.grids.remove(index);
                }
            });
        })
        .open("main");
}

// ───── links ─────────────────────────────────────────────────────────────────

fn after(sharing: &DropSharing) -> emaki_kit::form::Group {
    group()
        .header("After an upload")
        .row(switch_row(
            "settings.drop.copy-link",
            "Copy the link",
            sharing.copy_link,
            |on| change_drop_sharing(|sharing| sharing.copy_link = on),
        ))
        .row(switch_row(
            "settings.drop.notify",
            "Show a notification",
            sharing.notify,
            |on| change_drop_sharing(|sharing| sharing.notify = on),
        ))
        .footer(
            "Links also go under Recently Shared in Drop's menu, the last ten; picking one copies \
             it again.",
        )
}

// ───── accounts ──────────────────────────────────────────────────────────────

fn accounts(sharing: &DropSharing) -> Vec<emaki_kit::form::Group> {
    let in_file = secrets::kept_in_file(APP);
    let mut s3_footer = String::from(
        "Any S3-compatible store: give its endpoint (R2's region is “auto”). The web address, \
         when the bucket is served from one, is what links begin with.",
    );
    if in_file {
        s3_footer.push_str(
            " No keyring answered, so passwords are in ~/.config/emaki-drop/secrets.json, \
             readable only by you.",
        );
    }
    vec![
        group()
            .header("Phone")
            .row(
                FormRow::new("settings.drop.phone", "Send to Phone")
                    .subtitle("Through KDE Connect; pair a phone in Quick Settings › Phone")
                    .value("No setup"),
            )
            .footer("Files land in the phone's downloads; text and links open on it."),
        group()
            .header("0x0.st")
            .row(
                FormRow::new("settings.drop.0x0", "0x0.st")
                    .subtitle(status(sharing, UploadTo::ZeroX0))
                    .value("No account"),
            )
            .footer(
                "Anyone with the link can see the file. It's kept from 30 days to a year, the \
                 bigger the shorter.",
            ),
        group()
            .header("Imgur")
            .row(field(
                "settings.drop.imgur.id",
                "Client ID",
                "From api.imgur.com/oauth2/addclient",
                |sharing| sharing.imgur_client_id.clone(),
                |sharing, text| sharing.imgur_client_id = text,
            ))
            .footer("Pictures and videos, uploaded without an account."),
        group()
            .header("SFTP")
            .row(field(
                "settings.drop.sftp.host",
                "Server",
                "example.org",
                |sharing| sharing.sftp.host.clone(),
                |sharing, text| sharing.sftp.host = text,
            ))
            .row(field(
                "settings.drop.sftp.port",
                "Port",
                "22",
                |sharing| sharing.sftp.port.to_string(),
                |sharing, text| sharing.sftp.port = text.parse().unwrap_or(22),
            ))
            .row(field(
                "settings.drop.sftp.user",
                "User name",
                "",
                |sharing| sharing.sftp.user.clone(),
                |sharing, text| sharing.sftp.user = text,
            ))
            .row(field(
                "settings.drop.sftp.folder",
                "Folder",
                "public_html/files",
                |sharing| sharing.sftp.folder.clone(),
                |sharing, text| sharing.sftp.folder = text,
            ))
            .row(field(
                "settings.drop.sftp.url",
                "Web address",
                "https://example.org/files",
                |sharing| sharing.sftp.url.clone(),
                |sharing, text| sharing.sftp.url = text,
            ))
            .row(key_row(sharing))
            .row(secret_row(
                "settings.drop.sftp.password",
                if sharing.sftp.key.is_some() {
                    "Key passphrase"
                } else {
                    "Password"
                },
                "sftp",
                "Drop: SFTP",
            ))
            .footer(
                "The server has to be known first: connect once with ssh, so it's in \
                 ~/.ssh/known_hosts. Without a web address an upload gives no link.",
            ),
        group()
            .header("S3")
            .row(field(
                "settings.drop.s3.endpoint",
                "Endpoint",
                "None for AWS",
                |sharing| sharing.s3.endpoint.clone(),
                |sharing, text| sharing.s3.endpoint = text,
            ))
            .row(field(
                "settings.drop.s3.region",
                "Region",
                "us-east-1",
                |sharing| sharing.s3.region.clone(),
                |sharing, text| sharing.s3.region = text,
            ))
            .row(field(
                "settings.drop.s3.bucket",
                "Bucket",
                "",
                |sharing| sharing.s3.bucket.clone(),
                |sharing, text| sharing.s3.bucket = text,
            ))
            .row(field(
                "settings.drop.s3.prefix",
                "Folder",
                "None",
                |sharing| sharing.s3.prefix.clone(),
                |sharing, text| sharing.s3.prefix = text,
            ))
            .row(field(
                "settings.drop.s3.key-id",
                "Access key ID",
                "",
                |sharing| sharing.s3.key_id.clone(),
                |sharing, text| sharing.s3.key_id = text,
            ))
            .row(secret_row(
                "settings.drop.s3.secret",
                "Secret access key",
                "s3",
                "Drop: S3",
            ))
            .row(field(
                "settings.drop.s3.url",
                "Web address",
                "The bucket's own",
                |sharing| sharing.s3.url.clone(),
                |sharing, text| sharing.s3.url = text,
            ))
            .row(switch_row(
                "settings.drop.s3.public",
                "Anyone can read uploads",
                sharing.s3.public,
                |on| change_drop_sharing(|sharing| sharing.s3.public = on),
            ))
            .footer(s3_footer),
    ]
}

/// "Ready", or what `to` still needs.
fn status(sharing: &DropSharing, to: UploadTo) -> String {
    sharing
        .missing(to)
        .map_or_else(|| String::from("Ready"), String::from)
}

/// A row with a text field that keeps what's typed on Return or once it
/// loses the keys.
fn field(
    id: &'static str,
    title: &str,
    placeholder: &str,
    get: fn(&DropSharing) -> String,
    set: fn(&mut DropSharing, String),
) -> FormRow {
    emaki_kit::app::fill_once(id, || get(&drop_sharing()));
    let keep = move |text: String| {
        let text = text.trim().to_string();
        if text != get(&drop_sharing()) {
            change_drop_sharing(|sharing| set(sharing, text));
        }
    };
    commit_on_blur(id, keep);
    let input = text_field(id, placeholder).on_submit(keep);
    FormRow::new(format!("{id}.row"), title).trailing(field_box(id, input, FIELD_WIDTH))
}

/// A password or key, typed and then kept in the keyring: the field
/// empties once it's kept, and Forget lets it go.
fn secret_row(id: &'static str, title: &str, name: &'static str, label: &'static str) -> FormRow {
    let kept = secrets::known(APP, name);
    let keep = move |text: String| {
        if text.is_empty() {
            return;
        }
        TextInput::set_text(id, "");
        std::thread::spawn(move || {
            secrets::store(APP, name, label, &text);
        });
    };
    commit_on_blur(id, keep);
    let input = text_field(
        id,
        match kept {
            Some(true) => "Kept",
            Some(false) => "None kept",
            None => "",
        },
    )
    .password()
    .on_submit(keep);
    let mut trailing: Vec<Box<dyn Widget>> = vec![boxed(field_box(id, input, FIELD_WIDTH))];
    if kept == Some(true) {
        trailing.insert(
            0,
            boxed(
                PushButton::new(format!("{id}.forget"), "Forget")
                    .on_press(move || {
                        std::thread::spawn(move || secrets::forget(APP, name));
                    })
                    .build(),
            ),
        );
    }
    FormRow::new(format!("{id}.row"), title)
        .subtitle(match kept {
            Some(true) => "In the keyring",
            _ => "Typed, then kept in the keyring",
        })
        .trailing(Row::new(trailing).gap(6.0).align(Center))
}

/// The private key SFTP logs in with, instead of a password.
fn key_row(sharing: &DropSharing) -> FormRow {
    let choose = PushButton::new("settings.drop.sftp.key.choose", "Choose…")
        .on_press(|| {
            FileDialog::open("Choose a Private Key").show(|picked| {
                if let Some(key) = picked.into_iter().next() {
                    change_drop_sharing(|sharing| sharing.sftp.key = Some(key));
                }
            });
        })
        .build();
    let mut trailing: Vec<Box<dyn Widget>> = vec![boxed(choose)];
    if sharing.sftp.key.is_some() {
        trailing.push(boxed(
            IconButton::new("settings.drop.sftp.key.clear", ICON_CLOSE)
                .on_press(|| change_drop_sharing(|sharing| sharing.sftp.key = None))
                .build(),
        ));
    }
    FormRow::new("settings.drop.sftp.key", "Private key")
        .subtitle(sharing.sftp.key.as_ref().map_or_else(
            || String::from("None: the password is used"),
            |key| key.display().to_string(),
        ))
        .trailing(Row::new(trailing).gap(4.0).align(Center))
}
