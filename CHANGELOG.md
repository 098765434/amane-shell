# Changelog

All notable changes to emaki (amane-shell until 2026-11) are recorded here.

The format is [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

<!-- one line per change, newest last -->

- **BREAKING: amane-shell is emaki.** The amane library underneath keeps its name, and its
  own `AMANE_*` variables (`AMANE_DEBUG`, `AMANE_FRAMES`, `AMANE_VELLO_*`, …). Renamed:
  - binaries: `amane-shell` is `emaki`; `amane-settings`, `amane-messages`, `amane-mirror`,
    `amane-shader-lab`, `amane-drop`, `amane-assistant`, `amane-pinentry` and
    `amane-train-watch` are `emaki-…`
  - packages and crates: `amane-shell` is `emaki`, `amane-kit` is `emaki-kit`, and the other
    `amane-*` crates are `emaki-*` (`emaki_kit::` in Rust); the flake's package is
    `packages.<system>.emaki` (and `default`)
  - app ids and desktop files: `amane-settings.desktop` and the others are `emaki-*.desktop`,
    the icons `emaki-settings` and `emaki-messages`
  - environment variables the shell reads: `AMANE_THEME`, `AMANE_DENSITY`, `AMANE_LOG`,
    `AMANE_RBW`, `AMANE_ASSISTANT_MODELS` and the rest are `EMAKI_*`
  - sockets: `$XDG_RUNTIME_DIR/amane.sock`, `amane-drop.sock`, `amane-assistant.sock` and
    `amane-drop/` are `emaki.sock`, `emaki-drop.sock`, `emaki-assistant.sock` and `emaki-drop/`
  - D-Bus: `org.amane.StatusBar1` at `/org/amane/StatusBar` is `org.emaki.StatusBar1` at
    `/org/emaki/StatusBar`, with no alias; item ids `org.amane.*` are `org.emaki.*`
  - layer namespaces: `amane.dock`, `amane.status-bar`, `amane.launcher` and the others are
    `emaki.*`; the niri config's markers are `// >>> emaki layer-rules` and
    `// <<< emaki layer-rules`
  - folders: `~/.config/amane-shell`, `~/.local/state/amane-shell` and `~/.cache/amane-shell`
    are `…/emaki`, the `amane-*` ones `emaki-*`, `~/.cache/amane/messages` is
    `~/.cache/emaki/phone/messages`, `~/.local/share/amane-assistant` is
    `~/.local/share/emaki-assistant`
  - the settings app: Shell Settings is **Emaki Settings**
  - NixOS: the flake input `amane-shell` is `emaki`, the options `amane.*` are `emaki.*`, the
    flake output `amaneSettings` is `emakiSettings`, `amane-nixos-activate` and its polkit
    action `org.amane.nixos-activate` are `emaki-nixos-activate` and `org.emaki.nixos-activate`,
    the units `amane-shell.service`, `amane-drop.service` and `amane-update-check` are
    `emaki.service`, `emaki-drop.service` and `emaki-update-check`

### Added

- The move from amane-shell, once, on each program's first start: the old folders move to the
  new names, the ids the settings keep (`org.amane.*`, `amane-*.desktop`) are renamed, and
  `~/.config/emaki/.migrated-from-amane` marks it done; keyring passwords under the old names
  are still found. `scripts/migrate-from-amane` does the moves by hand and renames the niri
  config's markers and sockets ([troubleshooting](docs/user/troubleshooting.md#migrating-from-amane-shell)).
- Emaki Settings reads back a `displays.kdl` that Shell Settings wrote.

## [0.1.0] - 2026-11-08

The first tagged release: the desktop as it runs today.

### Added

- **Status bar**: one bar a monitor, top or bottom, in three sizes (Compact, Regular,
  Comfortable) and four styles (Frosted, Glass, Solid, Clear), with the workspace's shape,
  status icons, the tray, programs' items, Activities, a clock and weather; auto-hide modes.
- **Dock**: every workspace's windows and pinned apps, rising from the bottom edge and with
  niri's overview; drag to reorder or move, drop files to open them; a reveal strip when hidden.
- **Launcher**: a search pill with Applications, Files, Actions, a clipboard history and
  Passwords (Bitwarden through rbw), and the on-device **assistant** (Cactus: Needle 3 for
  commands, Whistle for speech) in the socket-activated `amane-assistant` daemon.
- **Notifications**: the shell is the notification daemon: toasts, a notification center with a
  month calendar and a history kept across restarts, per-app rules and Do Not Disturb.
- **Quick Settings**: a grid of glass modules with a page per control (sound, Wi-Fi, VPN,
  Bluetooth with pairing, power, Night Light, keyboard, session, Tailscale, Keep Awake, the
  phone) and Edit Controls.
- **Desktop widgets** on niri's desktop layer: a stroke-drawn clock, batteries and the system's
  readings, on a shared grid with Edit Widgets.
- **On-screen display** for volume, brightness, the keyboard light, lock keys and switches.
- **Drop** (`amane-drop`): a drop shelf in the bar: folders, apps, the phone and uploads
  (0x0.st, Imgur, SFTP, S3), with a Drop Bar to hold files.
- **Shell Settings** (`amane-settings`): appearance, status bar, dock, Quick Settings,
  notifications, launcher, passwords, Drop, **Displays** (resolution, refresh, scale, rotation,
  arrangement, HDR, DDC/CI brightness, Night Light) and the **System** pages that manage the
  machine's NixOS configuration (General, Apps, Services, Power, Storage, Updates) through
  polkit; the shell is the session's polkit agent.
- **Messages** (`amane-messages`) and **Phone Screen** (`amane-mirror`): KDE Connect texts,
  attachments and remote input, and the phone's screen through scrcpy.
- **Shader Lab** (`amane-shader-lab`): shader effects tried live, with saved looks the kit
  can put on any rectangle.
- **amane-kit**: the shell's look as a library: the Penpot design tokens, theme (mode, accent,
  density, text scale, glass), controls, containers, motion, an app window, settings files.
- **Status apps**: `org.amane.StatusBar1` on the session bus, with the `amane-status` crate,
  so any program can put an item and Activities in the bar.
- The shell's IPC socket (`$XDG_RUNTIME_DIR/amane.sock`), niri layer rules generated from the
  tokens, and a Nix flake that builds every program with its runtime wiring.

[Unreleased]: https://github.com/098765434/amane-shell/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/098765434/amane-shell/releases/tag/v0.1.0
