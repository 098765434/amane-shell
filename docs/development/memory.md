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

**Measured.** amane's `gradient` example: 193 MB → 65 MB of VRAM. The desktop's figures before
and after go in the table below as each phase lands. See amane's
[memory.md](https://github.com/098765434/amane/blob/ocbk/docs/fork/internals/memory.md) for the
mechanism and the per-example baseline.

## Planned

In order of payoff, from the 2026-10-26 research: niri's global `blur {}` set to glass's
passes/offset so the unused base chain goes away; a radeon-only `VK_ICD_FILENAMES` in the
wrapper (−6.5 MB RSS per amane process); amane releasing a hidden window's GPU resources, ageing
out spare textures and presenting with Fifo at one frame of latency; niri clearing blur-shape
captures, resetting a swiped tile's offscreen, and freeing the screenshot targets' xray buffers;
the widgets surface sized to its grid rather than the output, with content-sized vibrant
layers; the stats tick repainting only on a visible change; and images decoded at their drawn
size. Each lands with a section above and a row here:

| Scenario | 2026-10-26 baseline | After |
|---|---|---|
| idle, whole GPU | 1.96 GiB VRAM + 318 MB GTT | |
