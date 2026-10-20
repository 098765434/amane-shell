# amane-shell

A custom Wayland desktop bar for the [niri](https://github.com/YaLTeR/niri) compositor, built on
the [amane](../amane-fork) shell library (fork). macOS-flavoured: a translucent frosted panel,
rounded pills, generous spacing — compact and ambient in normal use, expanding into an interactive
dock while niri's overview is open.

This is the user shell instance. It lives in the desktop workspace at
`~/Projects/desktop/shell` and is symlinked to the conventional location `~/.config/amane`.

## Features

- **Taskbar** ([docs/taskbar.md](docs/taskbar.md)) — bottom bar with three zones:
  - left: overview toggle + workspace pips;
  - center: focused-window title (compact) ↔ interactive window dock (overview);
  - right: volume, network, battery, clock.
  - **Overview smooth-expand**: the bar height eases between compact and tall in lockstep with
    niri's overview zoom, driven by the amane `Gestures::overview_progress()` value (requires the
    niri gesture-IPC fork at `../niri-fork`).
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

Or `nix develop` once to drop into the environment, then use `cargo` directly. The bar depends on
the amane library fork by relative path (`amane = { path = "../amane-fork" }` in `Cargo.toml`), so
the two are developed together as siblings. See [../runbooks/build-shell.md](../runbooks/build-shell.md).

> **Do not** build this shell with the `amane` CLI (`amane dev`/`compile`) installed via
> nixos-config: that CLI compiles against its own bundled library copy in `~/.cache/amane/library`,
> **not** this fork (so it lacks `Gestures` and won't compile the bar). `nix develop` is the one
> canonical build path.

## niri layer-rules (required)

The bar's surfaces need matching `layer-rule` blocks in the niri config (blur + corner rounding).
`niri-layer-rule.kdl` is the **source of truth**; `./gen-layer-rules.sh` injects those blocks into
`~/nixos-config/niri/config.kdl` between its markers, so the two never drift (no hand-copying). See
[docs/layer-rules.md](docs/layer-rules.md) — the namespaces must still match the Rust
`LayerWindow::namespace(...)` calls.

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

MIT — see [LICENSE](LICENSE).
