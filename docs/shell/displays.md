# Displays

Shell Settings' **Displays** page (`apps/settings/src/displays/`) shows each screen niri drives,
with what the screen can do found out from the screen itself, and changes its resolution, refresh
rate, variable refresh, scale, rotation, place and HDR. It also holds brightness (a monitor's
own over DDC/CI), lowering the refresh rate on battery, Night Light's schedule and, on an OLED
panel, OLED care. `amane-settings --page displays` opens it.

## What a screen can do

Three sources are read for each output, and none of them is configured by hand:

| Source | Read by | Gives |
|---|---|---|
| niri's IPC (`Outputs`) | `niri.rs` | make, model, serial, size in mm, every mode, the current one, whether VRR is supported and on, the logical size, scale, transform and place |
| the EDID, `/sys/class/drm/card*-<connector>/edid` | `edid.rs` | the panel's name, bits per colour and interface, its colour primaries (so its gamut), the refresh range (the range limits descriptor, or AMD's FreeSync block), the CTA-861 colorimetry and the HDR static metadata (PQ, HLG, peak, full-frame and black luminance) |
| the DRM connector's properties | `drm.rs` (the `drm` crate, read-only: never DRM master) | `panel_type` (amdgpu marks OLED panels), `vrr_capable`, the `max bpc` range, the `Colorspace` encodings, and whether `HDR_OUTPUT_METADATA` is there |

`caps.rs` puts them together (`Caps`) and reads them again only when the EDID changes, as when
another monitor goes into the same port. The facts show as tags under the screen's name. On the
ThinkPad T14 Gen 7's panel they are:

> **OLED** · HDR10 · 1107 nits · FreeSync 30–120 Hz · Up to 120 Hz · P3 color · 10-bit

The EDID parser covers what the page shows and nothing more. A block that's cut short is left out,
never guessed. It's tested against that panel's EDID (`displays/atna40hq10.edid`). To print what
the three sources say on this machine, run `cargo test -p amane-settings -- --ignored --nocapture live`.

The screens are polled every two seconds, and only while the page shows: niri announces no change
of outputs, so a monitor plugged in shows up on the next poll.

## Where the settings go

niri makes an output change at once over IPC (`Output { output, action }`) and keeps it only until
it reads its config again. So a lasting change is written to **`displays.kdl`**, beside niri's
`config.kdl` (where `~/.config/niri/config.kdl` leads: `~/nixos-config/niri/` here), which
includes it:

```kdl
// Displays are set in Shell Settings, which writes this file:
include optional=true "/home/ocbk/nixos-config/niri/displays.kdl"
```

The path is absolute because niri looks for an include beside the config's symlink, not beside
the file it points to. Settings owns `displays.kdl` and writes it whole each time, through a
temporary file and a rename, so niri never reads half of it. niri reloads it by itself. When the
folder is in a git repo, each change is committed alone (`displays: eDP-1 scale 1.5`), never
pushed.

```kdl
output "eDP-1" {
    mode "2880x1800@120.000"
    scale 1.75
    transform "normal"
    variable-refresh-rate on-demand=true
}

// on-demand variable refresh turns on for these apps' windows
window-rule {
    match app-id="^steam_app_1091500$"
    variable-refresh-rate true
}
```

A laptop's panel goes by its connector. An external monitor goes by `make model serial`, so it
keeps its settings on any port, or by its connector when it reports no serial.

**Move to Settings.** niri uses the first `output` block that matches, so blocks left in
`config.kdl` would win. Until they're moved, the page only shows the settings, with a
**Move to Settings…** button. After a dialog, the blocks become comments, the include takes the
first one's place, the same values go to `displays.kdl`, and both files are committed together.
A `config.kdl` with uncommitted edits is refused.

## The page

| Section | Control | Saved as |
|---|---|---|
| Arrangement | the screens to drag ([`Arrangement`](../kit/components.md#arrangement)), with two or more; a dropdown picks the screen the page shows, the ones switched off too | `position x= y=`, at once |
| Header | the name, the diagonal, the panel and connector, the mode, and the [tags](../kit/components.md#tag) | none |
| Brightness | slider, on a built-in panel, through amane's `Brightness` (logind); on a monitor, Brightness and Contrast over DDC/CI ([below](#a-monitors-own-brightness)) | none |
| Resolution | dropdown, largest first, the preferred one marked "(Default)", with what it looks like after scaling | `mode`, tried first |
| Refresh rate | segmented (a dropdown past five rates), the rates at this resolution; with a battery, **Lower the refresh rate on battery** ([below](#refresh-rate-on-battery)) | `mode`, tried first; the switch in `displays.json` |
| Variable refresh rate | segmented: Off, On demand, Always; the range it follows; under On demand, the apps that turn it on (by app id, with **Add** and **Remove**) | `variable-refresh-rate [on-demand=true]` and the `window-rule`, at once |
| Scale | dropdown, 100–300 % in quarters, each "looks like W × H", and any other scale already in use | `scale`, tried first |
| Rotation | dropdown: Standard, 90°, 180°, 270°, flipped and flipped turned (counter-clockwise, as niri turns) | `transform`, tried first |
| High dynamic range | **Use HDR**, tried first, with what the screen shows; while on, **HDR brightness**, 80–480 nits ([below](#hdr)) | `hdr`, `sdr-brightness` |
| Night Light | a switch, how warm, and when: All day, Sunset to sunrise, Set hours | the shell's state ([below](#night-light)) |
| OLED care | on an OLED panel: Dark appearance, hiding the dock, hiding the status bar, and **Screen Sleep Timing…** (Power & Lock) | `appearance.json`, `dock.json`, `status-bar.json` |
| This display | with two or more: Use this display (not the last one lit), Main display | `off` (tried first), `focus-at-startup` (one at most) |
| Advanced… | Custom scale (0.5–4, or a percentage), Custom mode (`2560x1600@90`), Overview backdrop (`#rrggbb`), and the raw facts each source gave | `scale`, `mode custom=true`, `backdrop-color` |

**Trying a change.** A change that could leave a screen unreadable is tried first: resolution,
refresh rate, scale, rotation, a custom mode, HDR, and turning a screen off. niri makes it, and
"Keep these display settings?" asks with a 15-second countdown
([`Dialog::countdown`](../kit/components.md#dialog)). **Keep Changes** writes it. **Revert**,
Escape, or nobody answering puts the old setting back over IPC, and nothing is written. Brightness,
variable refresh, the place, the main display and the backdrop apply at once. HDR brightness
applies at once and is written once the slider rests, so a drag is one commit.

**Variable refresh.** niri's on-demand mode turns VRR on only while a window that a
`variable-refresh-rate true` rule matches is on the screen, so On demand without any apps does
nothing. The note says that OLED panels can flicker at low frame rates with VRR on, which is why
On demand suits them.

## HDR

niri drives an output in HDR10 when its EDID says it decodes PQ (see niri's
[hdr.md](https://github.com/098765434/niri/blob/ocbk/docs/fork/display/hdr.md)). The page shows
what the screen can do (HDR10 support and peak, from the EDID and from niri's `peak_luminance`),
and:

- **Use HDR** is niri's `hdr` (`Change::Hdr`). It's tried first because niri sets the output up
  again from scratch, so the screen blanks for a moment. Saved as `hdr` in `displays.kdl`.
- **HDR brightness** is where SDR white sits in HDR mode (`sdr-brightness`, niri's default 203
  nits). The desktop looks the same in HDR mode; this is how bright it is. The note says the
  backlight may not follow the usual brightness while HDR is on, as on some amdgpu OLED panels.
- The note names what shows HDR: mpv with `--vo=gpu-next`, Firefox with `gfx.wayland.hdr`, and
  Vulkan games that ask for HDR. These describe their colour through `wp-color-management-v1`,
  and niri shows their highlights up to the panel's peak.

A display that supports HDR10 under a niri without it (an older build, or a screen on another GPU)
gets a note instead of the switch. The `Output` mirror reads niri's HDR fields with
`serde(default)`, so an older niri still parses.

## A monitor's own brightness

An external monitor has no backlight the kernel controls, but most take brightness and contrast
over DDC/CI (`ddc.rs`). The connector's sysfs folder links to its I²C bus (`ddc` → `i2c-N`), so
the page asks `ddcutil --bus N --brief getvcp 10` and `12` once, off the UI thread, and shows the
answers as sliders. A slider sends `setvcp` once it has rested for 150 ms, since each call takes a
tenth of a second or more. A monitor that doesn't answer gets a note that its own buttons set it.
This needs `ddcutil` and access to `/dev/i2c-*` (`hardware.i2c.enable` and the `i2c` group in the
NixOS configuration).

## Refresh rate on battery

**Lower the refresh rate on battery** (`battery_refresh` in `displays.json`) is the shell's
(`services/power/power_refresh.rs`). Every five seconds it reads amane's `Battery`. On battery, it
moves each display to its slowest rate at the same resolution (60 Hz on this panel) with niri's
temporary `niri msg output … mode`, and remembers the rate it had. On AC, or once the switch is
off, it puts that rate back. `displays.kdl` keeps the AC rate throughout, so a config reload on
battery brings it back for a moment, until the next check lowers it again. niri is only asked while
on battery, or while a display is waiting to go back.

## Night Light

The shell owns `wlsunset` ([services.md](../integration/services.md)), so the page drives it over
IPC (`night-light`, [ipc.md](../integration/ipc.md#night-light)) and watches its state file to
stay in step with the Quick Settings tile. The schedules are:

- **All day**: on at the chosen temperature until turned off.
- **Sunset to sunrise**: `wlsunset -l <lat> -L <lon>`, at the chosen temperature by night and
  6500 K by day. The place starts at the time zone's own, from `zone.tab` (`Europe/Oslo` is
  59.92, 10.75), and **Set** takes another.
- **Set hours**: `wlsunset -s <from> -S <to>`, overnight when To is earlier.

While a schedule is set, the Quick Settings tile reads "Scheduled".

## Cost

The page is quads: the kit's controls, `Tag` and `Arrangement` included. Nothing on it draws with
vello. DDC calls and niri requests run off the UI thread. Its poll rests while another page shows (`displays::showing`), and the countdown's tick runs
only while a countdown dialog is open.
