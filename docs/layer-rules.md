# niri layer-rules & surface namespaces

Each layer-shell surface the shell opens is given a **namespace** via amane's
`LayerWindow::namespace(...)`. niri matches those namespaces in `layer-rule` blocks to apply
backdrop blur and corner rounding. Two things must agree:

1. the Rust source — `LayerWindow::namespace("…")` in `src/components/**`;
2. `niri-layer-rule.kdl` — the rules, **generated** by `src/tokens/layer_rules.rs` from those
   namespace constants and the design tokens (corner radii are `sys.radius.popover`, or
   `notification.card.radius` for toasts: the radius the cards are drawn with).

Every glass surface but the bar says where to frost itself, through blur shapes
(`LayerWindow::blur_shapes`, niri-blur-shapes; `glass_region` in the kit), each shape naming the
`glass` material and carrying the glass's tint: Quick Settings' modules, the dock, the launcher,
the toasts and the notification center. Their rules are `clear` ones: `xray false`, since niri's
blur would otherwise show the wallpaper alone, and no blur of their own. Toasts and the center are
each only as big as their card, so a shadow the shell drew would be cut off at the surface's edge:
their rules round the surface (`notification.card.radius`, `notification.center.radius`) and give
it niri's `shadow`.

xdg-popups (menus, tooltips, the dropdowns) can't send blur shapes, so their `popups {}` blocks
frost them and name the same `glass` material (`background-effect { material "glass" }`, a rule
option of our niri fork): niri draws them as glass in the shape of their geometry, and the popup
draws its own tint over it. niri's `popups {}` block has no shadow, so they go without one.

The bar is the one plain-frost surface (`frost`): niri frosts all of it, square, with xray blur,
since it reserves its zone and only the wallpaper is ever behind it.

Where niri has the `glass` material (`amane::material_available`), it draws the lit edge too, so
the cards leave out the shader's (`components/glass.rs`) and their hairlines. A niri config
without the material, or without these rules installed, gets plain frost, and the shell lights
the edges itself; install the rules after every change (below), or menus lose their edge.

A golden test fails when (2) is out of date with the tokens. The live niri config is in turn
generated from (2), not hand-edited, so neither can drift.

## Namespaces

| Namespace | Surface |
|---|---|
| `amane.status-bar` | the bar: frosted whole, square, without a shadow or `xray false` (`frost`: it reserves its zone, so only the wallpaper is behind it); its xdg-popups, tooltips and Do Not Disturb's menu, are frosted too |
| `amane.quick-settings` | Quick Settings: a clear surface that frosts its own modules through a blur region; only `xray false` (a Bluetooth device's menu is a frosted xdg-popup of it) |
| `amane.widgets.<output>` | the desktop widgets, one surface a monitor on the background layer (matched by the prefix `^amane\.widgets\.`): a clear surface that frosts its own cards; `xray false`, and its popups (a card's menu) frosted |
| `amane.launcher` | the app launcher: a clear surface over the output that frosts its glass panel through a blur region; `xray false`, and its popups (an app's menu) frosted |
| `amane.notifications.toast` | each notification toast: one surface per banner, frosting its card itself; `clear`, rounded `notification.card.radius`, with niri's shadow at softness 48 (the banner design's `0 8 48`; other surfaces use 16) |
| `amane.alert` | the shell's alert in the middle of the screen, as big as its card, frosting itself in thick glass; `clear`, rounded `dialog.radius`, with niri's shadow at softness 48 and offset 18 (the dialog design's `0 18 48`) |
| `amane.notifications.center` | the notification center sidebar, frosting itself; `clear`, rounded `notification.center.radius`, with niri's shadow (its Do Not Disturb menu is a frosted xdg-popup of it) |
| `amane.context-menu-anchor` | the context-menu anchor window |
| `amane.context-menu.0` … `.7` | nested context-menu levels |

## Generating the rules

The generated file holds the layer rules, two `window-rule`s for each amane app (`APP_IDS`: its
window frosted behind in `frost`, its popups in `glass`; and its floating windows, its dialogs,
in `glass-large` with the alert's 34 corners and `0 18 48` shadow, which a window floated by hand
takes too) and, last, a `materials {}` block. It defines one
niri material per `material.<name>.*` token group: `glass`, `glass-small`, `glass-clear` and
`frost`, each with a `-still` twin (`materialize false`) for reduced motion. `glass` blurs with
passes and an offset of its own (`blur-passes`, `blur-offset`), more than `blur {}`, and
`glass-small` keeps `blur {}`'s: small glass frosts less than large ([materials.md](materials.md)). The shell asks for these by name
(`amane_kit::material`), so the niri config must not define them by hand as well. niri merges
`materials {}` blocks by name and the last one wins.

First bring the KDL up to date with the tokens (needed whenever a radius or a material changes in
Penpot; CI tells you):

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
