# niri layer-rules & surface namespaces

Each layer-shell surface the shell opens is given a **namespace** via amane's
`LayerWindow::namespace(...)`. niri matches those namespaces in `layer-rule` blocks to apply
backdrop blur and corner rounding. The namespaces are therefore a **three-way coupling** that must
stay in sync:

1. the Rust source — `LayerWindow::namespace("…")` in `src/components/**`,
2. `niri-layer-rule.kdl` — the **source of truth** draft of the rules, in this repo,
3. `~/nixos-config/niri/config.kdl` — the live niri config (managed; the rules are hand-copied
   here from `niri-layer-rule.kdl`).

## Namespaces

| Namespace | Surface |
|---|---|
| `amane.taskbar` | the bar |
| `amane.taskbar.wifi` | the Wi‑Fi panel |
| `amane.context-menu-anchor` | the context-menu anchor window |
| `amane.context-menu.0` … `.7` | nested context-menu levels |

## Keeping them in sync

When you add/rename a surface:

1. set its `.namespace("amane.…")` in the Rust source,
2. add/adjust the matching `layer-rule { match namespace="^amane\\.…$"; … }` in
   `niri-layer-rule.kdl`,
3. copy that block into `~/nixos-config/niri/config.kdl` and reload niri
   (the config is an out-of-store symlink — edits apply on reload, no `nixos-rebuild`).

> Auto-generating the niri-config blocks from `niri-layer-rule.kdl` is a planned follow-up to
> remove the manual copy step.
