# Taskbar

A bottom layer-shell bar (`src/components/taskbar/mod.rs`), full-width, anchored to the bottom on
the `Top` layer with `Zone::Reserve`. Namespace `amane.taskbar`. The root is an opaque rounded
`Rectangle` holding a three-zone `Stack`. The bar is intentionally **not** blurred (no layer-rule
for its namespace); only the floating surfaces (popups, Quick Settings, the notification center) are frosted.

## Height and the overview

The bar keeps its compact height (`sys.size.panel.height`) throughout, the overview included. The
open apps are the [dock](dock.md)'s, which rises over the bar from the bottom edge and while niri's
overview opens, in step with its zoom.

## Zones

- **Left** (`left_section`): the overview toggle button (`Windows::toggle_overview()`) followed by
  one **workspace pip** per `Workspaces::read().list()` entry. A pip widens/brightens when its
  workspace is active or focused; left-click focuses it (`Workspaces::focus(id)`).
- **Center** (`focused_title`): the focused window's title. A window's context menu
  (`open_window_menu`) is the dock's now, opened by right-clicking its icon.
- **Right** (`right_section`): first, GNOME's privacy icons while the screen is shared, the
  camera runs or something records, and glyphs for night light, a VPN, a power mode other than
  Balanced and airplane mode while they're on (each opens Quick Settings). Then the optional status icons (see
  [below](#status-icons)), each opening its Quick Settings page: volume (scroll to change; the
  glyph shows what the default output is — built-in speaker, speaker, headphones, headset, TV,
  car …, and a [tooltip](components.md#tooltip) shows the level), Bluetooth (only with an
  adapter; off / on / connected), network (signal wedges follow the joined network's strength, a
  cable when wired), Tailscale (its nine-dot mark, only while connected; accent while routing
  through an exit node, a tooltip with the address or the exit node) and battery (a tooltip
  shows the charge). Then the Quick Settings button (see
  [quick-settings.md](quick-settings.md)), the notification indicator (a moon while Do Not Disturb is on, a bell while
  something arrived unseen; right-click for Do Not Disturb), clock (click → notification
  center, see [notifications.md](notifications.md)).

## Status icons

Volume, Bluetooth, network, Tailscale and battery are optional: each Quick Settings page that
has one (Sound, Bluetooth, Wi‑Fi, Tailscale, Battery) ends in a **Show in Taskbar** switch, and IPC does the same:

```sh
printf 'taskbar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock   # show | hide | toggle
```

The names are `volume`, `bluetooth`, `network`, `tailscale` and `battery`. The hidden ones are kept in
`$XDG_STATE_HOME/amane-shell/taskbar`, one per line (`indicators.rs`); a missing file shows them
all.

`panel.rs` holds the one-open-at-a-time state Quick Settings and the notification center share
(`Panel`, `toggle`, `close`) and the overlay-surface helpers (`overlay`, `dismissable`,
`popover_margin`) they and the toasts are built on.

A battery reads low from UPower's warning level, or at 15 % and below when nothing gives one
(`battery_low`, `LOW_BATTERY`), so the bar and Quick Settings agree.

## Related

- Window actions and the `Windows`/`Gestures` services are documented in the library:
  [../../amane/docs/compositor.md](../../amane/docs/compositor.md).
- The per-window menu is the shell's context menu: [context-menu.md](context-menu.md).
- Sizes and colours are Penpot design tokens: [design-token-manual.md](design-token-manual.md).
- Surface namespaces & niri layer-rules: [layer-rules.md](layer-rules.md).
