//! `nixos.json`: the NixOS configuration the System pages manage.

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

settings_file!(Nixos, crate::app_ids::SHELL, "nixos", nixos, change_nixos);

// ───── Drop (amane-drop) ────────────────────────────────────────────────────
