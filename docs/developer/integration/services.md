# Services

What the shell reads and changes on the system, and the files it keeps. What amane has no service
for is a shell service in `apps/shell/src/services/`; the rest (battery, audio, networks, Bluetooth, niri's
workspaces and windows, notifications …) are amane's.

## The shell's services

Most are an amane `Service`: live state a view reads, quiet when nothing changed, and "not
present" (its control hidden) when its daemon or device is missing. `timer`, `clipboard_history`
and `file_search` keep their state in the module behind a kit `marker!`, and `dark_style` is only a
write: amane's `Appearance` reads the preference back. The first ten back Quick
Settings ([quick-settings.md](../../user/features/quick-settings.md)); the rest the bar, the launcher and the
widgets.

| Service | Reads | Changes with |
|---|---|---|
| `session` | what logind allows (`CanSuspend`, `CanReboot`, `CanPowerOff`), asked once | logind `Suspend`/`Reboot`/`PowerOff`, `Session.Lock`; logging out quits niri (`amane::niri::quit`) |
| `night_light` | its own state file; a thread waits on the running `wlsunset`, so one that exits turns it off | runs `wlsunset` at one temperature all day or on a schedule; stopping it gives the screens their colors back |
| `power_refresh` | `displays.json`, amane's `Battery` and `Outputs`, checked when one changes | on battery, each display's choice (a rate, its slowest, or variable refresh up to a rate) through niri's temporary output mode and VRR; back on AC, niri reads its config again (`load-config-file`) for `displays.kdl`'s settings ([displays.md](../../user/features/displays.md#refresh-rate-on-battery)) |
| `wallpaper_dim` | `appearance.json` and the portal's light/dark, checked when one changes | under Black, niri dims or hides the wallpaper over a black backdrop (fork action `SetWallpaperDim`, feature `wallpaper-dim`); otherwise undimmed |
| `dark_style` | amane's `Appearance` (the Settings portal) | `dconf write /org/gnome/desktop/interface/color-scheme` |
| `rfkill` | amane's `Rfkill` (`/dev/rfkill` events) | `Rfkill::set_blocked(None, …)`, as `rfkill block all` |
| `keyboard_backlight` | the first `*::kbd_backlight` LED, polled once a second (LEDs announce no change) | logind `SetBrightness("leds", …)` |
| `connections` | amane's `Network`, which follows NetworkManager's signals: VPN profiles, the Ethernet device, a Wi‑Fi profile in access-point mode | `Network::activate` / `deactivate` / `set_device` |
| `privacy` | `pw-dump --monitor` (recording streams, running v4l2/libcamera sources); amane's `Casts` (niri's cast events) | stopping a cast (`amane::niri::stop_cast`) |
| `background_apps` | `org.freedesktop.background.Monitor`'s `BackgroundApps`, on each change | the app's `quit` action over the session bus, else `flatpak kill` |
| `keep_awake` | what it holds and until when (kept in memory only); `NameHasOwner` on the screensaver and logind's `BlockInhibited`, asked once, say whether it can; logind's `Session.Lock` for this session | `org.freedesktop.ScreenSaver.Inhibit`/`UnInhibit` for the screen; logind `Inhibit("handle-lid-switch", …, "block")` for the lid, held as the fd it hands back; `amane::niri::power_off_monitors`. While locked, the locker's exit is waited on with a pidfd |
| `tailscale` | `tailscale status --json` and `tailscale debug prefs`, read again whenever `tailscale debug watch-ipn` prints (a burst settles for 300 ms first; the stream is restarted with a 1–60 s backoff when the daemon goes) | `tailscale up` (never with flags), `down`, `set --exit-node / --accept-routes / --accept-dns / --shields-up`, `login` (left running; its URL is taken from `status --json` and opened with `xdg-open`); copying is `wl-copy` |
| `clock` | the wall-clock time and date, read again on the minute (each second while seconds or flashing colons show), after a change to the bar's settings, and on waking from sleep | — |
| `timer` | the launcher's one countdown; a timer on amane's event loop runs it out, then a notification says so | Start Timer / Cancel Timer |
| `clipboard_history` | amane's `ClipboardWatcher` while it's on (data-control, no program started); the copies under `clipboard/` | `wl-copy`, then `wtype` Ctrl+V (Ctrl+Shift+V in a terminal) |
| `file_search` | LocalSearch's index through `tinysparql`, else `fd` over the home folder, off the UI thread | — |
| `status_apps` | `org.emaki.StatusBar1` on the session bus: other programs' items and Activities, each kept with its owner's unique name, dropped when that name leaves ([status-apps.md](status-apps.md)) | the programs' own calls; clicks, picks and drops go back as unicast signals |
| `desktop/activities` | the Activities from the bus, by handle: owner, content, when last updated, when it goes (a kit `marker!`) | `status-bar activities end` / `press`, a button in the expanded view, its program leaving, 8 hours |
| `system_stats` | `/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `/proc/net/route`, `/sys/class/hwmon`, `/proc/mounts` and `statvfs`, on one 2 s tick, only while a system widget shows (asleep otherwise) | — |

**Tailscale needs the operator grant to change anything.** Reads work for anyone, but `tailscaled`
refuses writes from a user who isn't root or its operator. Grant it once with
`sudo tailscale set --operator=$USER`; it's kept in tailscaled's state across reboots. Without it
the tile and page show the true state read-only (judged from `OperatorUser` in the prefs, so
without a probe write), and the page has the command to copy. The shell never escalates.

**Night light owns `wlsunset`.** The compositor gives the color ramps to one program at a time,
so turning night light on stops any other `wlsunset`. A `spawn-at-startup "wlsunset" …` line in
the niri config would fight the shell's, so remove it. By default it's on at the chosen
temperature until turned off; Emaki Settings' Displays page can give it a schedule (sunset to
sunrise, or set hours), which `wlsunset` follows by itself. Its state is kept in the state file
`night-light.json`, so it comes back on after a restart; the IPC `state` verb still answers the old
one-line form (`on 4000 manual`). A last-used VPN is kept beside it, in `vpn.json`.

**Keep Awake never writes a power setting** (amane's idle support:
[`system/idle.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/idle.md)). Each hold is an inhibitor tied to the shell: the
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
[`compositor/services.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/compositor/services.md), the system ones under `system/` (below), the
backdrop readings in [`graphics/blur.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/graphics/blur.md).

| Service | What reads it |
|---|---|
| `Workspaces`, `Windows` | the bar's workspace shape, the dock, the widgets' monitor; window actions (focus, move, close) |
| `Overview` | the dock rising with niri's overview, from its progress |
| `Desktop`, `WorkspaceSwitch` | the widgets: frozen while the desktop is hidden, fading with a workspace switch; the bar's workspace shape, morphing with it |
| `Notifications` | toasts and the notification center; amane owns `org.freedesktop.Notifications` ([`system/notifications.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/notifications.md)) |
| `Audio`, `AudioLevel` | the bar's volume, the Sound card and page with each app's level, Emaki Settings' Sound page and its input meter ([`system/audio.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/audio.md)) |
| `Network`, `Bluetooth` | the bar's icons, the Wi‑Fi and Bluetooth tiles and pages; `Network` also Emaki Settings' Network page (profiles, `details`, `forget`, `connect_hidden`); `Bluetooth` also the pairing alert and the low-battery notices, and the shell serves its pairing agent |
| `KdeConnect`, `PhoneMessages` | the Phone control and page, the bar's phone icon, the pairing alert and the phone's low-battery notices; `PhoneMessages` only in `emaki-messages` ([emaki-phone](../../../crates/phone/README.md), [phone.md](../../user/features/phone.md)) |
| `Battery`, `PowerProfile` | the bar's battery, the Battery page, power mode, the battery widgets ([`system/battery.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/battery.md), [`system/power-profile.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/power-profile.md)) |
| `Brightness`, `Media` | the Display slider, Now Playing |
| `Appearance` | the system's light/dark, accent, contrast and reduced motion (`emaki_kit::theme`), Dark Style ([`system/appearance.md`](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/system/appearance.md)) |
| `Backdrop` | adaptive glass: how bright the backdrop behind a blur shape is ([materials.md](../design-system/materials.md)) |

## Settings and state

**Settings** live in `$XDG_CONFIG_HOME/emaki/` (else `~/.config/emaki/`), as JSON
through amane's `Config` (`emaki_kit::settings`). The running shell follows each file at once,
whether [Emaki Settings](../../user/features/settings-app.md), the launcher or a text editor changed it:

| File | Holds |
|---|---|
| `appearance.json` | mode, accent, density, text scale, glass, reduced motion, adaptive glass, vibrancy |
| `desktop.json` | whether the widgets show, and how they dim |
| `launcher.json` | the apps' layout, the search sources left out, quick keys, the clipboard history's settings |
| `notifications.json` | each app's notification rule (banners and list, list only, off), by its desktop entry else its name |

**State** lives in `$XDG_STATE_HOME/emaki/` (else `~/.local/state/emaki/`). A missing
file means the default. Each state file is JSON with its format's version beside the data,
`{"version": 1, "data": …}`, written whole through a part file and a rename
(`services/state.rs`). A file from before this format, under the old name in the last column, is
read when the new one isn't there, and left as it was: deleting the new file reads the old one
again. The old readers go after 2027-01.

| File | Holds | Written by | Read before it |
|---|---|---|---|
| `dnd.json` | Do Not Disturb: `"off"`, `"forever"` or `{"until": <unix seconds>}` | `services/do_not_disturb.rs` | `dnd` |
| `dock-pins.json` | the dock's pins | `dock/pins.rs` | `dock-pins` |
| `quick-settings.json` | Quick Settings' layout | `quick_settings/layout.rs` | `quick-settings` (version 2) |
| `widgets.json` | each monitor's widgets and their settings | `widgets/store.rs` | `widgets` (version 1) |
| `night-light.json` | whether night light is on, how warm and when | `services/display/night_light.rs` | `night-light` (`on 4000 manual`) |
| `vpn.json` | the last VPN used | `services/network/connections.rs` | `vpn` |
| `hdr-white.json` | each panel's SDR white in HDR, in nits, set in niri again after a config read | `services/display/hdr_white.rs` | nothing: it was the `sdr-brightness` line of `displays.kdl` |
| `launcher-usage.json` | how much each result was used, and the last searches | `launcher/usage.rs` | the same file without a version |
| `notifications.json` | the notification center's history: up to 100, none older than 14 days, without actions | `services/notification_history.rs` | nothing: history lasted the session |
| `clipboard/index.json` | the clipboard history; each copy is a file of its own beside it | `services/launcher/clipboard_history.rs` | the same file without a version |

Keep Awake keeps nothing: its holds are inhibitors tied to the running shell.

**Caches** in `$XDG_CACHE_HOME/emaki/`, so Emaki Settings can list what isn't running:
`tray-seen.json`, `status-apps-seen.json` (programs' items: id → name),
`activities-seen.json` (the names activities gave) and `notifiers-seen.json` (apps that sent a
notification: key → name).

### emaki-drop's files

[Drop](../../user/features/drop.md) keeps its own, followed live by Drop and Emaki Settings alike:

| File | Holds |
|---|---|
| `~/.config/emaki-drop/grids.json` | the grids, their tiles, and tiles a row (`DropGrids`) |
| `~/.config/emaki-drop/sharing.json` | the SFTP and S3 accounts, Imgur's client ID, and what an upload's link does (`DropSharing`) |
| `~/.config/emaki-drop/secrets.json` | passwords and keys, mode 0600, only when no keyring answered `secret-tool` (`emaki_kit::secrets`); otherwise they're in the keyring under `application emaki-drop` |
| `$XDG_STATE_HOME/emaki-drop/dropbar.json` | each grid's Drop Bar: stacks of paths, by the grid's name |
| `$XDG_STATE_HOME/emaki-drop/shared.json` | Recently Shared: the last 10 links, with what they were and when |

## While idle

Nothing the shell runs polls on a clock of its own while the desktop is left alone, apart from the
keyboard light (once a second: an LED announces no change). Toasts, Do Not Disturb, the clock,
weather, Keep Awake and the vault wait for their next deadline or a change (`services::Wake`,
amane's `Timer`); the radios, the network, casts, lock keys, outputs, the clipboard and
brightness are pushed by amane. The vault is looked at when a fill or the launcher's passwords
want it, and when its lock timeout could have run out. `scripts/idle-report` measures it; see
[idle-baseline.md](../idle-baseline.md).
