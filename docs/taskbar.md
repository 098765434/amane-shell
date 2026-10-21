# Taskbar

A bottom layer-shell bar (`src/components/taskbar/mod.rs`), full-width, anchored to the bottom on
the `Top` layer with `Zone::Reserve`. Namespace `amane.taskbar`. The root is an opaque rounded
`Rectangle` holding a three-zone `Stack` — the bar is intentionally **not** blurred (no layer-rule
for its namespace); only the floating surfaces (popups, Wi-Fi panel) are frosted.

## Height & the overview smooth-expand

`bar()` reads two compositor signals:

- `Windows::read().overview()` — the boolean overview open/closed state; it decides the **center
  content swap** (it flips when the gesture commits).
- `Gestures::read().overview_progress()` — the continuous `0.0..1.0` overview progress from the
  niri gesture-IPC fork; it drives the **height**:

  ```rust
  let progress = Gestures::read().overview_progress().clamp(0.0, 1.0) as f32;
  let height = t.height_compact + (t.height_overview - t.height_compact) * progress;
  ```

So the bar grows/shrinks smoothly in lockstep with niri's overview zoom (and previews-then-settles
if you cancel a partial swipe), instead of snapping. niri animates `overview_progress` itself, so
no client-side easing is needed. Without the gesture fork the value stays `0.0` and the bar simply
uses its compact height until the overview bool flips.

## Zones

- **Left** (`left_section`): the overview toggle button (`Windows::toggle_overview()`) followed by
  one **workspace pip** per `Workspaces::read().list()` entry. A pip widens/brightens when its
  workspace is active or focused; left-click focuses it (`Workspaces::focus(id)`).
- **Center** (`center_section`): `focused_title` (the focused window's title) when compact, or the
  interactive `window_dock` (a tile per open window) when the overview is open. Dock tiles show the
  app icon + heading/title; left-click focuses (`Windows::focus`), right-click opens the per-window
  context menu.
- **Right** (`right_section`): volume (scroll to change, click to mute), network (click → Wi‑Fi
  panel), battery, clock.

## Related

- Window actions and the `Windows`/`Gestures` services are documented in the library:
  [../../amane-fork/docs/compositor.md](../../amane-fork/docs/compositor.md).
- Context menus use the library's popup support:
  [../../amane-fork/docs/popups.md](../../amane-fork/docs/popups.md).
- Surface namespaces & niri layer-rules: [layer-rules.md](layer-rules.md).
