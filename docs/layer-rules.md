# niri layer-rules & surface namespaces

Each layer-shell surface the shell opens is given a **namespace** via amane's
`LayerWindow::namespace(...)`. niri matches those namespaces in `layer-rule` blocks to apply
backdrop blur and corner rounding. Two things must agree:

1. the Rust source — `LayerWindow::namespace("…")` in `src/components/**`;
2. `niri-layer-rule.kdl` — the rules, **generated** by `src/tokens/layer_rules.rs` from those
   namespace constants and the design tokens (corner radii are `sys.radius.popover`, or
   `notification.card.radius` for toasts: the radius the cards are drawn with).

A blurred surface gets `xray false`: niri's layer-surface blur is xray by default, which shows
only the wallpaper, so the cards would look opaque over windows. (Popups already default to
non-xray.)

Because niri frosts the whole surface, a frosted card draws no blur of its own (`Card::blur`
stays off for the panels), and a surface can't be bigger than its card.

A surface can instead say where to frost (`LayerWindow::blur_region`, ext-background-effect):
Quick Settings does, for its modules alone, so its rule is a `clear` one — no corner radius,
blur or shadow, only `xray false`, since niri's blur region would otherwise show the wallpaper
alone (see [quick-settings.md](quick-settings.md)).

A blurred surface also gets a `shadow`: each surface is only as big as its card, so a shadow
the shell drew would be cut off at the surface's edge. niri's `popups {}` block has no shadow,
so context menus and the menus of panels go without one. The shell lights the cards' edges
itself (`components/glass.rs`).

A golden test fails when (2) is out of date with the tokens. The live niri config is in turn
generated from (2), not hand-edited, so neither can drift.

## Namespaces

| Namespace | Surface |
|---|---|
| `amane.taskbar` | the bar (intentionally **not** blurred); its rule only frosts its xdg-popups: tooltips and a window's menu |
| `amane.taskbar.wifi` | the Wi‑Fi panel |
| `amane.taskbar.power` | the power panel |
| `amane.taskbar.bluetooth` | the Bluetooth panel (its device menu is a frosted xdg-popup of it) |
| `amane.taskbar.audio` | the Sound panel |
| `amane.quick-settings` | Quick Settings: a clear surface that frosts its own modules through a blur region; only `xray false` (a Bluetooth device's menu is a frosted xdg-popup of it) |
| `amane.notifications.toast` | each notification toast (one surface per banner, radius `notification.card.radius`) |
| `amane.notifications.center` | the notification center sidebar (its Do Not Disturb menu is a frosted xdg-popup of it) |
| `amane.context-menu-anchor` | the context-menu anchor window |
| `amane.context-menu.0` … `.7` | nested context-menu levels |

## Generating the rules

First bring the KDL up to date with the tokens (needed whenever a radius changes in Penpot; CI
tells you):

```sh
cargo run -q -- layer-rules > niri-layer-rule.kdl
```

`gen-layer-rules.sh` reads everything below the `//<<<GENERATED-RULES-BELOW>>>` sentinel in
`niri-layer-rule.kdl` (a test checks the script and the generator use the same one) and replaces the region **between the markers** in the target config,
touching nothing else:

```
// >>> amane layer-rules (generated from shell/niri-layer-rule.kdl — do not edit by hand)
// <<< amane layer-rules
```

```sh
./gen-layer-rules.sh                         # default target: ~/.config/niri/config.kdl
./gen-layer-rules.sh /path/to/other.kdl      # or an explicit target
niri validate -c ~/.config/niri/config.kdl
niri msg action load-config-file
```

A target that is a symlink is written through, to the file it points at, and stays a link; the
file keeps its permissions. That covers home-manager's out-of-store links into a repo. A link
into the Nix store is read-only, so then pass the source file you edit instead (for example
`./gen-layer-rules.sh ~/nixos-config/niri/config.kdl`) and rebuild.

```sh
# then check the surfaces are up
niri msg layers | grep amane
```

The script is idempotent — re-run it after every edit to `niri-layer-rule.kdl`.

### One-time setup (per target config)

The target needs the two marker lines once. Replace the hand-written amane `layer-rule` blocks in
the niri config with just:

```kdl
// >>> amane layer-rules (generated from shell/niri-layer-rule.kdl — do not edit by hand)
// <<< amane layer-rules
```

then run `./gen-layer-rules.sh` to fill them. (If the markers are missing the script prints this
setup hint and makes no changes.)

## Adding or renaming a surface

1. set its `.namespace("amane.…")` in the Rust source, as a `pub const`;
2. add a `LayerRule` for it in `rules()` in `src/tokens/layer_rules.rs`, taking its radius from
   a `sys.radius.*` token;
3. `cargo run -q -- layer-rules > niri-layer-rule.kdl`, then `./gen-layer-rules.sh` and reload
   niri.
