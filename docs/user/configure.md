# Configuring emaki

**Everything the shell remembers is a JSON file under `~/.config/emaki/`, and the
running shell follows each file as it changes.** Emaki Settings (`emaki-settings`) writes
them; so can a text editor. A missing file or field takes its default, so an older or
hand-written file still works.

## The settings files

`$XDG_CONFIG_HOME/emaki/` (else `~/.config/emaki/`):

| File | Holds | Emaki Settings page |
|---|---|---|
| `appearance.json` | light or dark (or follow the system), accent, density, text scale, glass (clear, tinted, solid), reduced motion, adaptive glass, vibrant labels | Appearance |
| `status-bar.json` | position (top or bottom), size (Compact, Regular, Comfortable), style (Frosted, Glass, Solid, Clear), auto-hide, the items' order and which are hidden, the clock's format, weather, which apps' Activities show | Status Bar ([status-bar.md](features/status-bar.md#settings)) |
| `dock.json` | icon size, magnification, how it hides and rises | Dock ([dock.md](features/dock.md)) |
| `desktop.json` | whether the widgets show, and how they dim | Desktop ([widgets.md](features/widgets.md)) |
| `launcher.json` | the apps' layout, the search sources left out, quick keys, the clipboard history's settings | Launcher ([launcher.md](features/launcher.md)) |
| `notifications.json` | each app's notification rule: banners and list, list only, or off | Notifications ([notifications.md](features/notifications.md)) |
| `displays.json` | a lower refresh rate on battery | Displays ([displays.md](features/displays.md)) |
| `passwords.json` (mode 0600) | the rbw server and account, lock timeout, which apps get fill | Passwords ([passwords.md](features/passwords.md)) |
| `nixos.json` | where the NixOS configuration is (`repo`, `host`) | System ([nixos.md](../developer/integration/nixos.md)) |

Drop keeps its own files in `~/.config/emaki-drop/` ([drop.md](features/drop.md)). Quick
Settings' layout, the dock's pins, the widgets and the other things the shell keeps for itself
are **state**, in `~/.local/state/emaki/`; the whole list, and the caches, is in
[services.md](../developer/integration/services.md#settings-and-state).

An example `appearance.json`, with every field at its default:

```json
{ "mode": null, "accent": null, "density": "comfortable", "text_scale": 1.0, "glass": "tinted",
  "reduce_motion": null, "adaptive_glass": false, "vibrancy": false }
```

## Theme, density and text size

- **Theme.** The shell follows the system's light or dark preference (the Settings portal), else
  dark. `mode` in `appearance.json`, or `EMAKI_THEME`, forces one.
- **Accent.** The system's accent color, else the shell's own. `accent`, or Emaki Settings'
  swatches.
- **Density.** `comfortable` is the default; `compact` tightens spacing and rows.
- **Text scale.** `text_scale`, 0.5 to 3, scales every piece of text; the layouts follow.
- **Glass.** `clear`, `tinted` or `solid`: how much of what's behind a surface shows through.
  **Adaptive glass** turns small glass light or dark with its backdrop; **vibrancy** lets labels
  on glass blend with what's under them (a niri fork feature). The materials are described in
  [materials.md](../developer/design-system/materials.md).

## Environment variables

Set these in the service's environment (`Service.Environment` in the unit) or on the command
line. An override wins over the settings file while it's set.

| Variable | Does |
|---|---|
| `EMAKI_THEME=light\|dark` | forces the theme |
| `EMAKI_DENSITY=compact\|comfortable` | forces the density |
| `EMAKI_TEXT_SCALE=<0.5–3>` | scales text |
| `EMAKI_GLASS=clear\|tinted\|solid` | forces the glass level |
| `EMAKI_ADAPTIVE_GLASS=1`, `EMAKI_VIBRANCY=1` | turns adaptive glass or vibrant labels on |
| `EMAKI_HIGH_CONTRAST=1` | higher contrast, as the system's accessibility setting would |
| `EMAKI_LOG=error\|warn\|info\|debug\|trace` | how much the shell and its apps log (`info` by default); the journal keeps it |
| `EMAKI_DOCK_ANIMATIONS=off` | turns the dock's animations off, whatever the settings say |
| `EMAKI_DOCK_FULL_WHILE_HIDDEN=1` | a hidden dock keeps its full surface instead of a reveal strip |
| `EMAKI_STATS_WHILE_HIDDEN=1` | the widgets' system readings go on while the desktop is covered |
| `EMAKI_ASSISTANT_IDLE=<seconds>` | how long the assistant's daemon stays loaded with nothing asked (300; `0` to stay) |
| `EMAKI_ASSISTANT_MODELS=<dir>` | where the assistant's models are (the flake sets it) |
| `EMAKI_RBW`, `EMAKI_PINENTRY_FALLBACK` | the `rbw` and pinentry programs the shell runs, by path (the flake sets them) |
| `EMAKI_DROP_0X0_URL=<url>` | points Drop's 0x0.st tile at another server, for testing |
| `EMAKI_MIRROR_SOFTWARE=1` | Phone Screen decodes on the CPU instead of VAAPI |

Some `EMAKI_*` variables belong to the **amane library**, not the shell, and are documented in
[its docs](https://github.com/098765434/amane/blob/ocbk/docs/fork/README.md): `AMANE_DEBUG`,
`AMANE_FRAMES`, `AMANE_VELLO_*`, `AMANE_IMAGE_*`, `AMANE_KEEP_*`, `AMANE_DEVICE_IDLE` and
`AMANE_PRESENT`. They tune rendering and are for measuring, not for daily use.

## niri's side

Two things live in niri's config rather than the shell's files:

- **The layer rules** that frost the shell's surfaces, spliced in by `gen-layer-rules.sh`
  ([install.md](install.md#3-niris-layer-rules)). Run it again after an update.
- **The keybinds** that open the launcher, Quick Settings and the rest
  ([keyboard-and-ipc.md](keyboard-and-ipc.md)).

The bar reserves its zone, so niri's `layout { gaps … }` and the workspace mat (a niri fork
feature) are set in niri, not here.
