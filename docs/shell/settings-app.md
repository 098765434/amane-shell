# Shell Settings

`amane-settings` (`apps/settings/`) is the shell's settings app, built on the kit's
[app scaffolding](../kit/app.md). Each control changes one of the shell's settings files, which the
running shell follows at once; Shell Settings' own window redraws in the new look too. Editing a
file by hand works the same way.

Only one copy runs: launching it again brings the open window forward. The flake installs it with
a `.desktop` entry, so the launcher lists it as *Shell Settings*.

## The window

An `app_window` titled "Shell Settings", 760 × 520 (at least 600 × 400), with a 200 px
[`sidebar`](../kit/app.md#sidebar) of seven pages and the page's title over its settings, each a
caption over its control. Each page scrolls in an amane `ScrollArea` that keeps its own offset. Ctrl+W closes it (`standard_shortcut`).

Its content is drawn on the opaque window background; only the sidebar shows the frost behind the
window. That frost, and the frosted popups (the dropdowns' menus), come from the `window-rule`
the generated config has for `amane-settings` ([layer-rules.md](../integration/layer-rules.md)).
Without it the sidebar is an opaque panel. A niri window rule can still round the window like
other apps:

```kdl
window-rule {
    match app-id="^amane-settings$"
    geometry-corner-radius 12
    clip-to-geometry true
}
```

## Pages

### Appearance

Writes `appearance.json` (`apps/settings/src/appearance.rs`).

| Setting | Control | Choices | Field |
|---|---|---|---|
| Appearance | radios | Follow the system, Light, Dark | `mode`: `null`, `"light"`, `"dark"` |
| Accent colour | dropdown | System, Blue, Purple, Pink, Red, Orange, Yellow, Green, Graphite (macOS 27's colours) | `accent`: `null` or `#rrggbb` |
| Density | radios | Comfortable, Compact | `density` |
| Text size | dropdown | Smaller 0.9, Default 1, Larger 1.1, Large 1.25, Largest 1.5 (the nearest one shows for a hand-set value) | `text_scale` |
| Glass | radios | Clear, Tinted, Solid | `glass` |
| Adaptive glass | switch, off under Solid glass | small glass turning light or dark with its backdrop (an experiment) | `adaptive_glass` |
| Vibrant labels | switch, off under Solid glass | labels blending with the frosted backdrop, through niri's vibrancy (an experiment) | `vibrancy` |
| Motion | radios | Follow the system, Reduce motion, Full motion | `reduce_motion`: `null`, `true`, `false` |

An accent saved before macOS 27's colours shows as its new counterpart; picking it again saves
the new colour. What each setting does to the look is in
[materials.md](../design-system/materials.md#settings) and [tokens.md](../design-system/tokens.md).

### Desktop

Writes `desktop.json` (`desktop.rs`): **Show widgets on the desktop** (`show_widgets`, a switch)
and **Widget style** (`widget_style`): Dim while windows are open (`automatic`, the default),
Always full (`full`) or Always dimmed (`dim`). See [widgets.md](widgets.md).

### Status Bar

Writes `status-bar.json` (`status_bar.rs`); see [status-bar.md](status-bar.md#settings).

| Setting | Control | Field |
|---|---|---|
| Position | segmented: Bottom, Top | `position` |
| Material | radios: Frosted, Glass, Solid (black in dark mode, white in light) | `style` |
| Clock | switches: 24-hour time, Show seconds | `clock_24h`, `clock_seconds` |
| Date in the clock's tooltip | dropdown: Sunday 25 October, Sun 25 Oct, the locale's numbers | `date_format` |
| Workspaces | switches: Show workspaces, Scroll over them to move between workspaces | `show_workspaces`, `scroll_workspaces` |
| Show in the status bar | a switch per optional icon | `hidden` |

### Dock

Writes `dock.json` (`dock.rs`); see [dock.md](dock.md).

| Setting | Control | Field |
|---|---|---|
| Icon size | slider, 32–64 pt | `icon_size` |
| Magnification | segmented: Off, Small, Medium, Large | `magnification` |
| Hiding | switches: hide until the pointer meets the edge, show with the overview | `auto_hide`, `rise_with_overview` |
| Icons | switches: dots under open apps, an app's name over its icon, animate | `running_dots`, `tooltips`, `animations` |

### Quick Settings

The layout is the shell's own state, so this page (`quick_settings.rs`) has no file of its own: it
calls the running shell over IPC ([ipc.md](../integration/ipc.md#quick-settings)) and watches
`$XDG_STATE_HOME/amane-shell/quick-settings` to stay in step with Customize.

- **Customize…** (`quick-settings customize`) and **Reset Layout…** (`reset-layout`, after a
  dialog).
- **Controls**: a switch per control this machine has (`controls`, then `show` or `hide`).
- While the shell doesn't answer, a line says so, with **Try Again**.

### Spotlight

Writes `launcher.json` (`launcher.rs`); see [launcher.md](launcher.md).

| Setting | Control | Field |
|---|---|---|
| Search | a switch per source: Applications, Actions, Files, Calculations and conversions, Web addresses, Web search | `excluded_providers`: the ids switched off (`apps`, `actions`, `files`, `answers`, `url`, `web`) |
| Show applications as | radios: Grid, List | `apps_layout` |
| Clipboard history | Keep what's copied, for Clipboard; Keep copied images too; a caption with how many it keeps and the apps it leaves out | `clipboard.enabled`, `clipboard.images` (`max_items` and `exclude_apps` only by hand) |
| Quick keys | each action's keys with **Remove**, and **Reset All Quick Keys** | `quick_keys` |

Quick keys are set in the launcher itself (Actions, Ctrl+K); this page only lists and removes
them.

### About

The app's version, where the settings files are, and a note that `AMANE_THEME`, `AMANE_DENSITY`
and `AMANE_TEXT_SCALE` win over these settings when set.

## The files

`appearance.json`, `desktop.json`, `status-bar.json`, `dock.json` and `launcher.json` are JSON in
`$XDG_CONFIG_HOME/amane-shell/` (else `~/.config/amane-shell/`), read and
written through amane's `Config` by `amane_kit::settings`. A missing field takes its default, so
an older or hand-written file still works:

```json
{ "mode": null, "accent": null, "density": "comfortable", "text_scale": 1.0, "glass": "tinted",
  "reduce_motion": null, "adaptive_glass": false, "vibrancy": false }
```

`AMANE_THEME`, `AMANE_DENSITY`, `AMANE_TEXT_SCALE`, `AMANE_GLASS`, `AMANE_ADAPTIVE_GLASS=1` and
`AMANE_VIBRANCY=1` win over the file when set. The shell's other files, its state, are in
[services.md](../integration/services.md#settings-and-state).
