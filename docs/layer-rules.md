# niri layer-rules & surface namespaces

Each layer-shell surface the shell opens is given a **namespace** via amane's
`LayerWindow::namespace(...)`. niri matches those namespaces in `layer-rule` blocks to apply
backdrop blur and corner rounding. Two things must agree:

1. the Rust source — `LayerWindow::namespace("…")` in `src/components/**`;
2. `niri-layer-rule.kdl` — the **single source of truth** for the rules (this repo).

The live niri config (`~/nixos-config/niri/config.kdl`) is **generated** from (2), not hand-edited,
so it can't drift.

## Namespaces

| Namespace | Surface |
|---|---|
| `amane.taskbar` | the bar (opaque — intentionally has **no** blur rule) |
| `amane.taskbar.wifi` | the Wi‑Fi panel |
| `amane.context-menu-anchor` | the context-menu anchor window |
| `amane.context-menu.0` … `.7` | nested context-menu levels |

## Generating the rules

`gen-layer-rules.sh` reads everything below the `//<<<GENERATED-RULES-BELOW>>>` sentinel in
`niri-layer-rule.kdl` and replaces the region **between the markers** in the target config,
touching nothing else:

```
// >>> amane layer-rules (generated from shell/niri-layer-rule.kdl — do not edit by hand)
// <<< amane layer-rules
```

```sh
./gen-layer-rules.sh                         # default target: ~/nixos-config/niri/config.kdl
./gen-layer-rules.sh /path/to/other.kdl      # or an explicit target
niri validate -c ~/nixos-config/niri/config.kdl
niri msg action reload-config                # live reload (config is an out-of-store symlink)
```

The script is idempotent — re-run it after every edit to `niri-layer-rule.kdl`.

### One-time setup (per target config)

The target needs the two marker lines once. Replace the hand-written amane `layer-rule` blocks in
`~/nixos-config/niri/config.kdl` with just:

```kdl
// >>> amane layer-rules (generated from shell/niri-layer-rule.kdl — do not edit by hand)
// <<< amane layer-rules
```

then run `./gen-layer-rules.sh` to fill them. (If the markers are missing the script prints this
setup hint and makes no changes.)

## Adding or renaming a surface

1. set its `.namespace("amane.…")` in the Rust source;
2. add/adjust the matching `layer-rule { match namespace="^amane\\.…$"; … }` block in
   `niri-layer-rule.kdl` (below the sentinel);
3. run `./gen-layer-rules.sh` and reload niri.
