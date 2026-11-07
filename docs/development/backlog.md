# Backlog: features the shell doesn't have yet

What the shell lacks, and what would round out what's there, from a survey on 2026-10-28. Each
item says **why**, **what exists to build on**, and a rough **size** (S / M / L). New UI stays on
amane's quad pipeline, with vello only where it can't be avoided. Items that have shipped are
marked **Done**, with where they're documented; mark one when it lands rather than deleting it.

Ordered by how often the gap is felt in daily use, not by effort.

---

## Tier 1: felt every day

### 1. On-screen display (volume, brightness, mic, keyboard backlight) · M

**Done**: [docs/shell/osd.md](../shell/osd.md).
**Gap.** The media and brightness keys are bound in niri to `wpctl` / `brightnessctl` / `playerctl`
(`nixos-config/niri/config.kdl` l. 571–582), so pressing one gives no feedback at all. The token
vocabulary already reserves an `osd` component group (`kit/src/tokens/mod.rs:50`) with no tokens.
**Build.** A small glass capsule (Material::Control) near the bottom edge, above the bar/dock: glyph
+ level bar, fades after ~1.5 s. Drive it from amane's `Audio` and `Brightness` services changing
(so it shows whoever changed the level), or add IPC (`osd volume up|down|mute`, `osd brightness …`)
and rebind the keys to the shell so it both acts and shows. Also: Caps Lock, keyboard backlight,
mic mute, airplane mode, power mode switched.
**Builds on.** `panel::overlay`, `Material::Control`, `slider.rs` drawing, `motion::fade`.

### 2. Window switcher (Alt+Tab / Super+Tab) · M–L
**Gap.** Nothing in the shell switches windows by keyboard; niri's own focus keys are spatial.
**Build.** An overlay with the open windows' icons (and titles) in MRU order, held with the modifier,
released to focus. MRU order needs tracking focus changes from `Windows` (niri's IPC gives focus
events). Variant: per-app cycling (Super+\`). The dock's `model.rs` already turns niri windows into
icons.
**Caveat.** Releasing the modifier must reach the shell: needs a keyboard-exclusive layer surface
while open, like the launcher.

### 3. Notification persistence and per-app settings · M
**Gap.** "History lasts the session" and "No sounds, inline reply, per-app settings, or Focus
schedules" (`docs/shell/notifications.md`, *Not yet*).
**Build, in order of value.**
- Persist the center across restarts (`$XDG_STATE_HOME/amane-shell/notifications.json`, capped,
  images by path only).
- Per-app: allow / banners off / center only / never, keyed by `desktop-entry`; a Shell Settings
  **Notifications** page listing apps seen so far.
- Notification sounds (`sound-name` / `sound-file` hints, a default sound, silenced in DND).
- Inline reply (`inline-reply` capability, as KDE/GNOME advertise it), using `text_edit.rs`.
- Focus schedules: DND on a timetable (e.g. 22:00–07:00), and "while screen sharing" (the
  `privacy` service already knows when a cast is live).

### 4. A calendar in the notification center · S–M
**Gap.** The clock opens the notification center, but there's no month view or upcoming events, which
GNOME puts there.
**Build.** A month grid card at the top of the center (today highlighted, week numbers optional);
later, events from Evolution Data Server / a local `.ics` / khal. The bar clock's tooltip already
formats the date (`Clock::date`).

### 5. Lock screen of the shell's own · L
**Gap.** Locking runs plain swaylock (`services/session.rs`, `keep_awake.rs` watch for it), so the
most-seen screen after the desktop doesn't match the rest of the look, and the shell has to guess
when it's unlocked ("swaylock says nothing when it's unlocked").
**Build.** `ext-session-lock-v1` client in amane (or the shell): blurred wallpaper, the stroke clock,
the user's name, a password field (PAM via the same machinery as the polkit agent), Now Playing and
notification count. The shell would know exactly when it locks and unlocks, which simplifies Keep
Awake's hold juggling.
**Caveat.** A lock screen must never crash open; it needs its own process or a very careful
failure mode (session-lock's "compositor keeps it locked if the client dies" helps).

---

## Tier 2: fills obvious holes in Shell Settings / Quick Settings

### 6. Displays page · M

**Done**: [docs/shell/displays.md](../shell/displays.md).
**Gap.** No way to set resolution, refresh rate, scale, arrangement or primary output; it's all hand
edits in `niri/config.kdl`. The widgets, dock and bar are already per-monitor.
**Build.** Read outputs with `niri msg --json outputs`; apply live through niri's output IPC
(`niri msg output … mode/scale/position/transform/vrr`), and offer "Keep these settings?" with a
15 s revert (the shell's alert). Persist by writing the config's `output {}` blocks (or a managed
include). A drag-to-arrange canvas can reuse `grid/drag.rs` ideas.

### 7. Sound page in Shell Settings + per-app volume in Quick Settings · M
**Gap.** Sound defers to `pavucontrol`. No per-app levels, no balance, no alert sound.
**Build.** In Quick Settings' Sound page, a row per playing stream (app icon + slider + mute); in
Shell Settings, output/input devices, balance, input level meter, alert sound. Needs amane's
`Audio` to expose sink-inputs (pulse/pipewire stream list), which it doesn't yet.

### 8. Network page in Shell Settings · M
**Gap.** Wi‑Fi and VPN details defer to `nm-connection-editor`. Missing: forget network, saved
networks list, hidden network, IP/DNS details, VPN import (`.ovpn`, WireGuard `.conf`), hotspot
setup.
**Builds on.** `services/connections.rs` (nmcli), amane's `Network`.

### 9. Input page: keyboard, mouse, touchpad, shortcuts · M
**Gap.** Keyboard layout is a NixOS setting (`system/xkb.rs`), but there's no UI for repeat
rate/delay, touchpad tap/natural scroll/speed, mouse acceleration, or niri's keybinds.
**Build.** These are all niri `input {}` options. Write them to the niri config (it reloads live),
or a managed include like the layer rules use (`gen-layer-rules.sh`). A read-only **Keyboard
Shortcuts** list parsed from `binds {}` is a cheap first step; editing is a bigger one.
Also: a layout indicator/switcher in the bar when more than one layout is configured (niri reports
`KeyboardLayoutsChanged`).

### 10. Wallpaper picker · S–M
**Gap.** The glass and adaptive tint depend heavily on the wallpaper, yet choosing one is outside the
shell. Light/dark-paired wallpapers would follow the theme the shell already tracks.
**Build.** A Shell Settings **Wallpaper** page (folder of images, per-monitor, fit mode, light/dark
pair, optional slideshow). Set through whichever wallpaper daemon is in use (swaybg/swww), or a
background layer of the shell's own, if memory allows (see `idle-ram-plan.md`: an image layer per
monitor costs VRAM).

### 11. Night Light schedule · S

**Done**: [docs/shell/displays.md](../shell/displays.md#night-light).
**Gap.** "It's manual only: on at the chosen temperature until turned off"
(`docs/integration/services.md`).
**Build.** Sunset-to-sunrise (from a set location, or geoclue) and a custom schedule, with a gradual
ramp. `wlsunset` already supports `-l/-L` and `-S/-s`; the service just needs to pass them.

### 12. Default apps and file associations · S

**Done**: Shell Settings' Default Apps page.
The dock already reads `MimeType` for drops. A page to pick the browser, mail, terminal, image
viewer, etc. (`xdg-mime default`), and the launcher's web search engine, which is hard-coded to
Google (`launcher/model.rs SEARCH_URL`, *Known limits*).

---

## Tier 3: polish and power-user features

### 13. Launcher additions · S each
- **Window search:** open windows as results ("Firefox — GitHub") that focus them; the Applications
  rows could show "Running" and offer "Switch to" first.
- **Emoji picker** mode (a `Ctrl+5` mode, or `:smile` prefix).
- **Calculator history** and currency conversion (rates cached daily).
- **Selectable search engine** and custom search shortcuts (`gh foo`, `nix foo` → search.nixos.org,
  which also fits the NixOS Apps page).
- **Settings search:** Shell Settings pages and Quick Settings pages as results ("Night Light",
  "Displays"), opening `amane-settings --page …`. Fixes the *Known limit* that controls which only
  open a page aren't actions.
- **Clipboard:** pin items, and images beyond PNG/JPEG (once amane can show them).
- **Run a command** (`>` prefix, in a terminal or not).

### 14. Screenshot and screen recording UI · M
Screenshot is niri's UI. Missing: a recording toggle in Quick Settings and the bar (a red stop
chip like the timer's, which already has the pattern), via `wf-recorder` or the portal; a
post-capture thumbnail toast with Copy / Open / Annotate (the toast stack is already there).

### 15. Hot corners and gestures · S–M
A corner trigger (overview, launcher, notification center, show desktop) needs only a thin
corner layer surface, like the dock's 2 px edge strip (`EdgeReveal`). The amane fork already has
`Gestures`; a configurable three-/four-finger mapping (open launcher, notification center) would
fit in the Input page.

### 16. Dock niceties · S each
From `docs/shell/dock.md` *Known limits* and what a dock is expected to do:
- Closing window shrinks away instead of vanishing.
- Notification badges on icons (count from the center's per-app groups, or the Unity
  `LauncherEntry` D-Bus API that many apps already emit), and progress bars from the same API.
- Window previews on hover (needs a niri screencopy of a window; memory cost to weigh).
- "Show Recent Files" / app jump lists in the right-click menu.
- Per-monitor popup placement bug (menus from a second monitor's dock open on the first) needs a
  per-output namespace or an amane fix.

### 17. Bar additions · S each
- Keyboard layout indicator (see 9).
- Weather in the clock's tooltip or as a widget (Open-Meteo, no key needed).
- Media chip: title of what's playing, click for Now Playing (GNOME style).
- Recording/screen-share chip with a stop button (the privacy icon only shows state today).
- Optional tray (StatusNotifierItem) for apps that still rely on one (Discord, Steam,
  Nextcloud, etc.). It goes against the bar's clean look, so it should be hideable, but without it those
  apps have no way to be reopened or quit from the shell. Background Apps covers only portal apps.

### 18. More widgets · S each
The grid, store and gallery are generic (`widgets/kind.rs`, `catalog/`), so each is mostly a face:
- Calendar / Up Next (shares 4).
- Now Playing (shares the Quick Settings media tile's data).
- Weather.
- World clocks (the stroke clock with a time zone setting).
- Notes / sticky note (text_edit + a state file).
- GPU usage/VRAM (amdgpu `gpu_busy_percent` / `mem_info_vram_used`, useful given the memory work).
- Photo frame (a folder slideshow; image memory to weigh).

### 19. Battery health and charge limit · S
The Battery page could show capacity vs. design capacity and cycle count (UPower has them), and a
charge limit (80 %) where `charge_control_end_threshold` exists (needs a polkit action or a small
NixOS-managed udev rule, which the System pages can write).

### 20. Accessibility · M
Orca is toggled by a raw niri bind (`Super+Alt+S`). The shell itself draws its own text, so screen
readers can't read it. Worth tracking: AT-SPI exposure in amane (large), plus cheap wins in Shell
Settings: an **Accessibility** page with reduce motion (already a setting), increase contrast
(`Appearance` reads it), larger text (the text scale), cursor size, sticky keys/slow keys (niri/xkb
options), and a zoom toggle if niri gains one.

### 21. Users and session · S–M
Switch User is left out because greetd lacks it; a **Users** page (name, picture, password change
via `passwd` through the polkit agent) and a profile picture shown in the Power Off page and lock
screen would still be useful. NixOS users are declared, so this ties into the managed settings.

### 22. Shell crash/restart resilience · S
State like Keep Awake is deliberately in-memory, but a `restart` IPC command (re-exec in place,
keep the socket), and a visible "shell restarted" notice when the systemd unit restarted it after
a crash, would help both development and daily use.

---

## Suggested order

1. **OSD** (1): small, visible every day, tokens group already reserved.
2. **Window switcher** (2) and **calendar in the center** (4).
3. **Notification persistence + per-app settings** (3).
4. **Night Light schedule** (11) and **default apps / search engine** (12): quick wins.
5. **Displays** (6) and **Input** (9) pages, since today they mean editing niri's config by hand.
6. **Lock screen** (5): the largest item, but it removes the swaylock guesswork in Keep Awake.
7. The rest as polish.
