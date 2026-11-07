# Services

What the shell reads and changes on the system, and the files it keeps. What amane has no service
for is a shell service in `src/services/`; the rest (battery, audio, networks, Bluetooth, niri's
workspaces and windows, notifications …) are amane's.

## The shell's services

Most are an amane `Service`: live state a view reads, quiet when nothing changed, and "not
present" (its control hidden) when its daemon or device is missing. `timer`, `clipboard_history`
and `file_search` keep their state in the module behind a kit `marker!`, and `dark_style` is only a
write: amane's `Appearance` reads the preference back. The first ten back Quick
Settings ([quick-settings.md](../shell/quick-settings.md)); the rest the bar, the launcher and the
widgets.

| Service | Reads | Changes with |
|---|---|---|
| `session` | what logind allows (`CanSuspend`, `CanReboot`, `CanPowerOff`), asked once | logind `Suspend`/`Reboot`/`PowerOff`, `Session.Lock`; logging out quits niri |
| `night_light` | its own state file | runs `wlsunset` at one temperature all day or on a schedule; stopping it gives the screens their colours back |
| `power_refresh` | `displays.json` | on battery, each display's slowest refresh rate at its resolution, through niri's temporary output mode; the rate it had back on AC ([displays.md](../shell/displays.md#refresh-rate-on-battery)) |
| `dark_style` | amane's `Appearance` (the Settings portal) | `dconf write /org/gnome/desktop/interface/color-scheme` |
| `rfkill` | `/sys/class/rfkill`, polled each second | `rfkill block all` / `unblock all` |
| `keyboard_backlight` | the first `*::kbd_backlight` LED, polled | logind `SetBrightness("leds", …)` |
| `connections` | `nmcli -t` every 3 s: VPN profiles, the Ethernet device, a Wi‑Fi profile in access-point mode | `nmcli connection up/down`, `nmcli device connect/disconnect` |
| `privacy` | `pw-dump --monitor` (recording streams, running v4l2/libcamera sources); `niri msg --json casts` every 2 s | — |
| `background_apps` | `org.freedesktop.background.Monitor`'s `BackgroundApps`, on each change | the app's `quit` action, else `flatpak kill` |
| `keep_awake` | what it holds and until when (kept in memory only); `NameHasOwner` on the screensaver and logind's `BlockInhibited`, asked once, say whether it can; logind's `Session.Lock` for this session | `org.freedesktop.ScreenSaver.Inhibit`/`UnInhibit` for the screen; logind `Inhibit("handle-lid-switch", …, "block")` for the lid, held as the fd it hands back; `niri msg action power-off-monitors` |
| `tailscale` | `tailscale status --json` and `tailscale debug prefs`, read again whenever `tailscale debug watch-ipn` prints (a burst settles for 300 ms first; the stream is restarted with a 1–60 s backoff when the daemon goes) | `tailscale up` (never with flags), `down`, `set --exit-node / --accept-routes / --accept-dns / --shields-up`, `login` (left running; its URL is taken from `status --json` and opened with `xdg-open`); copying is `wl-copy` |
| `clock` | the wall-clock time and date, read again each second; it signals only when the shown text changes | — |
| `timer` | the launcher's one countdown; a thread sleeps it out, then a notification says so | Start Timer / Cancel Timer |
| `clipboard_history` | `wl-paste --watch` while it's on; the copies under `clipboard/` | `wl-copy`, then `wtype` Ctrl+V (Ctrl+Shift+V in a terminal) |
| `file_search` | LocalSearch's index through `tinysparql`, else `fd` over the home folder, off the UI thread | — |
| `status_apps` | `org.amane.StatusBar1` on the session bus: other programs' items and Activities, each kept with its owner's unique name, dropped when that name leaves ([status-apps.md](status-apps.md)) | the programs' own calls; clicks, picks and drops go back as unicast signals |
| `desktop/activities` | the Activities from the bus, by handle: owner, content, when last updated, when it goes (a kit `marker!`) | `status-bar activities end` / `press`, a button in the expanded view, its program leaving, 8 hours |
| `system_stats` | `/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `/proc/net/route`, `/sys/class/hwmon`, `/proc/mounts` and `statvfs`, on one 2 s tick, only while a system widget shows | — |

**Tailscale needs the operator grant to change anything.** Reads work for anyone, but `tailscaled`
refuses writes from a user who isn't root or its operator. Grant it once with
`sudo tailscale set --operator=$USER`; it's kept in tailscaled's state across reboots. Without it
the tile and page show the true state read-only (judged from `OperatorUser` in the prefs, so
without a probe write), and the page has the command to copy. The shell never escalates.

**Night light owns `wlsunset`.** The compositor gives the colour ramps to one program at a time,
so turning night light on stops any other `wlsunset`. A `spawn-at-startup "wlsunset" …` line in
the niri config would fight the shell's, so remove it. By default it's on at the chosen
temperature until turned off; Shell Settings' Displays page can give it a schedule (sunset to
sunrise, or set hours), which `wlsunset` follows by itself. Its state (`on 4000 manual`) is kept in
`$XDG_STATE_HOME/amane-shell/night-light`, so it comes back on after a restart. A last-used VPN
is kept beside it, in `vpn`.

**Keep Awake never writes a power setting** (amane's idle support:
[`system/idle.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/idle.md)). Each hold is an inhibitor tied to the shell: the
screen's is on amane's one session-bus connection (niri serves `org.freedesktop.ScreenSaver`, and
hypridle honours its inhibits, so nothing dims, locks or turns the monitors off), and the lid's
is the file descriptor logind hands back, which needs no root (`HandleLidSwitch` in `/etc` is
left alone). A shell that dies lets go of both, and the machine behaves as normal again by
itself. That's also why the state isn't kept across a restart. A time limit is a wall-clock
instant, so one that spans a suspend has run out on waking. Nothing blocks a suspend that's
asked for.

**Locking lets the screen blank.** On logind's Lock signal for this session the screen hold is
let go, so the lock screen blanks on hypridle's timers instead of staying lit (burn-in on this
OLED panel); the lid stays held. swaylock says nothing when it's unlocked, so the hold comes back
once no `swaylock` process is left (it's given 5 s to start). Locks have to go through logind for
the shell to see them, so niri's Super+Alt+L runs `loginctl lock-session`, and hypridle's
`lock_cmd` starts swaylock.

A VPN is a NetworkManager `vpn` or `wireguard` profile, as in GNOME. A tunnel NetworkManager
doesn't manage (tailscale) isn't one.

## amane's services

The shell reads these from the amane fork. Each is an amane `Service`, so a view that reads one
draws again when it changes. amane documents them: the niri-backed ones in
[`compositor/services.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/compositor/services.md), the system ones under `system/` (below), the
backdrop readings in [`graphics/blur.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/graphics/blur.md).

| Service | What reads it |
|---|---|
| `Workspaces`, `Windows` | the bar's workspace shape, the dock, the widgets' monitor; window actions (focus, move, close) |
| `Overview` | the dock rising with niri's overview, from its progress |
| `Desktop`, `WorkspaceSwitch` | the widgets: frozen while the desktop is hidden, fading with a workspace switch; the bar's workspace shape, morphing with it |
| `Notifications` | toasts and the notification center; amane owns `org.freedesktop.Notifications` ([`system/notifications.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/notifications.md)) |
| `Audio` | the bar's volume, the Sound card and page ([`system/audio.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/audio.md)) |
| `Network`, `Bluetooth` | the bar's icons, the Wi‑Fi and Bluetooth tiles and pages; `Bluetooth` also the pairing alert and the low-battery notices, and the shell serves its pairing agent |
| `KdeConnect`, `PhoneMessages` | the Phone control and page, the bar's phone icon, the pairing alert and the phone's low-battery notices; `PhoneMessages` only in `amane-messages` ([`system/kdeconnect.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/kdeconnect.md), [phone.md](../shell/phone.md)) |
| `Battery`, `PowerProfile` | the bar's battery, the Battery page, power mode, the battery widgets ([`system/battery.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/battery.md), [`system/power-profile.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/power-profile.md)) |
| `Brightness`, `Media` | the Display slider, Now Playing |
| `Appearance` | the system's light/dark, accent, contrast and reduced motion (`amane_kit::theme`), Dark Style ([`system/appearance.md`](https://github.com/098765434/amane/blob/ocbk/docs/fork/system/appearance.md)) |
| `Backdrop` | adaptive glass: how bright the backdrop behind a blur shape is ([materials.md](../design-system/materials.md)) |

## Settings and state

**Settings** live in `$XDG_CONFIG_HOME/amane-shell/` (else `~/.config/amane-shell/`), as JSON
through amane's `Config` (`amane_kit::settings`). The running shell follows each file at once,
whether [Shell Settings](../shell/settings-app.md), the launcher or a text editor changed it:

| File | Holds |
|---|---|
| `appearance.json` | mode, accent, density, text scale, glass, reduced motion, adaptive glass, vibrancy |
| `desktop.json` | whether the widgets show, and how they dim |
| `launcher.json` | the apps' layout, the search sources left out, quick keys, the clipboard history's settings |

**State** lives in `$XDG_STATE_HOME/amane-shell/` (else `~/.local/state/amane-shell/`). A missing
file means the default:

| File | Holds | Written by |
|---|---|---|
| `dnd` | Do Not Disturb: `forever` or `until <unix seconds>` | `notifications/dnd.rs` |
| `status-bar` | the hidden status icons, one per line (the old `taskbar` file is read until it exists) | `status_bar/indicators.rs` |
| `dock-pins` | the dock's pins, one a line | `dock/pins.rs` |
| `quick-settings` | Quick Settings' layout (JSON) | `quick_settings/layout.rs` |
| `widgets` | each monitor's widgets and their settings (JSON) | `widgets/store.rs` |
| `night-light` | whether night light is on, how warm and when (`on 4000 manual`, `on 3500 sun 59.92 10.75`, `on 4000 hours 21:00 07:00`; a line without the schedule is all day) | `services/display/night_light.rs` |
| `vpn` | the last VPN used | `services/network/connections.rs` |
| `launcher-usage.json` | how much each result was used, and the last searches | `launcher/usage.rs` |
| `clipboard/` | the clipboard history: `index.json` and a file a copy | `services/launcher/clipboard_history.rs` |

Keep Awake keeps nothing: its holds are inhibitors tied to the running shell.

**Caches** in `$XDG_CACHE_HOME/amane-shell/`, so Shell Settings can list what isn't running:
`tray-seen.json`, `status-apps-seen.json` (programs' items: id → name) and
`activities-seen.json` (the names activities gave).

### amane-drop's files

[Drop](../shell/drop.md) keeps its own, followed live by Drop and Shell Settings alike:

| File | Holds |
|---|---|
| `~/.config/amane-drop/grids.json` | the grids, their tiles, and tiles a row (`DropGrids`) |
| `~/.config/amane-drop/sharing.json` | the SFTP and S3 accounts, Imgur's client ID, and what an upload's link does (`DropSharing`) |
| `~/.config/amane-drop/secrets.json` | passwords and keys, mode 0600, only when no keyring answered `secret-tool` (`amane_kit::secrets`); otherwise they're in the keyring under `application amane-drop` |
| `$XDG_STATE_HOME/amane-drop/dropbar.json` | each grid's Drop Bar: stacks of paths, by the grid's name |
| `$XDG_STATE_HOME/amane-drop/shared.json` | Recently Shared: the last 10 links, with what they were and when |
