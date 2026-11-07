# Status bar

A layer-shell bar (`apps/shell/src/surfaces/status_bar/mod.rs`), full-width, anchored to the bottom (or the
top, see [Settings](#settings)) on the `Top` layer with `Zone::Reserve`. There is one on every
monitor (`App::window_per_monitor`): each shows the
workspaces of its own monitor and hides on its own, and what it opens (Quick Settings, the
notification center, menus and tooltips) opens on that monitor (`panel::on_output`, from
`amane::pointer_monitor`). Namespace `amane.status-bar`. The root is a rounded
`Rectangle` holding one `Row` justified `SpaceBetween`: workspaces on the left and the status
cluster on the right. The bar is one uniform frosted strip: its layer
rule frosts the whole surface in niri's `frost` material (no lens, square corners), and the bar
fills it with `Material::Bar`'s tint (`sys.color.material.bar`, as thick as a sheet's), since it
holds text. It reserves its zone,
so only the wallpaper is ever behind it, and niri's cheaper xray blur is enough. The dock's glass
rises over it. Its popups (tooltips, Do Not Disturb's menu) are frosted too. It is `Keyboard::None`, so clicking it never takes
focus from the active app.

## Size, height and the overview

The bar comes in three sizes (`size`, Shell Settings' Status Bar → Size), all in
`status_bar/size.rs` (`Metrics`):

| Size | Height | Glyphs | Item width | Text |
|---|---|---|---|---|
| Compact (the default, the bar it always was) | 18 (`sys.size.panel.height`) | 16 (`sys.size.icon.m`) | 28 (`bar.indicator.width`) | 13 px |
| Regular | 24 | 16 | 30 | 13 px |
| Comfortable | 32 | 24 | 40 | 14 px |

All bar text (the clock, the weather, programs' titles, activities' text, the keyboard
layout) is on a 16 px line, in `sys.type.label`'s weight, or `sys.type.numeric`'s for figures
(`Metrics::label`, `::numeric`). Tray pictures, activities' leading pictures (a glyph plus 2),
the privacy chips (three quarters of a glyph) and the analog clock grow with the glyph. Regular's
and Comfortable's numbers wait for Penpot ([penpot-backlog.md](../design-system/penpot-backlog.md)).

What keeps clear of the bar asks `status_bar::height()`: Quick Settings and the other cards
(`panel::popover_margin`), the dock's gap, the widgets' canvas and gallery. Apps outside the shell
(amane-drop) read `BarSize::height()` from the kit.

The bar keeps its height throughout, the overview included. The
open apps are the [dock](dock.md)'s, which rises over the bar from the bottom edge and while niri's
overview opens, in step with its zoom. niri's own keybind (Mod+O) toggles the overview.

## Items

Every item is a bar-height button (the size's item width) with a tooltip. While hovered, it
turns into a white rounded tile (`bar.indicator.active.bg`, `sys.radius.raised`) with its glyph
black (`bar.indicator.active.fg`). It stays that way while what it opens is open:

- each status icon, while Quick Settings shows its page;
- the Quick Settings button, while Quick Settings is open;
- the notification icon and the clock, while the notification center is open.

The glyphs that open Quick Settings' grid (privacy, the "on" glyphs) only light on hover, or
opening Quick Settings would light them all. The look comes from `ButtonBuilder::highlight` and
`::active` ([controls.md](../kit/controls.md)); the bar's `item()` helper applies it to every item.

### Left: workspaces (`workspaces`, `workspace_shape.rs`)

One **shape** says which of the monitor's workspaces shows: the first a circle, then a square, a
triangle, a diamond, a pentagon, a hexagon and a five-point star; after those a heptagon, an
octagon, and stars with 6, 7, 8… points. It sits in a 16 px box, the text's line, whatever the
bar's size, and turns red while any workspace there is urgent.

It morphs as niri moves: each shape is its edge's distance from its middle at 120 even angles,
corners softened by averaging neighbours, scaled to cover the area of a 13 px circle (or as much
as fits) and centred. Part way between two workspaces it's the two mixed (smoothstep-eased, so it
rests near the true shapes) and turned up to 15°. The position is niri's fork's
`WorkspaceSwitchProgress` (`WorkspaceSwitch`), so it follows a swipe's finger, a cancelled swipe and
the keyboard's spring; `switch_position` keeps a rubber band at the end shape and settles within
0.01 of a landing (the widgets' fade shares it). niri reports only the focused monitor, so on the
others the shape springs (280 ms, no bounce) to the workspace shown, and that spring is kept where
niri last said so nothing jumps when focus leaves.

It's one closed straight-sided `Path` in a `Canvas`, so it draws as a single quad outline (no
vello). Clicking it opens niri's overview (`Windows::toggle_overview`); its tooltip is the
workspace's name or "Workspace N". Scrolling over it focuses the next or previous workspace: one per
wheel notch, or one per `SCROLL_STEP` lines of touchpad scroll, stopping at either end.

### Right: the status cluster (`status`)

Every item has a name (`indicators.rs`, `Item`), and `StatusBar::order` keeps their order by those
names (`BAR_ITEMS` and `StatusBar::items` in kit's settings, shared with Shell Settings). Items it
leaves out (such as ones added later) go back after the item they usually follow, and names it
doesn't know are skipped. Over IPC: `status-bar move <item> <index>` and `status-bar reset-order`.

**Dragging** (`drag.rs`) needs no held key: the bar never has the
keyboard, so it can't see a held key, and a press that moves more than 6 px picks the item up
instead (a click stays a click). The others close up as it passes, letting go keeps the new order,
and letting go more than 40 px off the bar hides an item that can be hidden (it dims on the way).
The drag is measured on one still layer over the whole cluster, never on the item that moves, and
the click that ends a drop is swallowed (`buttons::swallow_click`). The layer takes the items'
own press, so they don't shrink when pressed. The usual order, left to right:

1. **Apps** (the tray, [below](#apps-the-tray)).
2. **Privacy** (`chips.rs`): a green camera while a camera runs; an orange
   microphone while an app records. Screen sharing is an [activity](#activities). Their tooltips name the apps, and the dots' menu lists them by
   camera and microphone; picking one brings its window up (found by its process, or its parent's,
   in niri's window list), and Sound Settings… opens Quick Settings' Sound page.
3. Glyphs for settings that are **on**: night light, a VPN, a power mode other than Balanced, and
   airplane mode. Each opens Quick Settings.
4. **Activities** (`activities.rs`, [below](#activities)): Now Playing, the timer, screen
   sharing and other programs' activities.
5. The optional **status icons** (see [below](#status-icons)), each opening its Quick Settings page:
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
   - **Phone**: a phone through KDE Connect, while it's connected and has sent its charge;
     accent while charging, red when low; the tooltip shows its name and charge
     ([phone.md](phone.md)).
   - **Battery**: accent while charging, red when low; the tooltip shows the charge.
6. The **Quick Settings** button (see [quick-settings.md](quick-settings.md)).
7. The **notification** icon: a moon while Do Not Disturb is on, a bell while something arrived
   unseen, nothing otherwise. Click it for the center, right-click for Do Not Disturb.
8. The **clock**: click it for the notification center (see [notifications.md](notifications.md));
   its tooltip is the date (`Clock::date`).

Hidden items take no space: each section yields `Option`s that are flattened, so the row's gaps
stay even.

## Apps (the tray)

The first item, **Apps** (`tray.rs`), is the system tray's icons from
amane's `Tray` (StatusNotifierItem, see amane's `docs/fork/system/tray.md`), for Steam, Discord,
nm-applet and the like, in the order they came. Each is the app's picture (16 px; a dot for one that
sent none), ringed in the accent while the app wants attention. Apps whose item is `Passive` (KDE
Connect's indicator with no phone, say) don't show.

- **Left click**: the app's action (`Activate`), or its menu when it has only a menu.
- **Right click**: its menu, read over dbusmenu as it opens and shown as the shell's own context
  menu: ticks as a ✓ or • before the label, entry icons, submenus. An app without a dbusmenu is
  asked to show its own (`ContextMenu`).
- **Middle click**: its second action. **Scrolling** goes to it.

**Allow in the Status Bar**: Shell Settings lists every app seen in the tray
(`~/.cache/amane-shell/tray-seen.json`), each with a switch; one switched off stays out of the bar
(`StatusBar::tray_denied`, by the item's id) and keeps running.

**Overflow**: when the right-hand items would be wider than the bar leaves them (its
width less the workspaces and insets), the last apps go behind a » button, which lists them in a
menu, each doing what its click would.

Over IPC: `status-bar tray list`, `status-bar tray activate <id>` and `status-bar tray menu <id>`.

## Programs' items

Other programs put their own items in the bar over its bus service, `org.amane.StatusBar1`. Each
is a status item, put up with amane's `StatusItem` client or plain JSON over
`busctl` ([status-apps.md](../integration/status-apps.md)). The server is
`services/status_apps.rs` and the drawing is `apps.rs`.

- **What shows.** Each item is its picture (one of the bar's symbols, a theme icon or a file),
  then an optional title cut at 120 px and a badge. It shows in the accent while it wants
  attention.
- **Clicks.** A click and a middle click go back to the program, with the item's middle on its
  monitor and the bar's edge, so it can open its own window under the item.
- **Menus.** A right-click shows the item's menu as the shell's context menu, and a pick goes
  back to the program. An item with no menu hears the right-click instead.
- **Drops.** An item that accepts files or text takes them when they're let go on it
  (`Rectangle::on_drop`). One that opens on drag hears as soon as files come over it, so the
  program can open a window to drop them on.
- **Order and hiding.** Items keep their place in `order` as `app:<id>`. A new item goes just
  after the tray. Items drag and hide like the bar's own, and `hidden` keeps the hidden ones by
  the same name.
- **Allow in the Status Bar.** Shell Settings lists every program seen
  (`~/.cache/amane-shell/status-apps-seen.json`).
- **Ownership.** An item belongs to the program that registered it: no other program may use
  its id. It goes when that program leaves the bus.

## Activities

What's going on now, shown in the bar until it ends (`activities.rs`).

**Sources**

- The bar's own are read straight from their services:
  - **Now Playing**: the art and title, while a player plays or was paused in the last five
    minutes. `media_always` shows it always, as "Not Playing" when nothing plays. Its buttons
    are previous, play/pause and next, plus the player in Quick Settings. A middle click plays
    or pauses.
  - **Timer**: the launcher's timer counting down, in orange, with Cancel and Open in the Launcher.
  - **Screen Sharing**: purple, with what each niri cast shares and Stop Sharing for each.
- Other programs' come over `org.amane.StatusBar1` and are kept in
  `services/desktop/activities.rs`.

**Presentation**

- **Ranking.** Each activity has a relevance: Sharing 0.9, the timer 0.7, Now Playing 0.3, and
  other programs say their own. The most relevant `activities_compact` (1–3, 2 by default) show
  **compact**: the picture and the trailing text, countdown, count-up or progress ring.
- **Minimal.** The rest show their picture alone. The bar's overflow takes the tray's apps
  first.
- **Ticking.** A countdown or count-up is ticked by the bar, a frame just after each second
  turns over (`motion::redraw_after`). Its program sends nothing in between.
- **Dimming.** An activity is dimmed once it is out of date (`stale_after` without an update),
  and while it shows its last state after being ended.

**The expanded view**

- A click opens the expanded view under the bar, an xdg-popup of the bar
  (`amane.status-bar.activity`, frosted by the bar's layer rule). It shows the title and
  subtitle, the trailing part large, a progress bar, and up to three buttons, whose presses go
  back to the program.
- An update that alerts brings the view up for 4 s, without a grab, but not under Do Not
  Disturb.

**Limits**

- An activity ends after 8 hours, or when its program leaves the bus.
- Shell Settings' Status Bar › Activities has the switch, how many show compact, Now
  Playing's "Always show", and which apps may show them (`activities_denied`, by the name they
  give). The bar's own three are always listed; others are listed once seen
  (`activities-seen.json`).

Every piece is rectangles, glyphs and arcs (the quad pipeline): `AMANE_FRAMES=1` with
`amane/vello-stats` shows `vello 0` on the bar and the view.

## Status icons

Volume, Bluetooth, network, Tailscale, Keep Awake, the phone and battery are optional, and so are four
groups: `activities` (the activities), `privacy` (camera, microphone), `status-icons` (Night
Light, VPN, power mode, Airplane Mode) and `notifications` (the bell or the moon). A `media`
hidden before Now Playing became an activity is kept out as one (`indicators::migrate`). Each Quick Settings page that has
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
| `style` | `frosted`: the layer rule's frost and `Material::Bar`'s tint. `glass`: one blur shape in `Material::Control`'s glass over the strip. `solid`: no blur, black in dark mode and white in light, not vibrant. `clear`: no strip; one all but invisible clear-glass shape (2%) with an id, so niri reads the wallpaper under it, and the items built under `Material::Control.adapted`, so their labels are light or dark to suit it |
| `clock_24h`, `clock_seconds` | the clock's format; `Clock` signals once a minute, or a second while seconds show |
| `clock_style` | `digital`, or `analog`: a small drawn face whose two hands are frames the size of the face, turned (`Rectangle::rotate`); its tooltip has the time |
| `show_date` | the date before the time: `never`, `always`, or `when-space`: dropped before any app's icon goes behind » |
| `show_weekday` | the day of the week before the time (`StatusBar::clock_pattern`) |
| `flash_separators` | the colons blink each second; a figure space stands in, and the clock keeps the width it has with colons. `Clock` signals each second while on |
| `media_always` | Now Playing shows with nothing playing too |
| `activities_compact`, `activities_denied` | how many activities show with their text (1–3), and the apps whose activities stay out, by name |
| `weather`, `weather_place` | the Weather item (`services/desktop/weather.rs`): Open-Meteo, the place found once by its geocoding, read every half hour and whenever the place changes; nothing is fetched while it's off. Its glyph and temperature, the tooltip the place, the conditions and the day's high and low; a moon on clear nights |
| `order`, `tray_denied` | the items' order and the apps kept out (above); programs' items are in `order` as `app:<id>`. The older `media` and `timer` read as `activities` |
| `date_format` | the clock tooltip's day: `long` ("Sunday 25 October"), `short` ("Sun 25 Oct"), `numeric` (the locale's `%x`) |
| `size` | `compact` (the default), `regular` or `comfortable` (above) |
| `show_workspaces`, `scroll_workspaces` | the workspace's shape, and whether scrolling over it moves between workspaces |
| `hidden` | the status icons left out (above) |
| `auto_hide` | `never`, `always`, `desktop` (while the bar's monitor shows a workspace with no windows) or `fullscreen` (while a window on that workspace is fullscreen: `WindowInfo::fullscreen`, which needs the niri fork's `is_fullscreen`); the old `true`/`false` read as `always`/`never`. Hiding slides the bar past its edge (an [`EdgeReveal`](../kit/components.md#edgereveal) at the top or bottom) and keeps no room from windows (`Zone::Ignore`). It comes back while the pointer is on a 2 px strip at the edge or on the bar, or while Quick Settings or the notification center is open, and goes again 400 ms after. Its frost becomes a blur shape that slides with it. At the bottom, a hiding dock (Overlay) takes the edge first |

Every style draws only rectangles and glyphs (the quad pipeline); the glass is niri's, not a
shader.

## Keyboard layout

With two or more layouts, a short code for the one in use ("NO", "US"), from
`services/desktop/keyboard_layout.rs`, which follows `niri msg --json event-stream`. A click lists
the layouts with a tick on the current one; picking one runs `niri msg action switch-layout`.

## Shared pieces that live elsewhere

- `components/panel.rs`: the one-open-at-a-time state that Quick Settings, the notification
  center and the launcher share (`Panel`, `toggle`, `close`), plus the overlay-surface helpers
  they and the toasts are built on (`overlay`, `dismissable`, `popover_margin`).
- `components/battery.rs`: a battery reads low from UPower's warning level, or at 15 % and below
  when nothing gives one (`battery::low`, `battery::LOW`), so the bar and Quick Settings agree.
  `battery::color` and `battery::tint` give the colour that says so.
- `services/clock.rs`: the `Clock` service (time and date), also read by the notification center.
- `dock/window_menu.rs`: a window's and a pinned app's context menus, opened from the dock. Their
  "Move to Workspace" and "Open in Workspace" submenus are the shared
  `surfaces/context_menu/workspace_menu.rs`, which the launcher's app menu uses too.

## Related

- Window actions and the `Windows`/`Gestures` services are documented in the library:
  [amane `compositor/services.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/compositor/services.md).
- Sizes and colours are Penpot design tokens: [tokens.md](../design-system/tokens.md).
- Surface namespaces & niri layer-rules: [layer-rules.md](../integration/layer-rules.md).
