# amane-shell

A Wayland desktop shell for the [niri](https://github.com/YaLTeR/niri) compositor, built on the
[amane](https://github.com/098765434/amane) shell library (a fork). It has a status bar on every
monitor, at the top or the bottom, in three sizes (Compact, Regular, Comfortable) and four styles
(Frosted, Glass, Solid, Clear); a dock that rises from the bottom edge and with niri's overview;
and glass floating surfaces (context menus, Quick Settings, the launcher, notification toasts and
the notification center), all blurred by the compositor.

> **Built for a forked niri**: [098765434/niri](https://github.com/098765434/niri). The generated
> layer rules use options only the fork has (named `materials {}`, `background-effect { material }`
> and the `desktop` layer the widgets sit on), the dock rises with the overview from the fork's
> live overview progress, and dragging a window's icon in the dock to reorder it needs
> `MoveColumnToIndex` with a `window_id` (stock niri would move the focused column instead). What
the forks add is in their docs: [niri](https://github.com/098765434/niri/blob/ocbk/docs/fork/README.md), [amane](https://github.com/098765434/amane/blob/ocbk/docs/fork/README.md).

## Features

- **Status bar**: workspace pips, the status icons, Quick Settings, notifications and the clock,
  one bar a monitor, top or bottom, in three sizes and four styles
  ([docs/shell/status-bar.md](docs/shell/status-bar.md)).
- **Status apps and Activities**: other programs' items in the bar and what's going on now
  (Now Playing, the timer, screen sharing, uploads), over `org.amane.StatusBar1`
  ([docs/integration/status-apps.md](docs/integration/status-apps.md), and
  [writing one](docs/integration/writing-a-status-app.md)).
- **Drop**: `amane-drop`, a drop shelf in the bar: drop files on folders, apps, your phone
  or an upload (0x0.st, Imgur, SFTP, S3), with a Drop Bar to hold them
  ([docs/shell/drop.md](docs/shell/drop.md)).
- **Dock**: every workspace's windows and pinned apps, rising from the bottom edge and with the
  overview; drag to reorder or move, drop files to open them ([docs/shell/dock.md](docs/shell/dock.md)).
- **Launcher**: a search pill, with Applications, Files, Actions and a clipboard
  history ([docs/shell/launcher.md](docs/shell/launcher.md)).
- **Notifications**: the shell is the notification daemon: toasts, a notification center and
  Do Not Disturb ([docs/shell/notifications.md](docs/shell/notifications.md)).
- **Quick Settings**: GNOME's quick settings as a grid of glass modules, with a page per
  control and Edit Controls ([docs/shell/quick-settings.md](docs/shell/quick-settings.md)).
- **Desktop widgets**: a stroke-drawn clock, batteries and the system's readings on the wallpaper
  ([docs/shell/widgets.md](docs/shell/widgets.md)).
- **Context menus**: nested menus on xdg-popups ([docs/shell/context-menu.md](docs/shell/context-menu.md)).
- **Shell Settings**: a settings app the shell follows live ([docs/shell/settings-app.md](docs/shell/settings-app.md)).
- **amane-kit**: the look as a library: controls, components and the design tokens from Penpot,
  for the shell and apps like Shell Settings ([kit/README.md](kit/README.md)).

Every doc is listed in [docs/README.md](docs/README.md).

## Build & run

A Nix flake (`flake.nix`) provides a devShell with the Rust toolchain and the required
wayland/vulkan/pulse/fontconfig libraries wired up (no hardcoded store paths), so plain `cargo`
works inside it:

```sh
nix develop -c cargo build          # compile
nix develop -c cargo run            # run  (follows the system light/dark preference)
nix develop -c env AMANE_THEME=light cargo run   # force the light theme
```

The theme follows the system preference (the Settings portal), else dark. `AMANE_THEME=light|dark`
forces one, `AMANE_DENSITY=compact` tightens spacing and rows, and `AMANE_TEXT_SCALE` (0.5–3)
scales text.

For a release binary, `nix build` produces `./result/bin/amane-shell`, wrapped with the runtime
libraries it needs (so it runs standalone).

The `amane` library is a **git dependency** pinned to a rev (`Cargo.toml`). To develop the shell and
the library together, check `amane` out beside this repo and add a git-ignored
`.cargo/config.toml` with `paths = ["../amane"]` — see [CONTRIBUTING.md](CONTRIBUTING.md).

> **Do not** build this shell with the upstream `amane` CLI (`amane dev`/`compile`): it compiles
> against its own bundled library copy, **not** this fork (so it lacks `Gestures` and won't compile
> the bar). The flake is the one canonical build path.

## niri layer rules

The shell's glass is frosted by niri, through `layer-rule` blocks generated from the design
tokens into `niri-layer-rule.kdl` and injected into your niri config by `./gen-layer-rules.sh`:

```sh
cargo run -q -- layer-rules > niri-layer-rule.kdl
./gen-layer-rules.sh /path/to/your/niri/config.kdl   # e.g. ~/.config/niri/config.kdl
```

See [docs/integration/layer-rules.md](docs/integration/layer-rules.md).

## Running it persistently

Autostart the bar with your compositor/session. A **systemd user service** is the robust option
(crash recovery + logs); as a home-manager snippet:

```nix
systemd.user.services.amane-shell = {
  Unit = { Description = "amane-shell bar"; PartOf = [ "graphical-session.target" ]; After = [ "graphical-session.target" ]; };
  Service = { ExecStart = "${pkgs.amane-shell}/bin/amane-shell"; Restart = "on-failure"; };
  Install.WantedBy = [ "graphical-session.target" ];
};
```

Or, more simply, a niri `spawn-at-startup "amane-shell"` line (no crash recovery).

Run status apps such as `amane-drop` as units of their own, not from the shell: a restart of the
shell kills what's in its cgroup, and they put their items back by themselves once the bar is
there ([docs/shell/drop.md](docs/shell/drop.md#installing)).

## Shell Settings

`amane-settings` is a settings app for the shell, built on the [kit](kit/README.md): appearance
(light or dark, accent, density, text size, glass, adaptive glass, vibrant labels, motion), the
desktop's widgets and the launcher. It writes `~/.config/amane-shell/*.json`, which the running shell
follows at once. Its **System** pages manage the NixOS configuration the machine is built from
(the computer's name, language and keyboard; apps, services, idle timeouts, storage, history,
updates with a scheduled check), through [docs/integration/nixos.md](docs/integration/nixos.md),
and the shell is the session's polkit agent, so the password prompt is its own. See
[docs/shell/settings-app.md](docs/shell/settings-app.md).

## Layout

`src/` is the shell, `kit/` is amane-kit (the look as a library, for the shell and its apps),
`apps/` the programs beside it, and `design-tokens/tokens.json` the Penpot export. Where each
part lives, folder by folder, is in the source-layout table of [docs/README.md](docs/README.md).

## License

MIT — see [LICENSE](LICENSE). The bar links the `amane` library (also MIT) and talks to niri over
**IPC**, so the GPL-3.0 niri fork imposes no copyleft obligation on this crate.
