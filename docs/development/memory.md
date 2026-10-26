# Keeping It Small

The shell runs all day on an integrated GPU whose "VRAM" is ordinary RAM reserved at boot. This
guide is the memory side of [Keeping It Cheap](performance.md): what the desktop holds at idle,
why GPU memory is the number to watch, how to measure it, and what has been done about it. Each
fix says what it cost, what changed, how to switch it off and how to check it.

## Why GPU memory

On this laptop (Ryzen AI 7, Radeon 860M, 2880×1800 at 1.75, 32 GB) the firmware carves 4 GiB out
as VRAM before Linux boots, so it is already missing from `free`. Measured on 2026-10-26 the
desktop held 1.96 GiB of it at idle, plus 318 MB of GTT (system RAM mapped for the GPU), while
niri's own heap was 34 MB and the shell's 123 MB. Lowering GPU use pays twice: usage past the
carve-out spills into GTT, which is regular RAM; and once the desktop fits comfortably under
1 GiB, the BIOS UMA frame-buffer size can be lowered, which hands 2–3 GiB back to Linux. That is
far bigger than anything left in process RSS.

A full-output texture here is 2880×1800×4 = 20.7 MB. A layer surface costs roughly 4–5× its
pixels: a swapchain of 3–4 images plus amane's canvas, and a vibrant surface draws on up to three
such surfaces.

## Measuring

Per-process GPU memory is in each process's `fdinfo`. Reading niri's or the installed shell's
needs root (`ptrace_scope 1`); an example or a second shell you start yourself is readable as is.

```sh
gpu() { sudo awk '/drm-client-id/{id=$2} /drm-total-(vram|gtt):/{m[id SUBSEP $1]=$2*($3=="MiB"?1024:1)}
  END{for(k in m){split(k,a,SUBSEP); t[a[2]]+=m[k]} for(x in t) printf "  %s %.0f MiB\n", x, t[x]/1024}' /proc/$1/fdinfo/*; }
echo niri; gpu $(pidof niri)
echo shell; gpu $(pgrep -f '\.amane-shell-wrapped')
grep -E 'VmRSS|RssAnon' /proc/$(pidof niri)/status /proc/$(pgrep -f '\.amane-shell-wrapped')/status
cat /sys/class/drm/card*/device/mem_info_vram_used   # the whole GPU, in bytes
```

Record it at idle, after opening and closing the launcher and Quick Settings' edit mode, after a
screenshot, and after a window swipe. A second shell runs beside the live one with its own
runtime dir (see [performance.md](performance.md#measuring)); note that its widgets layer stays
frozen while a window covers the desktop, so it draws nothing through vello until the desktop
shows.

## Done

### vello's scratch buffers: a fork of vello

**What it cost.** Every amane process that draws a path, gradient, shadow or image through vello
paid 130–360 MB of GPU memory for vello's bump-allocated scratch buffers, sized upstream for a
30,000-path map of Paris. The shell pays it once (one renderer per process), as do Shell
Settings and the Shader Lab while they run.

**What changed.** amane's `vello_encoding` comes from
[098765434/vello](https://github.com/098765434/vello) (branch `ocbk`, one commit on tag
`v0.11.0`), with the buffers sized for a shell: about 29 MB a set instead of 165. Because a
`[patch.crates-io]` only applies from the root manifest, and amane is a git dependency here,
`Cargo.toml` carries the same patch entry as amane's, and `flake.nix` has the fork's hash in
`outputHashes`. When amane bumps its vello, both move together.

**Switching it off.** `VELLO_SCRATCH=upstream` in the shell's environment restores upstream's
sizes without a rebuild. An overflow is silent (the run comes out blank), so if something that
should draw through vello shows nothing, try that first.

**Checking a scene.** Build with amane's readback feature and run with frame logging:

```sh
cargo build -p amane-shell --features amane/vello-stats
AMANE_FRAMES=1 ./target/debug/amane-shell 2>&1 | grep ^scratch | sort -k10 -n | tail
```

Each vello run prints a `scratch WxH failed 0 tiles N lines N seg_counts N segments N …` line.
`failed 1` is an overflow. The fork's limits are 2¹⁸ (262,144) for tiles, lines, segment counts
and segments, and 2²¹ for ptcl; the heaviest amane example scene, a stroked clock face, takes
36k lines and 37k segments. Open the launcher, the notification centre with images, Quick
Settings' edit mode and the Shader Lab while it runs, since those are the largest scenes.
Frames are slow with the feature on; it is for checking, not for running.

**Measured.** amane's `gradient` example: 193 MB → 65 MB of VRAM. The shell run this way on
2026-10-26, with the desktop showing and the launcher and Quick Settings opened: 231 vello runs,
none failed; the largest took 38.7k tiles and 104k ptcl entries (Quick Settings), 3.5k segments
and 1.1k lines (the widgets layer), 7× or more under every limit. The desktop's figures before
and after go in the table below as each phase lands. See amane's
[memory.md](https://github.com/098765434/amane/blob/ocbk/docs/fork/internals/memory.md) for the
mechanism and the per-example baseline.

### niri's global blur matches glass

Every blurred rule names a material with its own `blur-passes`/`blur-offset`, and niri keeps one
blur chain per distinct pair in each xray buffer (about 26 MB each at 2880×1800), plus one for
the global `blur {}` whether or not anything uses it. The global block is now glass's 3/2
(nixos-config `niri/config.kdl`), so glass draws from that chain and no extra one is built.
Noise and saturation are per-element, so nothing looks different. `hotkey-overlay {
skip-at-startup }` saves its cached texture per output too. A new material should reuse an
existing pair unless it needs its own.

### Only the Radeon Vulkan driver

The wrapper named every driver in `/run/opengl-driver/share/vulkan/icd.d` (13 on NixOS,
lavapipe among them), and the loader opened them all: about 6.5 MB of RSS per amane process.
`flake.nix` now names only `radeon_icd.x86_64.json` when it exists, the whole dir otherwise,
and pins `WGPU_BACKEND=vulkan` so wgpu doesn't probe GL as well.

### Xwayland no longer starts at login

PipeWire's X11-bell module opened `DISPLAY` at login, and under niri that connection started
xwayland-satellite's Xwayland (about 85 MB between them) with no X app running. nixos-config
sets `module.x11.bell = false`; an X app still gets Xwayland on demand. Found with
`sudo ss -xp | grep X11-unix`, then the peer inodes.

### Hidden windows release their GPU memory (amane)

The launcher, Quick Settings, toasts and widget-edit hide with `visible(false)`. amane now lets
go of everything a hidden window held on the GPU (swapchain, canvas, spares, kept results,
pictures, letter atlas, vibrant layers) and makes it afresh on show; spare textures an animation
passed through are dropped after the frame; and the swapchain is Fifo with one frame of latency
instead of Mailbox with four images. `AMANE_KEEP_HIDDEN=1`, `AMANE_KEEP_SPARES=1` and
`AMANE_PRESENT=mailbox` restore each old behaviour. Measured on a second shell (launcher opened
and closed twice, then Quick Settings): 354 MB before, 128 MB after, and reopening no longer
grows. Quick Settings leaves about 115 MB of GTT behind when it closes; that is next to look
at. Details in amane's memory doc.

## Planned

In order of payoff, from the 2026-10-26 research: niri clearing blur-shape captures, resetting a
swiped tile's offscreen, and freeing the screenshot targets' xray buffers; dropping vello's
renderer when idle (what the launcher still leaves behind); the widgets surface sized to its
grid rather than the output, with content-sized vibrant layers; the stats tick repainting only
on a visible change; and images decoded at their drawn size. Each lands with a section above and
a row here:

| Scenario | 2026-10-26 baseline | After |
|---|---|---|
| idle, whole GPU | 1.96 GiB VRAM + 318 MB GTT | |
