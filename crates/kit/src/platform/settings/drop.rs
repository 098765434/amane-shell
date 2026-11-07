//! `amane-drop`'s `grids.json` and `sharing.json`: Drop's tiles and where it uploads.

#![allow(missing_docs)]

use super::*;

/// What a folder tile does with what's let go on it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "lowercase")]
pub enum FolderMode {
    #[default]
    Copy,
    Move,
}

/// A tile of a Drop grid: somewhere to let files go, or something to do
/// with them (an action).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case", tag = "kind")]
pub enum DropTile {
    /// Copies (or moves) what's let go into the folder; a click opens it.
    Folder {
        path: PathBuf,
        #[serde(default)]
        mode: FolderMode,
    },
    /// Opens what's let go with the app, by its desktop file id.
    App { id: String },
    /// Copies into a folder chosen once.
    CopyTo {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Moves into a folder chosen once.
    MoveTo {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Asks for a name, then moves the file there.
    RenameMove {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Saves text let go as a file there.
    SaveText {
        #[serde(default)]
        folder: Option<PathBuf>,
    },
    /// Sends what's let go to a phone over KDE Connect: the one paired, or
    /// the one asked for when there are more.
    Phone {
        #[serde(default)]
        device: Option<String>,
    },
    /// Uploads what's let go and copies its link.
    Upload { to: UploadTo },
}

/// Where an upload tile sends files.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(crate = "amane::serde", rename_all = "kebab-case")]
pub enum UploadTo {
    /// 0x0.st, the null pointer: no account, kept a while, then gone.
    #[serde(rename = "0x0")]
    ZeroX0,
    Imgur,
    Sftp,
    S3,
}

impl UploadTo {
    pub const ALL: [Self; 4] = [Self::ZeroX0, Self::Imgur, Self::Sftp, Self::S3];

    /// Its name on a tile and in Settings.
    pub fn title(self) -> &'static str {
        match self {
            Self::ZeroX0 => "0x0.st",
            Self::Imgur => "Imgur",
            Self::Sftp => "SFTP",
            Self::S3 => "S3",
        }
    }
}

/// One grid of tiles; each has a Drop Bar of its own.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct DropGrid {
    pub name: String,
    pub tiles: Vec<DropTile>,
}

impl Default for DropGrid {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let mut tiles = Vec::new();
        for folder in ["Downloads", "Desktop"] {
            let path = home.join(folder);
            if path.is_dir() {
                tiles.push(DropTile::Folder {
                    path,
                    mode: FolderMode::Copy,
                });
            }
        }
        tiles.extend([
            DropTile::CopyTo { folder: None },
            DropTile::MoveTo { folder: None },
            DropTile::RenameMove { folder: None },
            DropTile::SaveText { folder: None },
        ]);
        Self {
            name: String::from("Grid"),
            tiles,
        }
    }
}

/// amane-drop's grids, `~/.config/amane-drop/grids.json`, followed live by
/// the app and Shell Settings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct DropGrids {
    pub grids: Vec<DropGrid>,
    /// Tiles a row.
    pub columns: u8,
}

impl Default for DropGrids {
    fn default() -> Self {
        Self {
            grids: vec![DropGrid::default()],
            columns: 4,
        }
    }
}

impl Settings for DropGrids {
    const APP: &'static str = "amane-drop";
    const FILE: &'static str = "grids";
}

impl DropGrids {
    /// Tiles a row, 3 to 6.
    pub fn columns(&self) -> usize {
        usize::from(self.columns.clamp(3, 6))
    }
}

/// The grids as they are now; a view that reads them redraws when they change.
pub fn drop_grids() -> RwLockReadGuard<'static, Config<DropGrids>> {
    Config::<DropGrids>::read()
}

/// Changes the grids and saves them, for input handlers.
pub fn change_drop_grids(edit: impl FnOnce(&mut DropGrids)) {
    Config::<DropGrids>::change(edit);
}

/// The SFTP server Drop uploads to. A password, when there's no key, is
/// kept in the keyring as "sftp".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct SftpAccount {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// The folder files go in, on the server.
    pub folder: String,
    /// Where they can be read on the web, which a file's name is put after
    /// for its link; none, and an upload gives no link.
    pub url: String,
    /// A private key to log in with, else the password.
    pub key: Option<PathBuf>,
}

impl Default for SftpAccount {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            user: String::new(),
            folder: String::new(),
            url: String::new(),
            key: None,
        }
    }
}

/// The S3 bucket Drop uploads to, at AWS or any S3-compatible store. The
/// secret access key is kept in the keyring as "s3".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(crate = "amane::serde", default)]
pub struct S3Account {
    /// Another store's address (`https://<id>.r2.cloudflarestorage.com`),
    /// none for AWS.
    pub endpoint: String,
    /// `us-east-1` when none; R2's is `auto`.
    pub region: String,
    pub bucket: String,
    /// Put before each file's name: a folder in the bucket.
    pub prefix: String,
    pub key_id: String,
    /// Where the bucket can be read on the web, for links; none, and the
    /// bucket's own address is given.
    pub url: String,
    /// Uploads readable by anyone (`public-read`).
    pub public: bool,
}

/// amane-drop's accounts and what it does with a link,
/// `~/.config/amane-drop/sharing.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct DropSharing {
    pub sftp: SftpAccount,
    pub s3: S3Account,
    /// Imgur's client id for an app registered there (anonymous uploads).
    pub imgur_client_id: String,
    /// An upload's link goes on the clipboard.
    pub copy_link: bool,
    /// A notification says it's done.
    pub notify: bool,
}

impl Default for DropSharing {
    fn default() -> Self {
        Self {
            sftp: SftpAccount::default(),
            s3: S3Account::default(),
            imgur_client_id: String::new(),
            copy_link: true,
            notify: true,
        }
    }
}

impl Settings for DropSharing {
    const APP: &'static str = "amane-drop";
    const FILE: &'static str = "sharing";
}

impl DropSharing {
    /// What `to` still needs before it can upload, in a few words; none
    /// when it's ready. Secrets are checked when it runs.
    pub fn missing(&self, to: UploadTo) -> Option<&'static str> {
        match to {
            UploadTo::ZeroX0 => None,
            UploadTo::Imgur if self.imgur_client_id.trim().is_empty() => Some("No client ID"),
            UploadTo::Imgur => None,
            UploadTo::Sftp if self.sftp.host.trim().is_empty() => Some("No server"),
            UploadTo::Sftp if self.sftp.user.trim().is_empty() => Some("No user name"),
            UploadTo::Sftp => None,
            UploadTo::S3 if self.s3.bucket.trim().is_empty() => Some("No bucket"),
            UploadTo::S3 if self.s3.key_id.trim().is_empty() => Some("No access key"),
            UploadTo::S3 => None,
        }
    }
}

/// The accounts as they are now.
pub fn drop_sharing() -> RwLockReadGuard<'static, Config<DropSharing>> {
    Config::<DropSharing>::read()
}

/// Changes the accounts and saves them.
pub fn change_drop_sharing(edit: impl FnOnce(&mut DropSharing)) {
    Config::<DropSharing>::change(edit);
}
