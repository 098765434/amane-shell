# Quick Settings

GNOME's quick settings menu, drawn in the macOS Control Center look
(`src/components/quick_settings/`). It's one frosted card above the bar's right end, with the
session's actions, the sliders and a grid of tiles. Each tile can open a page of its own.

It's opened from the bar's Control Center button (the slider glyph, between the battery and the
notification bell), and closed by the same button, by Escape, or by opening any other panel. It
shares the panels' one-open-at-a-time state (`Panel::QuickSettings`). The bar's own Sound,
Bluetooth, Wi‑Fi and power panels are still there: a left click on their icons opens them as
before. A right click on the volume, Bluetooth or network icon opens Quick Settings straight on
that page.

## What's in it

In GNOME's order. Every row shows only when the machine has what it controls, as in GNOME:

| Row | Shows | Click | Page |
|---|---|---|---|
| System row | the battery, screenshot, lock, power | battery → its page; screenshot closes the card, then opens niri's screenshot UI; lock locks through logind | Battery (charge, time left, power mode, peripherals); Power Off |
| Now Playing | the active player's art, title and artist | previous, play/pause, next | — |
| Output slider | the default output's glyph and level | the glyph mutes; drag or scroll sets the level | Sound (output and input devices), when there's more than one output |
| Input slider | only while something records | as above, for the microphone | Sound |
| Brightness | only with a backlight | drag or scroll | — |
| Wired | only with a cable in; the connection | connects or disconnects the device | — |
| Wi‑Fi | the network joined, "Connecting…", "Not Connected" or "Off" | switches the radio | Wi‑Fi: the networks (rescanned every 15 s while shown), joining with a password |
| Wi‑Fi hotspot | "<ssid> Hotspot", while the machine is one | stops it | — |
| VPN | only with a VPN profile; the active one | turns on the last used, or off | VPN: a switch per profile |
| Bluetooth | only with an adapter; the device connected, or how many | switches the radio | Bluetooth: the devices, searching while shown |
| Power Mode | only with power-profiles-daemon; the mode | Balanced, or back to the mode last picked | Power Mode: the modes |
| Night Light | only with `wlsunset`; the temperature | on or off | Night Light: how warm |
| Dark Style | the system preference | switches it | — |
| Do Not Disturb | on, until when, or off | on or off | For 1 Hour, Until This Evening, Until Tomorrow |
| Keyboard | only with a keyboard light; its level | off, or back to its level | Off/Low/High (a slider for keyboards with many steps) |
| Airplane Mode | only with a radio | blocks or unblocks every radio | — |
| Background Apps | only while an app runs without a window; how many | — | the apps, each with a close button |

A tile works like a macOS Control Center module. A click on the round glyph switches the setting,
and a click anywhere else on the tile opens its page. A tile with no page switches from anywhere.
A right click opens the page too. The Power Off page's Restart…, Power Off… and Log Out… ask for a
second click before they act, which stands in for GNOME's confirmation dialog. Switch User is left
out because greetd has no user switching.

A page's last row opens the app with the rest of its settings when that app is installed:
`nm-connection-editor` for Wi‑Fi and VPN, `blueman-manager` for Bluetooth and `pavucontrol` for
Sound.

The bar also shows GNOME's status icons, each only while it applies:

- **Privacy icons**, first: the screen being shared, the camera in use, and the microphone in use.
  The microphone's tooltip names the app.
- **Glyphs for settings that are on**: night light, a VPN, a power mode other than Balanced, and
  airplane mode.

Clicking any of them opens Quick Settings.

## Motion

The card unfolds upwards from the bar and fades in on a `motion::sheet` spring. A page slides in
from the right while the grid recedes to the left (`quick.page.recede`), and the card springs to
the page's height. Escape goes back to the grid, then closes. Closing, the card fades at the size
it had. A surface resized just before it hides still has the compositor's answers to the resize
on their way, and amane draws on those after the hide, which niri rejects as a protocol error.

## IPC

```sh
printf 'quick-settings\ntoggle\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'quick-settings\nopen\nwifi\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
printf 'quick-settings\nclose\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock
```

The pages are `grid`, `sound`, `wifi`, `vpn`, `bluetooth`, `power-mode`, `power`, `night-light`,
`do-not-disturb`, `keyboard`, `session` and `background-apps`. A niri keybind:

```kdl
Mod+S { spawn "sh" "-c" "printf 'quick-settings\\ntoggle\\n' | nc -U -N $XDG_RUNTIME_DIR/amane.sock"; }
```

The card takes keys once it's clicked into (`Keyboard::OnDemand`, like the panels), so Escape
works after a click. A click outside doesn't close it, the same as the panels.

## Components

The parts are in `src/components/quick_settings/`:

- **`tile.rs` (`QuickToggle`)**: GNOME's QuickToggle as a Control Center module.
  - A rounded `surface.raised` tile, half or full width (`Span`).
  - A round disc holding the glyph. It springs to the accent when the setting is on, like the
    `Switch` knob.
  - A title and an elided subtitle beside the disc.
  - Like the other controls, it shows a click before the service agrees, for up to 1.5 s.
- **`grid.rs`**: GNOME's two-column layout, packed from rows. amane has no grid widget, on
  purpose. Half tiles go two to a row, and a full-width cell starts its own. Every cell has a
  fixed height, so the grid knows its height before it's drawn, and the surface is sized to it.
- **`detail.rs` (`DetailPage`)**: a page.
  - A back chevron (`TopAppBar::back`), the glyph, the title and subtitle, and an optional
    `Switch`.
  - A body that says how tall it is.
  - An optional link to a settings app.
- **`system_row.rs`, `tiles/`, `pages.rs`**: the rows and pages themselves. `tiles/` has one file
  per setting, holding its grid cell and its page.

It reuses the shared components:

- **`Slider::large`**: the Control Center slider, a control-tall track with the glyph as its knob.
  The glyph can be a button (`on_icon_press`, used for mute).
- **The panels' bodies**: `wifi::networks`, `bluetooth::devices`, `audio::device_section`,
  `power::profile_section` and `power::peripheral_section`.
- **`dnd::choices`**: the timed choices, which the Do Not Disturb menu uses too.

Tokens: `quick.tile.height`, `quick.tile.radius`, `quick.tile.disc`, `quick.media.art` and
`quick.page.recede` (see [penpot-todo.md](penpot-todo.md)). Everything else is `sys.*`.

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
