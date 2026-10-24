# Taskbar

A bottom layer-shell bar (`src/components/taskbar/mod.rs`), full-width, anchored to the bottom on
the `Top` layer with `Zone::Reserve`. Namespace `amane.taskbar`. The root is an opaque rounded
`Rectangle` holding a three-zone `Stack`. The bar is intentionally **not** blurred (no layer-rule
for its namespace); only the floating surfaces (popups and the panels below) are frosted.

## Height & the overview smooth-expand

`bar()` reads two compositor signals:

- `Windows::read().overview()` — the boolean overview open/closed state; it decides the **center
  content swap** (it flips when the gesture commits).
- `Gestures::read().overview_progress()` — the continuous `0.0..1.0` overview progress from the
  niri gesture-IPC fork; it drives the **height**:

  ```rust
  let progress = Gestures::read().overview_progress().clamp(0.0, 1.0) as f32;
  let compact = sys::size::panel::height();
  let height = compact + (bar::overview::height() - compact) * progress;
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
- **Right** (`right_section`): first, GNOME's privacy icons while the screen is shared, the
  camera runs or something records, and glyphs for night light, a VPN, a power mode other than
  Balanced and airplane mode while they're on (each opens Quick Settings). Then volume (scroll to
  change, click → Sound panel; the glyph shows what
  the default output is — built-in speaker, speaker, headphones, headset, TV, car …, and a
  [tooltip](components.md#tooltip) shows the level), Bluetooth
  (only with an adapter; off / on / connected, click → Bluetooth panel), network (click → Wi‑Fi
  panel; signal wedges follow the joined network's strength, a cable when wired), battery (click →
  power panel; a tooltip shows the charge), the Quick Settings button (see
  [quick-settings.md](quick-settings.md)), the notification indicator (a moon while Do Not Disturb is on, a bell while
  something arrived unseen; right-click for Do Not Disturb), clock (click → notification
  center, see [notifications.md](notifications.md)). Right-clicking volume, Bluetooth or
  network opens Quick Settings on that page.

## Panels

Four bottom-right popover cards share `panel.rs`: one `LayerWindow` each (namespaces
`amane.taskbar.{audio,bluetooth,wifi,power}`, each with a frosted layer-rule), but **only one is open
at a time** — `panel::toggle(Panel::…)` closes whichever other one is up. Escape or toggling again
closes it. `panel.rs` also holds the overlay-surface helpers (`overlay`, `dismissable`) the
notification center and toasts use. Each panel is built from the
[shared components](components.md): a frosted `Card` (`panel::card`), a `TopAppBar`, `List`s of
`ListItem`s and a `Slider`, plus the buttons and `Switch` from [buttons.md](buttons.md).

- **Sound** (`audio.rs`): for the output and for the microphone, a mute button showing the
  default device's glyph, a volume slider and the device list; clicking a device makes it the
  default (`Audio::set_output` / `set_input`), and PipeWire moves what is playing along. Device
  kinds come from the amane fork (`AudioKind`, see `amane/docs/audio.md`).
- **Bluetooth** (`bluetooth.rs`): power switch, then "My Devices" (paired) and "Other Devices"
  (found by the search that runs only while the panel is open). Click connects, disconnects or
  pairs; the row says "Connecting…" until BlueZ reports back (up to 15 s); right-click a paired
  device for Connect/Disconnect and Forget. Battery shows when the device reports it. **Pairing
  works only for devices that need no PIN** — the fork has no BlueZ agent to show or type a code.
- **Wi‑Fi** (`wifi.rs`): power switch and rescan, the networks found, and a password field for a
  secured one that isn't saved.
- **Power** (`power.rs`): the battery and its time left, the power-profile segmented button, and
  the batteries of connected peripherals.

A battery reads low from UPower's warning level, or at 15 % and below when nothing gives one
(`battery_low`, `LOW_BATTERY`), so the bar and the power panel agree.

## Related

- Window actions and the `Windows`/`Gestures` services are documented in the library:
  [../../amane/docs/compositor.md](../../amane/docs/compositor.md).
- The per-window menu is the shell's context menu: [context-menu.md](context-menu.md).
- Sizes and colours are Penpot design tokens: [design-token-manual.md](design-token-manual.md).
- Surface namespaces & niri layer-rules: [layer-rules.md](layer-rules.md).
