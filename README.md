# amane-shell

A custom Wayland desktop bar for the [niri](https://github.com/YaLTeR/niri) compositor, built on the
[amane](https://github.com/098765434/amane) shell library (a fork). macOS-flavoured: a
translucent frosted panel, rounded pills, generous spacing — compact and ambient in normal use,
expanding into an interactive dock while niri's overview is open.

> **Requires a forked niri** for its signature feature (the overview smooth-expand needs live
> gesture-IPC events): [098765434/niri](https://github.com/098765434/niri). It runs on
> stock niri without that animation.

## Features

- **Taskbar** ([docs/taskbar.md](docs/taskbar.md)) — bottom bar with three zones:
  - left: overview toggle + workspace pips;
  - center: focused-window title (compact) ↔ interactive window dock (overview);
  - right: volume, network, battery, clock.
  - **Overview smooth-expand**: the bar height eases between compact and tall in lockstep with
    niri's overview zoom, driven by the amane `Gestures::overview_progress()` value (requires the
    [niri gesture-IPC fork](https://github.com/098765434/niri)).
- **Wi‑Fi panel** — a toggle in the status cluster opens a layer-shell panel to scan/connect.
- **Context menus** — right-click a window pill for focus/close/fullscreen/float/move-to-workspace,
  built on amane's xdg-popup support (nested submenus, outside-click dismiss).
- **Design tokens** — all sizing/colour/typography live in `design-tokens/*.json`, compiled into
  the binary via `include_str!` and parsed in each component's `tokens.rs`.

## Build & run

A Nix flake (`flake.nix`) provides a devShell with the Rust toolchain and the required
wayland/vulkan/pulse/fontconfig libraries wired up (no hardcoded store paths), so plain `cargo`
works inside it:

```sh
nix develop -c cargo build          # compile
nix develop -c cargo run            # run  (default theme: dark)
nix develop -c env AMANE_THEME=light cargo run   # light theme
```

For a release binary, `nix build` produces `./result/bin/amane-shell`, wrapped with the runtime
libraries it needs (so it runs standalone).

The `amane` library is a **git dependency** pinned to a rev (`Cargo.toml`). To develop the shell and
the library together, check `amane` out beside this repo and add a git-ignored
`.cargo/config.toml` with `paths = ["../amane"]` — see [CONTRIBUTING.md](CONTRIBUTING.md).

> **Do not** build this shell with the upstream `amane` CLI (`amane dev`/`compile`): it compiles
> against its own bundled library copy, **not** this fork (so it lacks `Gestures` and won't compile
> the bar). The flake is the one canonical build path.

## niri layer-rules (required)

The bar's surfaces need matching `layer-rule` blocks in your niri config (blur + corner rounding).
`niri-layer-rule.kdl` is the **source of truth**; `./gen-layer-rules.sh` injects those blocks into a
target niri config between marker comments, so the two never drift (no hand-copying):

```sh
./gen-layer-rules.sh /path/to/your/niri/config.kdl   # e.g. ~/.config/niri/config.kdl
```

See [docs/layer-rules.md](docs/layer-rules.md) — the namespaces must match the Rust
`LayerWindow::namespace(...)` calls.

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

Or, more simply, a niri `spawn-at-startup "amane-shell"` line (no crash recovery). Default theme is
dark; set `AMANE_THEME=light` to switch.

## Layout

```
src/
  main.rs                       App builder: windows, popups, IPC handlers
  components/
    taskbar/                    the bar (mod.rs) + clock, wifi, theme, tokens
    context_menu/               reusable nested context menu on xdg-popups
design-tokens/                  bar.tokens.json, menu.tokens.json
niri-layer-rule.kdl             layer-rule source of truth (see docs/layer-rules.md)
gen-layer-rules.sh              injects those rules into the niri config
flake.nix                       devShell (Rust toolchain + wayland/vulkan/pulse libs)
```

## License

MIT — see [LICENSE](LICENSE). The bar links the `amane` library (also MIT) and talks to niri over
**IPC**, so the GPL-3.0 niri fork imposes no copyleft obligation on this crate.
