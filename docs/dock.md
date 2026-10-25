# Dock

The macOS Big Sur dock, for the windows open on every workspace (`src/components/dock/`). It's one
frosted glass card over the bar on every monitor (`App::window_per_monitor`), namespace
`amane.dock`.

## What it shows

- **Every workspace of its monitor at once**, side by side, a rule (`Divider::vertical`) between
  one workspace and the next: every workspace with a window or a pin on it, plus the active one
  even when it's empty. The active workspace's icons are lit behind (`sys.color.state.hover`).
- **An icon per window**, in niri's column order (windows stacked in one column one after the
  other), floating windows after the tiled ones. A dot under an icon is brighter while its window
  has the focus. The name over the hovered icon is the window's title.
- **Pinned apps**, after the windows of the workspace they were pinned on. A pinned app with a
  window on that workspace shows only the window's icon. A pin knows its workspace by niri's name
  for it when it has one, else by its place on the monitor; a pin on a workspace that isn't open
  yet (place 4 while there are 2) has a section of its own where that workspace would be.
- **Too many to fit**, the icons shrink (down to 20 px) so the card stays inside the monitor.

## Using it

| Do | What happens |
|---|---|
| Touch the bottom edge | It comes up over the bar; it goes back down 400 ms after the pointer leaves |
| Open niri's overview | It rises in step with the overview's zoom (`Overview::progress`) |
| Click a window's icon | Focuses it |
| Click a pinned app | Focuses its latest window, wherever it is; with none open, opens it on the pin's workspace |
| Right-click a window's icon | Its menu: focus, fullscreen, float, move to workspace, **Pin to Dock** / **Unpin from Dock**, close |
| Right-click a pinned app | Open, **Open in Workspace ▸** (once, the pin stays put), **Unpin from Dock** |
| Drag a window's icon along its workspace | The others make way; let go, niri moves its column there (the focus stays) |
| Drag a window's icon into another workspace | It moves to that workspace at that place; the empty workspace after the last shows while you drag, to make a new one |
| Drag a pinned app up off the dock | It fades; let go there, it's unpinned |
| Drag files from another program onto an icon | Icons whose app opens them (its desktop entry's `MimeType`) light up; let go to open them with it, on that icon's workspace |
| Hold the files over an icon | After 700 ms its window comes into view (a pinned app that isn't open opens), so you can drop into the window |

While its menu is open, an icon is held, or files are held over it, the dock stays up wherever the
pointer goes. Files held over it reveal it from the bottom edge like the pointer does.

Under the pointer (or held files), the icons swell as on macOS: 1.5× for the hovered icon, 1.2×
and 1.1× for its neighbours, with lifts of 10, 6 and 0 px.

Pins are kept in `$XDG_STATE_HOME/amane-shell/dock-pins`, one a line:
`app_id<TAB>output<TAB>name:<workspace name>` or `…<TAB>index:<place>`. Only an app with a desktop
entry (`app_icon::entry_for`) can be pinned, since that's how it's opened again.

## Configuring the motion

Every animation is in `DOCK` in `dock/motion.rs`, one `Anim` each (`Anim::spring(ms, bounce)`,
`Anim::ease(ms)`, `.off()` to switch one off):

| Field | Moves |
|---|---|
| `reveal`, `hide_delay` | coming up and going down; how long it stays after the pointer leaves |
| `follow_overview` | whether it rises with the overview |
| `resize`, `item` | the card widening; icons growing in, sliding to new places and making way for a held one |
| `magnify` | the swell's scales and lifts, `smooth` (follow the pointer across an icon, or step like the reference CSS), its `hover` coming and going; `None` turns magnification off |
| `tooltip` | the name fading in; `None` shows no name |
| `spring_load` | how long files wait over an icon before its window comes into view; `None` waits for the drop |

`AMANE_DOCK_ANIMATIONS=off` (or `0`, `false`, `no`) turns every animation off at once. The dock
then jumps instead of moving, but still magnifies, names and hides as configured. The view never
checks the flags: `motion::anim` makes each animation, and gives back one that arrives at once
when it's off.

## How it's built

| File | What |
|---|---|
| `mod.rs` | the surface, per-monitor state, the row and the frame's layout; the still layer's handlers |
| `model.rs` | sections and icons from niri's windows and workspaces and the pins (pure, tested) |
| `pins.rs` | the pins, kept in the state file; opening an app (with files) on a workspace |
| `drag.rs` | where a held window goes, the make-way preview and settling (pure, tested) |
| `magnify.rs` | swell and spread (pure, tested) |
| `item.rs` | an icon, its running dot and its name label (inert) |
| `motion.rs` | `DockMotion` |
| `window_menu.rs` | a window's and a pinned app's right-click menus |

It reuses the shared [`EdgeReveal`, `workspace_menu`, `app_icon`, `Divider::vertical` and
`glass_region`](components.md).

- **One size.** The surface spans the monitor and is as tall as the card at full icon size plus
  room for the swell and the name, so it never resizes, even as icons shrink to fit; a resize every
  frame costs a compositor round-trip. It is `Layer::Overlay` with `Zone::Ignore`, reaching the
  real bottom edge over the bar. While hidden, its input region is a 2 px strip along that edge.
  While out, the region is the card's column too.
- **The row.** Sections are laid out as one row of places, each an icon, a rule (narrower than a
  slot, never swelling) or an empty workspace's place. Each place keeps its own slide and growth
  by a key (`win:<id>`, `pin:<section>:<app_id>`, `rule:<section>`), so a window moving to
  another workspace slides there instead of popping.
- **A still layer takes the pointer.** A clear layer over the whole surface comes first in the
  frame and holds every handler: hover, motion, click, drag, release, files over and drop. The
  pointer is measured against the row **at rest**, so the swell can't feed back on itself, and the
  icons themselves are inert. (amane finds hovered and dragged targets by their place in the frame;
  see `quick_settings/edit.rs`.)
- **Dragging** starts 6 px from the press, so a click still clicks; the release that ends a drag
  isn't taken as a click. Where the held window goes is worked out from the row without it, never
  from the last preview, and the preview is that row with the window put there. Let go, it's sent
  with `Windows::move_to` (niri's `MoveWindowToWorkspace` with `focus: false`, then
  `MoveColumnToIndex` with the window's id), and shown where it went until niri reports it there
  (or 800 ms pass), so it doesn't jump back for a frame.
- **Opening on a workspace** focuses the workspace and spawns the app through amane's worker, in
  that order, as the launcher's "Open in Workspace" does. A window of that app turning up on
  another workspace within 10 s (a slow app, the pointer gone elsewhere) is moved there.
- **File drops** come from amane's `on_files_over` (the files are read as they come in, so an
  icon can say whether its app opens them, and the pointer shows it) and `on_drop`. Files open
  with the entry's `Exec`, its `%f`/`%F`/`%u`/`%U` filled in (`DesktopApp::commands_with`).
- **Glass** is Quick Settings': `Material::Control.shape` frosts behind the card (and the name
  label), and `Material::Control.lit` adds the shader's edge where the compositor has no glass
  material. The niri layer rule is `clear` (`xray false` only); the surface says where to frost.

## Known limits

- Reordering needs the niri fork (`MoveColumnToIndex` with a `window_id`). Stock niri ignores the
  id and moves the focused column.
- Dragging a window that shares its column with others moves the whole column, as niri does.
- After spring-loading, the dock goes down once the files leave it, like after the pointer.
- The name label is drawn inside the surface, not as a [tooltip](components.md#tooltip) popup.
  amane hangs a popup under the first surface with the parent's namespace, and every monitor's
  dock shares `amane.dock`. For the same reason, an icon's menu opened from a dock on a second
  monitor may open on the first.
- A window that closes leaves at once; its neighbours slide together, but it doesn't shrink away.
