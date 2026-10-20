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

Uses `run.sh`, a wrapper that puts the Nix toolchain + the required wayland/vulkan/pulse libraries
on `PATH`/`PKG_CONFIG_PATH`/`LD_LIBRARY_PATH` so `cargo` works outside a nix shell:

```sh
./run.sh build      # cargo build
./run.sh            # cargo run  (default theme: dark)
./run.sh light      # AMANE_THEME=light cargo run
```

The bar depends on the amane library fork by relative path (`amane = { path = "../amane-fork" }`
in `Cargo.toml`), so the two are developed together. See [../docs/runbooks/build-shell.md](../docs/runbooks/build-shell.md).

> The `amane` CLI (`amane dev`/`compile`) installed via nixos-config builds against its own bundled
> library copy in `~/.cache/amane/library`, **not** this fork — use `run.sh` for this shell.

## niri layer-rules (required)

The bar's surfaces need matching `layer-rule` blocks in the niri config (blur + corner rounding).
`niri-layer-rule.kdl` is the **source of truth** for those rules and their surface namespaces; the
same blocks are mirrored into `~/nixos-config/niri/config.kdl`. See
[docs/layer-rules.md](docs/layer-rules.md) — these namespaces must stay in sync with the Rust
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
run.sh                          build/run wrapper (Nix toolchain + lib paths)
```

## License

MIT — see [LICENSE](LICENSE).
