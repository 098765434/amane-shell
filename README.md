# amane-shell

A custom Wayland desktop bar for the [niri](https://github.com/YaLTeR/niri) compositor, built on the
[amane](https://github.com/098765434/amane) shell library (a fork). macOS-flavoured: a clean
opaque bar with rounded pills and generous spacing — compact and ambient in normal use, expanding
into an interactive dock while niri's overview is open. Its floating surfaces (context menus, Quick
Settings, notification toasts and the notification center) are the frosted, compositor-blurred
ones — the bar itself is not.

> **Requires a forked niri** for its signature feature (the overview smooth-expand needs live
> gesture-IPC events): [098765434/niri](https://github.com/098765434/niri). It runs on
> stock niri without that animation. Dragging a window's icon in the dock to reorder it needs
> the fork too (`MoveColumnToIndex` with a `window_id`); stock niri would move the focused
> column instead.

## Features

- **Status bar** ([docs/status-bar.md](docs/status-bar.md)) — a compact bottom bar: workspace
  pips on the left (click or scroll to switch) and the status cluster on the right (privacy,
  volume, Bluetooth, network, battery, Quick Settings, notifications, clock). Every item has a
  tooltip and turns into a white rounded tile while hovered or while what it opens is open.
- **Status icons** — volume, Bluetooth, network, Tailscale and battery, each optional and
  opening its Quick Settings page: Sound (volume, microphone and devices), Bluetooth (connect,
  pair, forget), Wi‑Fi (scan and join), Tailscale (connect, log in, exit node, routes/DNS,
  block incoming, the tailnet's devices) and Battery (power profile, peripherals).
- **Dock** ([docs/dock.md](docs/dock.md)) — a macOS-style dock that rises from the bottom edge
  (and with niri's overview), showing every workspace of its monitor side by side with a rule
  between them and an icon per window. Pin apps to a workspace, drag icons to reorder windows or
  move them between workspaces, and drop files on an icon to open them with that app (hold them
  there and its window comes into view).
- **Context menus** ([docs/context-menu.md](docs/context-menu.md)) — right-click a window's icon in the dock for focus/close/fullscreen/float/move-to-workspace/pin,
  built on amane's xdg-popup support (nested submenus, outside-click dismiss).
- **Notifications** ([docs/notifications.md](docs/notifications.md)) — the shell is the
  notification daemon (through amane): macOS-style toasts at the top-right (hover to hold, swipe
  to put away), a notification center sidebar opened from the clock with iOS-style stacks per
  app, and Do Not Disturb (on, for an hour, until this evening or tomorrow; critical
  notifications still show).
- **Quick Settings** ([docs/quick-settings.md](docs/quick-settings.md)) — GNOME's quick
  settings in the macOS Control Center look, from the bar's slider button. The system row
  (battery, screenshot, lock, power menu), Now Playing, volume, microphone and brightness
  sliders, and tiles for wired, Wi‑Fi (and hotspot), VPN, Bluetooth, power mode, night light,
  dark style, Do Not Disturb, keyboard light, airplane mode and background apps. Each shows only
  when the machine has it, and each opens a page of its own that slides in. Edit Controls opens
  a Customize window beside the card, after macOS's: a search, the controls by category, and a
  drag onto the card to add one or off it to remove one. The bar shows GNOME's privacy icons
  (screen shared, camera, microphone) and the settings that are on.
- **Launcher** ([docs/launcher.md](docs/launcher.md)) — macOS Tahoe's Spotlight, from a keybind
  (`launcher toggle` over IPC): a glass search pill with round buttons beside it for its modes,
  growing into a panel as you type. The search ranks apps by a fuzzy match and by how much
  they're used, answers sums and unit conversions in a card, finds session actions, URLs and a
  web search, narrows to an app's own actions with Tab, and brings back past searches with Up.
  Applications is a Launchpad grid with category pills, the apps used most on top, momentum
  scrolling and sideways swipes between categories.
- **Desktop widgets** ([docs/widgets.md](docs/widgets.md)) — macOS's widgets on the
  wallpaper, for watching: batteries, CPU, memory, temperature, network, disks and a System card,
  on Quick Settings' grid across each monitor. Drag them anywhere at any time, place the same
  kind more than once with its own settings, right-click for sizes and what each shows, and
  Edit Widgets for a gallery to drag new ones from. They fade back while windows are open.
- **Buttons** ([docs/buttons.md](docs/buttons.md)) and **components**
  ([docs/components.md](docs/components.md)) — shared push and icon buttons, a switch, a
  segmented button, cards, top app bars, lists, dividers and a slider, each after its Material 3
  counterpart but drawn in the shell's macOS/iOS look.
- **Design tokens** — every colour, size, gap and font is a token designed in Penpot and exported
  to `design-tokens/tokens.json`, compiled in with `include_str!` and resolved the way Penpot does
  (`src/tokens/`). See [docs/design-token-manual.md](docs/design-token-manual.md).

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

## niri layer-rules (for the frosted popups)

The shell's **floating** surfaces (context menus, Quick Settings, notification toasts and the center) get their blur + corner
rounding from matching `layer-rule` blocks in your niri config. The bar itself is opaque and needs
no rule. `niri-layer-rule.kdl` holds those blocks, generated from the design tokens
(`cargo run -q -- layer-rules > niri-layer-rule.kdl`), so the blur's corners follow the cards'
radii. `./gen-layer-rules.sh` injects the blocks into a target niri config between marker
comments, so the two never drift (no hand-copying):

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

Or, more simply, a niri `spawn-at-startup "amane-shell"` line (no crash recovery).

## Shell Settings

`amane-settings` is a settings app for the shell, built on the [kit](kit/README.md). Its
Appearance page picks light or dark (or the system's), the accent, the density and the text size.
It saves them to `~/.config/amane-shell/appearance.json`, and the running shell follows the file at
once; editing the file by hand works too. `AMANE_THEME`, `AMANE_DENSITY` and `AMANE_TEXT_SCALE`
still win when set.

Only one copy runs: launching it again brings the open one forward. The flake installs it with a
`.desktop` entry, so the launcher lists it as *Shell Settings*. Its window draws its own solid
surface, so it needs no layer rule; a niri window rule can still round it like other apps:

```kdl
window-rule {
    match app-id="^amane-settings$"
    geometry-corner-radius 12
    clip-to-geometry true
}
```

## Layout

```
src/
  main.rs                       App builder: windows, popups, IPC handlers, `tokens` CLI
  tokens/layer_rules.rs         the niri layer rules generated from the tokens
  components/
    status_bar/                 the bar (mod.rs) and its optional status icons (indicators.rs)
    panel.rs                    the one-open-at-a-time overlay state the panels share
    context_menu/               reusable nested context menu on xdg-popups
    notifications/              toasts, the notification center and Do Not Disturb
    launcher/                   the launcher: Spotlight's search pill, its modes and the app grid
    dock/                       the dock: workspaces' windows and pins, drag, file drops (docs/dock.md)
    quick_settings/             Quick Settings: tiles, sliders and their pages
    grid/                       the grid Quick Settings and the widgets share: cells, layout, drag
    widgets/                    desktop widgets, their faces and the gallery (docs/widgets.md)
    workspace_menu.rs           a submenu of workspaces (the dock's and the launcher's menus)
    mod.rs                      re-exports the kit's controls under their old paths
kit/                            amane-kit: the look as a library, for the shell and its apps
  build.rs                      generates `tokens::sys::…`, `tokens::bar::…`, … from tokens.json
  src/tokens/ theme.rs          Penpot token resolver, the manual's checks, mode/accent/density/scale
  src/settings.rs               appearance.json, which theme.rs reads, and desktop.json
  src/buttons/                  push, icon, drawn and segmented buttons, switch, checkbox, radio
  src/card.rs list.rs …         shared M3-style components (docs/components.md)
  src/app.rs dropdown.rs        an app's window, fields, sidebar and dialog; the dropdown
  examples/gallery.rs           every control in one window
apps/settings/                  Shell Settings (amane-settings)
design-tokens/tokens.json       Penpot export (normalized); edit in Penpot, not here
tests/fixtures/                 service fixtures (the token ones are in kit/tests/fixtures/)
niri-layer-rule.kdl             layer rules generated from the tokens (see docs/layer-rules.md)
gen-layer-rules.sh              injects those rules into the niri config
flake.nix                       the package (shell and apps) and the devShell
```

## License

MIT — see [LICENSE](LICENSE). The bar links the `amane` library (also MIT) and talks to niri over
**IPC**, so the GPL-3.0 niri fork imposes no copyleft obligation on this crate.
