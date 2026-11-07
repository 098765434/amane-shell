//! `nixos.json`: the NixOS configuration the System pages manage.

#![allow(missing_docs)]

use super::*;

/// Where the NixOS configuration Shell Settings manages lives: its flake
/// repo and the `nixosConfigurations` attribute to build.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(crate = "amane::serde", default)]
pub struct Nixos {
    /// The flake repo; none for `~/nixos-config`.
    pub repo: Option<PathBuf>,

    /// The `nixosConfigurations` attribute; empty for `nixos-niri`.
    pub host: String,
}

impl Nixos {
    /// The repo, `~/nixos-config` unless set.
    pub fn repo_path(&self) -> PathBuf {
        self.repo.clone().unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("nixos-config")
        })
    }

    /// The attribute to build, `nixos-niri` unless set (it's the flake's
    /// name for the machine, not its hostname).
    pub fn host_attr(&self) -> String {
        if self.host.trim().is_empty() {
            String::from("nixos-niri")
        } else {
            self.host.trim().to_string()
        }
    }
}

impl Settings for Nixos {
    const APP: &'static str = "amane-shell";
    const FILE: &'static str = "nixos";
}

/// The NixOS settings as they are now; a view that reads them redraws when
/// they change.
pub fn nixos() -> RwLockReadGuard<'static, Config<Nixos>> {
    Config::<Nixos>::read()
}

/// Changes the NixOS settings and saves them, for input handlers.
pub fn change_nixos(edit: impl FnOnce(&mut Nixos)) {
    Config::<Nixos>::change(edit);
}

// ───── Drop (amane-drop) ────────────────────────────────────────────────────
