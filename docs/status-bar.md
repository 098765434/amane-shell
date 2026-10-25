# Status bar

A bottom layer-shell bar (`src/components/status_bar/mod.rs`), full-width, anchored to the bottom
on the `Top` layer with `Zone::Reserve`. Namespace `amane.status-bar`. The root is a rounded
`Rectangle` holding one `Row` justified `SpaceBetween`: workspaces on the left and the status
cluster on the right. The bar itself is intentionally **not** blurred: its layer-rule only frosts
its popups (tooltips, Do Not Disturb's menu). It is `Keyboard::None`, so clicking it never takes
focus from the active app.

## Height and the overview

The bar keeps its compact height (`sys.size.panel.height`) throughout, the overview included. The
open apps are the [dock](dock.md)'s, which rises over the bar from the bottom edge and while niri's
overview opens, in step with its zoom. niri's own keybind (Mod+O) toggles the overview.

## Items

Every item is a bar-height button (`bar.indicator.width` wide) with a tooltip. While hovered, it
turns into a white rounded tile (`bar.indicator.active.bg`, `sys.radius.raised`) with its glyph
black (`bar.indicator.active.fg`). It stays that way while what it opens is open:

- each status icon, while Quick Settings shows its page;
- the Quick Settings button, while Quick Settings is open;
- the notification icon and the clock, while the notification center is open.

The glyphs that open Quick Settings' grid (privacy, the "on" glyphs) only light on hover, or
opening Quick Settings would light them all. The look comes from `ButtonBuilder::highlight` and
`::active` ([buttons.md](buttons.md)); the bar's `item()` helper applies it to every item.

### Left: workspaces (`workspaces`)

One **pip** per `Workspaces::read().list()` entry, each a `DrawnButton`. A pip widens and
brightens when its workspace is active or focused, and turns red when it's urgent. Clicking a pip
focuses its workspace, and its tooltip is the workspace's name or "Workspace N". Scrolling anywhere
over the pips focuses the next or previous workspace: one per wheel notch, or one per
`SCROLL_STEP` lines of touchpad scroll. Scrolling stops at either end.

### Right: the status cluster (`status`)

From left to right:

1. GNOME's **privacy** icons while the screen is shared, the camera runs or something records (red;
   the microphone's tooltip names the app). Each opens Quick Settings.
2. Glyphs for settings that are **on**: night light, a VPN, a power mode other than Balanced, and
   airplane mode. Each opens Quick Settings.
3. The optional **status icons** (see [below](#status-icons)), each opening its Quick Settings page:
   - **Volume**: the glyph shows the default output (built-in speaker, speaker, headphones,
     headset, TV, car …) and the tooltip shows the level. Scroll to change it, middle-click to
     mute.
   - **Bluetooth**: only with an adapter. Off, on, or connected (accent), with the connected
     devices' names in the tooltip.
   - **Network**: signal wedges follow the joined network's strength, a cable when wired. The
     tooltip shows the SSID, "Wired", "Offline" or "Wi‑Fi off".
   - **Tailscale**: its nine-dot mark, only while connected. Accent while routing through an exit
     node; the tooltip shows the address or the exit node.
   - **Keep Awake**: a cup, accent while it holds anything; the tooltip shows the time left.
   - **Battery**: accent while charging, red when low; the tooltip shows the charge.
4. The **Quick Settings** button (see [quick-settings.md](quick-settings.md)).
5. The **notification** icon: a moon while Do Not Disturb is on, a bell while something arrived
   unseen, nothing otherwise. Click it for the center, right-click for Do Not Disturb.
6. The **clock**: click it for the notification center (see [notifications.md](notifications.md));
   its tooltip is the date (`Clock::date`).

Hidden items take no space: each section yields `Option`s that are flattened, so the row's gaps
stay even.

## Status icons

Volume, Bluetooth, network, Tailscale, Keep Awake and battery are optional. Each Quick Settings
page that has one ends in a **Show in Status Bar** switch, and IPC does the same:

```sh
printf 'status-bar\nhide\nbattery\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock   # show | hide | toggle
```

The names are `volume`, `bluetooth`, `network`, `tailscale`, `battery` and `keep-awake`. The old
`taskbar` command still works. The hidden ones are kept in `$XDG_STATE_HOME/amane-shell/status-bar`,
one per line (`indicators.rs`), and a missing file shows them all. Until that file exists, the
old `taskbar` file is read instead.

## Shared pieces that live elsewhere

- `components/panel.rs`: the one-open-at-a-time state that Quick Settings, the notification
  center and the launcher share (`Panel`, `toggle`, `close`), plus the overlay-surface helpers
  they and the toasts are built on (`overlay`, `dismissable`, `popover_margin`).
- `components/battery.rs`: a battery reads low from UPower's warning level, or at 15 % and below
  when nothing gives one (`battery::low`, `battery::LOW`), so the bar and Quick Settings agree.
  `battery::color` and `battery::tint` give the colour that says so.
- `services/clock.rs`: the `Clock` service (time and date), also read by the notification center.
- `dock/window_menu.rs`: a window's context menu, opened from the dock.

## Related

- Window actions and the `Windows`/`Gestures` services are documented in the library:
  [../../amane/docs/fork/compositor.md](../../amane/docs/fork/compositor.md).
- Sizes and colours are Penpot design tokens: [design-token-manual.md](design-token-manual.md).
- Surface namespaces & niri layer-rules: [layer-rules.md](layer-rules.md).
