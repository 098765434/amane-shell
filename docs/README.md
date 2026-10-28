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

- niri fork: <https://github.com/098765434/niri> (branch `ocbk`); what it adds:
  [docs/fork/README.md](https://github.com/098765434/niri/blob/ocbk/docs/fork/README.md). Not the shell's, but part of the same desktop: the workspace
  mat, a backing niri paints behind the scrolling layout's windows ([layout/workspace-mat.md](https://github.com/098765434/niri/blob/ocbk/docs/fork/layout/workspace-mat.md)).
- amane fork: <https://github.com/098765434/amane> (branch `ocbk`); what it adds:
  [docs/fork/README.md](https://github.com/098765434/amane/blob/ocbk/docs/fork/README.md).

A feature's docs live where its code lives: these docs link to the forks' rather than repeat
them.

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
| Alert | macOS's alert in the middle of the screen, for questions like "Restart now?" | [shell/quick-settings.md#the-alert](shell/quick-settings.md#the-alert) |
| Shell Settings | the settings app: appearance, the desktop's widgets, Spotlight, and the System pages that manage the NixOS configuration | [shell/settings-app.md](shell/settings-app.md) |
| Displays | Shell Settings' page for the screens: what each can do (OLED, HDR, VRR range), resolution, refresh (lower on battery), scale, rotation, arrangement, HDR, a monitor's brightness over DDC/CI, Night Light's schedule | [shell/displays.md](shell/displays.md) |
| Context menu | nested xdg-popup menus: the dock's, the launcher's, the widgets', Do Not Disturb's and a Bluetooth device's | [shell/context-menu.md](shell/context-menu.md) |

## amane-kit

| Topic | Doc |
|---|---|
| Buttons, switch, checkbox, radio, dropdown, control sizes | [kit/controls.md](kit/controls.md) |
| Card, list, slider, dialog, tag, arrangement, scroll view and the other building blocks; the modules under them | [kit/components.md](kit/components.md) |
| An app's window, text fields and sidebar | [kit/app.md](kit/app.md) |
| The colour picker, and hex and HSV helpers | [kit/color-picker.md](kit/color-picker.md) |
| Saved Shader Lab looks, and putting one on a rectangle | [kit/lab.md](kit/lab.md) |
| Springs and fades, keyed list motion, `Kinetic`, `Pager` | [kit/motion.md](kit/motion.md) |

## Design system

| Topic | Doc |
|---|---|
| Design tokens: tiers, naming, Penpot workflow, CI checks | [design-system/tokens.md](design-system/tokens.md) |
| Materials: glass by role, its tints, vibrant labels | [design-system/materials.md](design-system/materials.md) |
| Typography: the type roles, font stacks, text scale | [design-system/typography.md](design-system/typography.md) |
| Tokens still waiting on Penpot | [design-system/penpot-backlog.md](design-system/penpot-backlog.md) |

## Integration

| Topic | Doc | The forks' side |
|---|---|---|
| niri layer rules and surface namespaces, generated from the tokens | [integration/layer-rules.md](integration/layer-rules.md) | niri [materials](https://github.com/098765434/niri/blob/ocbk/docs/fork/effects/materials.md), [desktop layer](https://github.com/098765434/niri/blob/ocbk/docs/fork/layout/desktop-layer.md); amane [blur](https://github.com/098765434/amane/blob/ocbk/docs/fork/graphics/blur.md) |
| Every CLI and IPC command, and the socket they go over | [integration/ipc.md](integration/ipc.md) | niri [IPC events](https://github.com/098765434/niri/blob/ocbk/docs/fork/ipc/events.md), [actions](https://github.com/098765434/niri/blob/ocbk/docs/fork/ipc/actions.md) |
| The shell's services and amane's, the settings files and the state files | [integration/services.md](integration/services.md) | amane [compositor services](https://github.com/098765434/amane/blob/ocbk/docs/fork/compositor/services.md), [system services](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/idle.md) (`system/`) |
| The NixOS configuration from Shell Settings: the registry and `managed/settings.json`, the `amaneSettings` output, build → compare → activate through polkit, the commit, "up to date" | [integration/nixos.md](integration/nixos.md) | nixos-config `modules/managed.nix` |

## Development

| Topic | Doc |
|---|---|
| Dev shell, checks, the token workflow | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| What makes a window draw, and the rules that keep an idle desktop still | [development/performance.md](development/performance.md) |
| What the desktop holds in GPU memory, how to measure it, and what has been cut (the vello fork) | [development/memory.md](development/memory.md) |
| Trying shader effects live, and porting Figma shaders | [development/shader-lab.md](development/shader-lab.md) |

## Archive

Implemented specs, kept as history and not kept up to date.

| Spec | Superseded by |
|---|---|
| [archive/spec-design-tokens-v2.md](archive/spec-design-tokens-v2.md): Penpot tokens, the resolver, CI checks | [design-system/tokens.md](design-system/tokens.md) |
| [archive/spec-macos27-variables.md](archive/spec-macos27-variables.md): the macOS 27 kit's labels, fills, materials and control sizes | [design-system/materials.md](design-system/materials.md), [kit/controls.md](kit/controls.md) |
