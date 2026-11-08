# emaki

**A complete desktop for niri.** emaki is a Wayland desktop shell built for the
[niri](https://github.com/niri-wm/niri) scrollable-tiling compositor. It covers everything a
desktop needs beyond window management — bar, dock, launcher, notifications, settings — plus a
set of first-party apps, all drawn with the same GPU-rendered design library. The shell is
written in Rust on top of [amane](https://github.com/098765434/amane) and
[vello](https://github.com/098765434/vello), and ships with a
[niri fork](https://github.com/098765434/niri) that adds the compositor features the shell
relies on.

- **Shell:** a status bar on every monitor, a dock that rises with the overview, a launcher,
  notifications, Quick Settings, desktop widgets and an on-screen display
- **Apps:** Emaki Settings, Messages, Phone Screen, Shader Lab, Drop and an on-device assistant
- **emaki-kit:** the shell's look as a library, so any app can match it

## Screenshots

Coming soon. The slots, and how they're taken, are in
[docs/user/screenshots](docs/user/screenshots/README.md).

| Status bar | Dock | Launcher |
|---|---|---|
| *bar, Regular, Frosted* | *the dock risen* | *a search and its results* |

| Quick Settings | Emaki Settings |
|---|---|
| *the grid of controls* | *the Appearance page* |

## Install

The shell is built and run through its Nix flake, and needs the niri fork.

```sh
nix develop -c cargo run    # in a niri session
nix build                   # ./result/bin/emaki and the apps
```

On NixOS, add `github:098765434/amane-shell` as a flake input, run the shell as a user service,
and let `gen-layer-rules.sh` put the shell's layer rules into niri's config. The steps are in
[docs/user/install.md](docs/user/install.md).

> **Do not** build this shell with the upstream `amane` CLI (`amane dev`, `amane compile`): it
> compiles against its own bundled copy of the library, not this fork. The flake is the one
> build path.

## Configure

Settings are JSON files under `~/.config/emaki/`, written by Emaki Settings and followed
live by the running shell. Theme, density, text size, the bar's position and size, the dock,
the launcher's sources and the notification rules are all there, with `EMAKI_*` variables to
override them: [docs/user/configure.md](docs/user/configure.md). The niri keybinds to copy are
in [docs/user/keyboard-and-ipc.md](docs/user/keyboard-and-ipc.md).

## Documentation

- **User guide:** [docs/user](docs/user): installing, configuring, the keybinds,
  troubleshooting, and a page per surface under [features](docs/user/features).
- **Developer docs:** [docs/developer](docs/developer): the architecture, the kit, the design
  system, the integration points (IPC, layer rules, status apps, NixOS), testing and releasing.
- **Index:** every doc is listed in [docs/README.md](docs/README.md).

## The stack

Four repositories make the desktop, each pinning the one below it; the chart and the bump order
are in [STACK.md](STACK.md).

- **[niri fork](https://github.com/098765434/niri)** (branch `ocbk`): materials and blur shapes,
  vibrancy, HDR, the workspace mat, the desktop layer the widgets sit on, gesture IPC.
- **[amane fork](https://github.com/098765434/amane)** (branch `ocbk`): the UI library the shell
  is written in, with the [vello fork](https://github.com/098765434/vello) as its renderer.

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md): the dev shell, the checks CI runs, and where a
change belongs (the shell, the kit, or one of the forks). The project follows the
[Contributor Covenant](CODE_OF_CONDUCT.md). Security reports go through GitHub's private
reporting, as [SECURITY.md](SECURITY.md) says. Changes are recorded in
[CHANGELOG.md](CHANGELOG.md).

## License

MIT — see [LICENSE](LICENSE). The shell links the `amane` library (also MIT) and talks to niri
over **IPC**, so the GPL-3.0 niri fork imposes no copyleft obligation on this crate.
