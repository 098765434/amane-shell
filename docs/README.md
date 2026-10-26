# amane-shell docs

amane-shell is a macOS-flavoured Wayland desktop shell for niri: a status bar, a dock, Spotlight,
notifications, Quick Settings and desktop widgets, plus **amane-kit** (`kit/`), the look as a
library that the shell and its apps (Shell Settings) share. It sits on two forks:

```
niri fork  (compositor: materials, blur shapes, vibrancy, the desktop layer, gesture IPC)
   ▲  Wayland protocols + niri IPC
amane fork (Rust UI library: layer windows, popups, services, blur regions)
   ▲  Cargo git dependency
amane-shell (this repo: src/ the shell, kit/ amane-kit, apps/settings/ Shell Settings)
```

- niri fork: <https://github.com/098765434/niri> (branch `ocbk`)
- amane fork: <https://github.com/098765434/amane> (branch `ocbk`)

Building and running are in the [README](../README.md); contributing in
[CONTRIBUTING.md](../CONTRIBUTING.md).

## Shell surfaces

| Surface | What it does | Doc |
|---|---|---|
| Status bar | the bottom bar: workspace pips, status icons, Quick Settings, notifications, the timer chip and the clock | [shell/status-bar.md](shell/status-bar.md) |
| Dock | every workspace's windows and pins, rising from the bottom edge and with the overview | [shell/dock.md](shell/dock.md) |
| Launcher | macOS Tahoe's Spotlight: search, Applications, Files, Actions, Clipboard | [shell/launcher.md](shell/launcher.md) |
| Notifications | the notification daemon's toasts, the notification center and Do Not Disturb | [shell/notifications.md](shell/notifications.md) |
| Quick Settings | GNOME's quick settings in the macOS Control Center look, with Edit Controls | [shell/quick-settings.md](shell/quick-settings.md) |
| Desktop widgets | readings and a clock on the wallpaper, on Quick Settings' grid | [shell/widgets.md](shell/widgets.md) |
| Context menu | nested xdg-popup menus: the dock's, the launcher's, the widgets', Do Not Disturb's and a Bluetooth device's | [shell/context-menu.md](shell/context-menu.md) |

## amane-kit

| Topic | Doc |
|---|---|
| Buttons, switch, checkbox, radio, control sizes | [kit/controls.md](kit/controls.md) |
| Card, list, slider, dialog, scroll view and the other building blocks | [kit/components.md](kit/components.md) |

## Design system

| Topic | Doc |
|---|---|
| Design tokens: tiers, naming, Penpot workflow, CI checks | [design-system/tokens.md](design-system/tokens.md) |
| Materials: glass by role, its tints, vibrant labels | [design-system/materials.md](design-system/materials.md) |
| Tokens still waiting on Penpot | [design-system/penpot-backlog.md](design-system/penpot-backlog.md) |

## Integration

| Topic | Doc |
|---|---|
| niri layer rules and surface namespaces, generated from the tokens | [integration/layer-rules.md](integration/layer-rules.md) |

## Development

| Topic | Doc |
|---|---|
| Dev shell, checks, the token workflow | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| What makes a window draw, and the rules that keep an idle desktop still | [development/performance.md](development/performance.md) |
