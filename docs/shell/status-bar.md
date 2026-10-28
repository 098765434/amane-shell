# Status bar

A layer-shell bar (`src/surfaces/status_bar/mod.rs`), full-width, anchored to the bottom (or the
top, see [Settings](#settings)) on the `Top` layer with `Zone::Reserve`. Namespace `amane.status-bar`. The root is a rounded
`Rectangle` holding one `Row` justified `SpaceBetween`: workspaces on the left and the status
cluster on the right. The bar is one uniform frosted strip, like macOS 27's toolbar: its layer
rule frosts the whole surface in niri's `frost` material (no lens, square corners), and the bar
fills it with `Material::Bar`'s tint (`sys.color.material.bar`, as thick as a sheet's), since it
holds text. It reserves its zone,
so only the wallpaper is ever behind it, and niri's cheaper xray blur is enough. The dock's glass
rises over it. Its popups (tooltips, Do Not Disturb's menu) are frosted too. It is `Keyboard::None`, so clicking it never takes
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
`::active` ([controls.md](../kit/controls.md)); the bar's `item()` helper applies it to every item.

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
6. The **timer** chip, only while the launcher's timer runs (`services/launcher/timer.rs`): a timer glyph
   and the time left in `sys.type.numeric`, counting down by the second. Its tooltip is the
   timer's label; a click opens the launcher in Actions, where it can be cancelled (see
   [launcher.md](launcher.md#actions)). amane has no timers, so the bar asks for a frame just
   after each second turns over (`motion::redraw_after`).
7. The **clock**: click it for the notification center (see [notifications.md](notifications.md));
   its tooltip is the date (`Clock::date`).

Hidden items take no space: each section yields `Option`s that are flattened, so the row's gaps
stay even.

## Status icons

Volume, Bluetooth, network, Tailscale, Keep Awake and battery are optional, and so are three
groups: `privacy` (screen sharing, camera, microphone), `status-icons` (Night Light, VPN, power
mode, Airplane Mode) and `notifications` (the bell or the moon). Each Quick Settings page that has
one ends in a **Show in Status Bar** switch; Shell Settings' Status Bar page has a switch for
each, and the `status-bar` IPC command does the same ([ipc.md](../integration/ipc.md#status-bar)).
The hidden ones' names are `hidden` in `status-bar.json` (`indicators.rs`), which the bar follows
live. At start, a shell with no `status-bar.json` yet brings them over once from the state file
they were kept in before (`$XDG_STATE_HOME/amane-shell/status-bar`, else the older `taskbar`).

## Settings

`~/.config/amane-shell/status-bar.json` (`amane_kit::settings::StatusBar`, Shell Settings' Status
Bar page), followed live:

| Field | Does |
|---|---|
| `position` | `bottom` or `top`. Quick Settings (`panel::popover_margin`, its anchor and its rise), the dock's gap, the widgets' canvas and gallery, and Do Not Disturb's menu follow `status_bar::edge()`; the notification center and toasts already keep clear of the reserved zone |
| `style` | `frosted`: the layer rule's frost and `Material::Bar`'s tint. `glass`: one blur shape in `Material::Control`'s glass over the strip. `solid`: no blur, black in dark mode and white in light, not vibrant |
| `clock_24h`, `clock_seconds` | the clock's format; `Clock` signals once a minute, or a second while seconds show |
| `date_format` | the clock tooltip's day: `long` ("Sunday 25 October"), `short` ("Sun 25 Oct"), `numeric` (the locale's `%x`) |
| `show_workspaces`, `scroll_workspaces` | the pips, and whether scrolling over them moves between workspaces |
| `hidden` | the status icons left out (above) |
| `auto_hide` | slides the bar past its edge (an [`EdgeReveal`](../kit/components.md#edgereveal) at the top or bottom) and keeps no room from windows (`Zone::Ignore`). It comes back while the pointer is on a 2 px strip at the edge or on the bar, or while Quick Settings or the notification center is open, and goes again 400 ms after. Its frost becomes a blur shape that slides with it. At the bottom, a hiding dock (Overlay) takes the edge first |

Every style draws only rectangles and glyphs (the quad pipeline); the glass is niri's, not a
shader.

## Shared pieces that live elsewhere

- `components/panel.rs`: the one-open-at-a-time state that Quick Settings, the notification
  center and the launcher share (`Panel`, `toggle`, `close`), plus the overlay-surface helpers
  they and the toasts are built on (`overlay`, `dismissable`, `popover_margin`).
- `components/battery.rs`: a battery reads low from UPower's warning level, or at 15 % and below
  when nothing gives one (`battery::low`, `battery::LOW`), so the bar and Quick Settings agree.
  `battery::color` and `battery::tint` give the colour that says so.
- `services/launcher/clock.rs`: the `Clock` service (time and date), also read by the notification center.
- `dock/window_menu.rs`: a window's and a pinned app's context menus, opened from the dock. Their
  "Move to Workspace" and "Open in Workspace" submenus are the shared
  `surfaces/context_menu/workspace_menu.rs`, which the launcher's app menu uses too.

## Related

- Window actions and the `Windows`/`Gestures` services are documented in the library:
  [amane `compositor/services.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/compositor/services.md).
- Sizes and colours are Penpot design tokens: [tokens.md](../design-system/tokens.md).
- Surface namespaces & niri layer-rules: [layer-rules.md](../integration/layer-rules.md).
