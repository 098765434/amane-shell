# Quick Settings

GNOME's quick settings menu, drawn as the macOS Control Center
(`src/components/quick_settings/`). As on macOS Tahoe the card itself has no background: it's a
clear surface above the bar's right end holding frosted glass modules, laid out as Control
Center is — Wi‑Fi and Bluetooth tiles beside Now Playing, the Focus tile and round buttons, a
Display card and a Sound card with their sliders, then the battery and more round buttons.
Each control can open a page of its own, and an edit mode moves, resizes, removes and adds them.

It's opened from the bar's Control Center button (the slider glyph, between the battery and the
notification bell), and closed by the same button, by Escape, or by opening the notification
center, whose one-open-at-a-time state it shares (`Panel::QuickSettings`). It's the only place
for sound, Wi‑Fi, Bluetooth and power: the bar's volume, Bluetooth, network and battery icons open
it straight on their page, and each of those pages has a **Show in Taskbar** switch for its icon
(see [taskbar.md](taskbar.md#status-icons)).

## What's in it

In Control Center's order (the default layout; edit mode changes it). Every control shows only
when the machine has what it drives, as in GNOME, and the grid packs around what's missing:

| Control | Shows | Click | Page |
|---|---|---|---|
| Wi‑Fi | the network joined, "Connecting…", "Not Connected", "Off" or "Hardware switch off"; "<ssid> Hotspot" while the machine is one | the disc switches the radio (or stops the hotspot); the tile opens the page | the networks (rescanned every 15 s while shown; "Searching…" or "Wi‑Fi is off" while there are none), joining with a password; the page stays open and the row says "Connecting…" |
| Bluetooth | the device connected, or how many | the disc switches the adapter; the tile opens the page | the device lists, searching while shown |
| Airplane Mode | a round button (or a tile) | switches every radio | — |
| Tailscale | a tile with Tailscale's mark (drawn, as the font hasn't got it), waiting in the Add list by default; "via <exit node>", this device's address, "Connecting…", "Not logged in", "Disconnected" or "Daemon not running"; hidden without the `tailscale` binary | the disc connects or disconnects (`tailscale up` / `down`), or logs in when logged out; the tile opens the page | what's wrong (no daemon; **Log In…**, which opens the browser once the control plane hands out the URL; no operator grant, with the command to copy; a failed change; tailscaled's own warnings), This Device (a click copies the address), Exit Node (None or a peer that offers itself; offline ones can't be picked), Accept Routes, Accept DNS and Block Incoming switches, and the tailnet's Devices, online first (a click copies the address); Show in Taskbar; Admin Console… |
| VPN, Wired | tiles, waiting in the Add list by default; VPN says "Off", the tunnel that's up or how many are; Wired the connection or "Not Connected" | the disc switches; the VPN tile opens its page | VPN: a switch in the header for the preferred tunnel, and one per profile |
| Now Playing | the active player's art (from a file or the web), title and artist, how far through the track, or "Not Playing" | previous, play/pause, next | — |
| Focus | Do Not Disturb: what's on, until when, or off | the disc switches it; the tile opens the page | Focus: For 1 Hour, Until This Evening, Until Tomorrow, the one that's on checked |
| Dark Style, Night Light | round buttons, accent while on | switch; a right click on Night Light opens its page | Night Light: how warm |
| Display | only with a backlight; the brightness slider between a dim and a bright glyph | drag or scroll | — |
| Sound | the default output's level; the small glyph says how loud (off, down, up) | the small glyph mutes; drag or scroll sets the level; the chevron or a right click opens the page | Sound: the output's level and percentage (its glyph mutes it) and its devices, then the microphone's level and its devices (with a microphone); the default device checked, a click makes another the default |
| Battery | only with a battery; the charge and how long it lasts, the disc in the critical colour when it's low | opens its page | Battery (time left, power mode, peripherals) |
| Round buttons | Power Mode (accent while not Balanced), Keyboard (the backlight), Background Apps (how many, in a tooltip), screenshot, lock, power off | switch; a right click opens the page; screenshot closes the card, then opens niri's screenshot UI; lock locks through logind | Power Mode: the modes; Keyboard: Off/Low/High (a slider for keyboards with many steps); Background Apps: the apps, each with a close button; Power Off: suspend, restart, power off, log out |

A control works like a macOS Control Center module. A click on the round disc switches the
setting, and a click anywhere else on a tile or a module row opens its page. A control with no
page switches from anywhere. A right click opens the page too; a round button has no label, so a
tooltip names it, and a tile whose lines are too long for it names itself the same way. The
Power Off page's Restart…, Power Off… and Log Out… ask for a second click within five seconds
(the row turns into "Click to Restart" in the critical colour), which stands in for GNOME's
confirmation dialog. Switch User is left out because greetd has no user switching.

A page is at least five rows tall, however short the grid is, and its lists scroll in the room
they have. A page whose setting goes away while it's open (the adapter unplugged) gives way to
the grid, and `quick-settings open` answers with an error for a page the machine can't show.

Now Playing is always there, saying "Not Playing" when nothing is, as on macOS; everything else
follows GNOME's rule and hides without its hardware.

A page's last row opens the app with the rest of its settings when that app is installed:
`nm-connection-editor` for Wi‑Fi and VPN, `blueman-manager` for Bluetooth and `pavucontrol` for
Sound.

The bar also shows GNOME's status icons, each only while it applies:

- **Privacy icons**, first: the screen being shared, the camera in use, and the microphone in use.
  The microphone's tooltip names the app.
- **Glyphs for settings that are on**: night light, a VPN, a power mode other than Balanced, and
  airplane mode.

Clicking any of them opens Quick Settings.

## Edit Controls

In edit mode the chrome (ghost discs, badges, handles) fades in, a control added grows into its
cells and one removed shrinks away, and when the grid gains or loses rows at rest the controls
slide to their new place rather than jump.

The grid is iOS Control Center's canvas: `quick.grid.cols` × `quick.grid.rows` (4 × 8) cells of
`quick.grid.cell` (74 px) with `quick.grid.gap` (16 px) between them, so a row is 344 px. A
control spans whole cells: a round button 1×1, a tile 2×1 (164 × 74), Now Playing and the
Now Playing 2×2 (164 × 164), a slider 3×1 (254 × 74, room for a button beside it) or
4×1 (344 × 74). Inside, a tile is padded `quick.tile.pad` (17) with `quick.tile.gap` (8) between
its disc (`quick.tile.disc`, 40) and its two lines (`quick.type.title` 14 px and `.caption` 12 px
on a `quick.type.line` 17 px line), so a 1×1 is exactly the disc and its padding. Out of edit
mode the card shows the rows in use; holes inside stay empty.

"Edit Controls" under the grid enters edit mode, as on iOS: the card grows to the whole canvas,
the controls go inert, every free cell shows as a faint disc, and each control gets a remove
badge at its top-left and, when it comes in more than one size, a resize handle at its
bottom-right.

- **Move**: drag a control; the cell its top-left is nearest takes it, and the others make way
  live: one control of the same size under it changes places with it, anything else goes to the
  first free cells. Where nothing fits, the arrangement stays as it last fit. Every preview is
  worked out from the layout as it was when the drag began, and the held control sits exactly
  where the pointer took it (the pointer less where it was grabbed), so the cell under it never
  shifts because of what the preview moved (the same cell always gives the same arrangement, and
  moving back restores the start). A control keeps its cell until it's clearly in the next (half
  a pitch and a tenth more), so a hand resting on a boundary doesn't make the grid flap; an
  accent outline shows the cells it will land in. Escape lets go of it where it started.
- **Resize**: drag the handle; the pull picks the nearest size the control comes in, by width
  and height (a round button becomes a tile, a tile three or four cells wide, Now Playing a
  full row). An outline follows the hand, the control snaps between its sizes (holding the one
  it has until another is clearly nearer), and the others make way the same way.
- **Remove**: the badge hides it. **Add**: the hidden controls list under the grid, each with a
  `+`, puts one back in the first free cells (at its smallest size if only that fits). One
  whose hardware is missing ("Not available") goes on the canvas as a dimmed placeholder that
  shows in edit mode only, waiting for its hardware; outside edit mode its cells count as
  empty. The list scrolls past six rows, and says so when every control is on the canvas.
- **Undo Remove** puts the last removed control back where it was; **Reset Layout** restores
  Control Center's order (as `quick-settings reset-layout` does). Both are under the Add list.
- **Done** (the accent capsule, or Escape) leaves edit mode. A right click on the canvas's
  empty space enters it.

The layout is kept in `$XDG_STATE_HOME/amane-shell/quick-settings`:

```json
{ "version": 2, "controls": [ { "id": "focus", "cols": 2, "rows": 1, "col": 0, "row": 2 }, … ] }
```

The ids are `wifi`, `bluetooth`, `now-playing`, `vpn`, `wired`, `focus`, `dark-style`,
`night-light`, `display`, `sound`, `battery`, `power-mode`, `keyboard`, `airplane`,
`background-apps`, `screenshot`, `lock`, `power-off` and `tailscale`. A control without `col`/`row` is
hidden. A file from an older version is ignored. One that places something off the canvas or
over another control moves it to the first free cells (or hides it); a control the file doesn't
name is added hidden, so a control that arrives with an update or new hardware waits in the Add
list rather than tearing a hole in the canvas. The default packs Control Center's order onto the
canvas for the controls this machine has, with VPN and Wired hidden. `quick-settings reset-layout` over IPC
restores it.

## Motion

The card has one height, the grid's, for the grid and every page. A surface resized every frame
costs a compositor configure round-trip and a second draw each time, which is what made the old
unfolding card stutter. So nothing resizes while anything moves: the card swells in from the
bar's corner (`motion::OPEN`, from `quick.open.scale`), a page slides in from the right while the
grid recedes to the left (`quick.page.recede`) inside the unchanged surface, and the card fades
out at the size it had (`motion::CLOSE`). Every piece of frost fades with what's on it: the card's
as it opens and closes, the grid's as it recedes under a page and the page's as it comes in, a
control's as it's added or removed in edit mode (`glass_region::faded`, alpha on each blur shape).
A compositor that can't fade frost (no niri-blur-shapes, `amane::blur_fades()` false) only shows
or hides it, and frost at full strength under modules still fading in reads as blurred patches
with nothing on them; there the card arrives whole, frost and modules in the same frame, closing
drops the frost in the first frame, and amane leaves out the frost of anything less than half
visible. The surface is resized only at rest: when
the grid gains or loses a row, and when edit mode begins or ends — never while a control is
held. A surface resized just before it hides
still has the compositor's answers to the resize on their way, and amane draws on those after
the hide, which niri rejects as a protocol error; closing never resizes.

Every control's hover layer fades (`motion::HOVER`), and a press shrinks a tile or disc to
`quick.press.scale` and springs it back (`motion::PRESS`): amane reports a press as the start of a
drag and not its release, so the pulse is timed rather than held. In edit mode the controls
spring between cells (`motion::animate`, keyed by control), and the one in hand draws over them
at `quick.drag.scale`, springing back to size when let go. The cursor a drag started with stays
until the drop (amane keeps the dragged target's cursor): a resize handle doesn't follow the
pointer, and a cursor flipping on every motion made niri redraw every output each time.

A drag is measured on one still layer over the canvas (`edit::drag_layer`), not on the control.
amane measures a drag against the hit areas of the last frame drawn, and a control on the move
is drawn somewhere new each frame; a drag measured on it counted the pointer's motion against
where the control *was*, several times a frame with a mouse that reports faster than the screen
refreshes, so the control overshot and snapped back every frame. amane also keeps the target
being dragged by its position in the frame's hit areas, so the layer comes before the controls,
after layers whose shape never changes under a drag (one ghost disc per cell, one drop outline).
The frost behind a moving control is the control as drawn, scaled about its centre, the same
shape its glass edge is lit on.

## Frost

niri frosts a layer surface where the surface asks, or its whole geometry when it doesn't ask.
The Quick Settings surface asks with one rounded shape per piece of glass
(`LayerWindow::blur_shapes`, `glass_region.rs`): the grid's modules, the Edit Controls capsule,
the Add list, and a page's sheet. The shapes go over niri-blur-shapes, a protocol of our niri
fork: each is a rounded rectangle in fractional surface coordinates with its own alpha, which
niri draws through its corner-rounding shader, half a pixel inside the module's own antialiased
edge, so no frost strays past a corner. Each also names the material `glass` and carries the
modules' tint. When niri's config defines a `glass` material (`materials {}`, see niri's
Window-Effects wiki page), niri draws the modules' tint and lit edge itself, from the real
backdrop and on the frost's own edge: the modules then leave out their tint fill and the grid
its rim pass (`glass_region::compositor_glass`), and a page's sheet its `glass::apply`. Without
the material the shell draws both as before. On a compositor without the protocol, amane
sends the shapes as an `ext-background-effect-v1` region instead: strips that step in at the
corners, rounded inwards. Its layer rule therefore carries no corner radius, blur or shadow, only
`xray false` (niri's blur shows the wallpaper alone otherwise); see
[layer-rules.md](layer-rules.md).

A module is the panels' tint, the panels' glass edge and a soft drop shadow (`tile::module`,
`quick.shadow`); vello draws the shadow analytically in the module's own pass, so sixteen of
them cost well under a millisecond. The card itself sits `quick.grid.margin` in from the
surface's edge, room for the shadows. The glass (`glass.wgsl`: the lit rim, the far catch and the
bloom; the panels' inner glow is left out, as across a whole module it read as a band inside the
edge) is a GPU pass whatever it covers, and a pass per module (`glass::apply`, as
the panels wear it) cost 13 ms more a frame on an integrated GPU, so the grid lights every
module in one pass over them all (`glass::rims`, up to thirty pieces a pass, their geometry in
the shader's values; amane's fork gives a shader 64 rows for that): +2 ms a frame, measured. A
page's sheet, one piece, wears `glass::apply` itself.

## IPC

```sh
printf 'quick-settings\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'quick-settings\nopen\nwifi\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'quick-settings\nclose\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

The pages are `grid`, `sound`, `wifi`, `vpn`, `bluetooth`, `power-mode`, `power`, `night-light`,
`do-not-disturb`, `keyboard`, `session`, `background-apps` and `tailscale`. `quick-settings edit` opens the
grid in edit mode, and `quick-settings reset-layout` restores the default layout. A niri keybind:

```kdl
Mod+S { spawn "sh" "-c" "printf 'quick-settings\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock"; }
```

The card takes the keyboard while it's open (`Keyboard::Exclusive`: it's modal, as Control
Center is), so Escape works from the first moment however the card was opened, and the Wi‑Fi
password field types without a click first. A click outside doesn't close it.

## Components

The parts are in `src/components/quick_settings/`:

- **`tile.rs` (`QuickToggle`, `module`)**: GNOME's QuickToggle in Control Center's shapes
  (`Style`): a `Card` (a glass capsule two, three or four cells wide), a `Row` (one line of a module), a
  `Round` button (the disc alone in a glass circle, named by a tooltip), a bare `Disc` inside a
  module. `module` is the glass piece every control sits in; `sheet` the page's, with the full
  glass edge; `capsule_button` the labelled capsules.
  - A round disc holding the glyph. It springs to the accent when the setting is on, like the
    `Switch` knob.
  - A title and an elided subtitle beside the disc.
  - Like the other controls, it shows a click before the service agrees, for up to 1.5 s.
  - `inert` draws it without any handler, for edit mode.
- **`grid.rs`**: the canvas's geometry: a `Span` (cells across and down), a `Place`, a `Rect`,
  the `Canvas` (from the tokens), `Metrics` (cell and gap in pixels, a rect's `Slot`, the
  nearest place to a point) and `Occupancy` (which control each cell holds; the first free
  place for a span, what a rect covers, the rows in use). amane has no grid widget, on purpose.
- **`controls.rs` (`ControlId`)**: every control, the sizes it comes in, and how to build it.
- **`layout.rs` (`Layout`)**: where each control sits and how big it is; the file; `moved` and
  `resized` give the layout a drag would leave (swap or push), `show` the first free cells.
- **`edit.rs`**: edit mode: the drag in hand and the layout it previews from the base, the
  resize ghost, the badge and handle, the ghost discs, the add list's actions.
- **`glass_region.rs`**: the frost behind a piece of glass as a blur shape, and its transforms.
- **`tiles/media.rs`**: the square Now Playing module.
- **`slider_card.rs`**: the Display and Sound cards: a title over each thin slider, a small
  glyph before the track and a large faint one after it, and a right click (`Card::on_secondary`)
  for a card with a page. The track's fill is the panel's `fg` (white in dark mode) as on macOS,
  a documented departure from the one-accent rule (§11.2). (Control Center's microphone slider
  would need a taller module; the grid's cells are one height, so it's on the Sound page.)
- **`detail.rs` (`DetailPage`)**: a page, as tall as the grid.
  - A back chevron (`TopAppBar::back`), the title and subtitle (elided to the room the
    chevron and switch leave), and an optional `Switch`.
  - A hairline, then a body built for the room it has (`List` rows are the page's width,
    `list::with_row_width`), at least `MIN_PAGE_ROWS` tall.
  - Pinned at the bottom: an optional **Show in Taskbar** switch (`DetailPage::taskbar`, for the
    bar's status icon) and an optional link to a settings app.
- **`system_row.rs`, `tiles/`, `pages.rs`**: the controls and pages themselves. `tiles/` has one
  file per setting, holding its toggle and its page.

It reuses the shared components:

- **`Slider`**: the thin slider, with `fill` and `thickness` for the cards and `inert` for
  edit mode; `Slider::large` (a control-tall track with the glyph as its knob, which can mute)
  on the pages.
- **`buttons::optimistic` / `expect`**: the click shown before the service agrees, for the
  mute and play/pause glyphs as `Pending` is for the tiles.
- **`dnd::choices`**: the timed choices, which the Do Not Disturb menu uses too.

Tokens: `quick.grid.{cell,cols,rows,gap,margin}`, `quick.tile.{pad,gap,disc}`,
`quick.card.radius`, `quick.media.{art,art-radius,play}`, `quick.slider.track`, `quick.shadow`,
`quick.type.{title,caption,line}`, `quick.page.recede`, `quick.open.scale`, `quick.press.scale`
and `quick.drag.scale` (see [penpot-todo.md](penpot-todo.md)). Everything else is `sys.*`.

## Backends

What amane has no service for is a shell service in `src/services/`. Each is an amane `Service`:
live state a view reads, quiet when nothing changed, and "not present" (its tile hidden) when its
daemon or device is missing.

| Service | Reads | Changes with |
|---|---|---|
| `session` | what logind allows (`CanSuspend`, `CanReboot`, `CanPowerOff`), asked once | logind `Suspend`/`Reboot`/`PowerOff`, `Session.Lock`; logging out quits niri |
| `night_light` | its own state file | runs `wlsunset` held at one temperature; stopping it gives the screens their colours back |
| `dark_style` | amane's `Appearance` (the Settings portal) | `dconf write /org/gnome/desktop/interface/color-scheme` |
| `rfkill` | `/sys/class/rfkill`, polled each second | `rfkill block all` / `unblock all` |
| `keyboard_backlight` | the first `*::kbd_backlight` LED, polled | logind `SetBrightness("leds", …)` |
| `connections` | `nmcli -t` every 3 s: VPN profiles, the Ethernet device, a Wi‑Fi profile in access-point mode | `nmcli connection up/down`, `nmcli device connect/disconnect` |
| `privacy` | `pw-dump --monitor` (recording streams, running v4l2/libcamera sources); `niri msg --json casts` every 2 s | — |
| `background_apps` | `org.freedesktop.background.Monitor`'s `BackgroundApps`, on each change | the app's `quit` action, else `flatpak kill` |
| `tailscale` | `tailscale status --json` and `tailscale debug prefs`, read again whenever `tailscale debug watch-ipn` prints (a burst settles for 300 ms first; the stream is restarted with a 1–60 s backoff when the daemon goes) | `tailscale up` (never with flags), `down`, `set --exit-node / --accept-routes / --accept-dns / --shields-up`, `login` (left running; its URL is taken from `status --json` and opened with `xdg-open`); copying is `wl-copy` |

**Tailscale needs the operator grant to change anything.** Reads work for anyone, but `tailscaled`
refuses writes from a user who isn't root or its operator. Grant it once with
`sudo tailscale set --operator=$USER`; it's kept in tailscaled's state across reboots. Without it
the tile and page show the true state read-only (judged from `OperatorUser` in the prefs, so
without a probe write), and the page has the command to copy. The shell never escalates.

**Night light owns `wlsunset`.** The compositor gives the colour ramps to one program at a time,
so turning night light on stops any other `wlsunset`. A `spawn-at-startup "wlsunset" …` line in
the niri config would fight the shell's, so remove it. It's manual only: on at the chosen
temperature until turned off. Its state (`on 4000`) is kept in
`$XDG_STATE_HOME/amane-shell/night-light`, so it comes back on after a restart. A last-used VPN
is kept beside it, in `vpn`.

A VPN is a NetworkManager `vpn` or `wireguard` profile, as in GNOME. A tunnel NetworkManager
doesn't manage (tailscale) isn't one.

## Related

- The bar and its panels: [taskbar.md](taskbar.md).
- Its frosted surface's niri layer rule: [layer-rules.md](layer-rules.md).
- The shared components it's built from: [components.md](components.md) and
  [buttons.md](buttons.md).
