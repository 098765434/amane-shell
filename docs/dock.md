# Dock

The macOS Big Sur dock, for the apps open on each workspace (`src/components/dock/`). It's one
frosted glass card over the bar on every monitor (`App::window_per_monitor`), namespace
`amane.dock`.

## What it shows

- **Open apps only**: one icon per app (grouped by app_id, or by title for a window without one).
  Apps keep the order they opened in. A dot under an icon stands for each of its windows, up to
  three, and the dots are brighter while one of those windows has the focus.
- **A page per workspace** on its monitor: every workspace with a window open, plus the active
  one even when it's empty. A column of dots at the card's left end stands for those pages. The
  page being shown is lit, and the active workspace is ringed while another page is being browsed.
- Coming up, it shows the **active workspace**.

## Using it

| Do | What happens |
|---|---|
| Touch the bottom edge | It comes up over the bar; it goes back down 400 ms after the pointer leaves |
| Open niri's overview | It rises in step with the overview's zoom (`Overview::progress`) |
| Two fingers up/down on a touchpad | The icons follow the fingers to the next/previous workspace's apps. When the fingers lift, it settles on the nearest page, or a flick turns the page |
| Mouse wheel | A page per notch |
| Click a dot | Goes to that workspace's apps |
| Click an icon | Focuses the app's latest window; clicking again cycles through its windows |
| Right-click an icon | That window's menu (focus, fullscreen, float, move to workspace, close) |

Swiping, the wheel and the dots change only what the **dock** shows. The workspace niri shows
stays the same until you click an app.

Under the pointer, the icons swell as on macOS: 1.5× for the hovered icon, 1.2× and 1.1× for its
neighbours, with lifts of 10, 6 and 0 px. The app's name shows above the hovered icon.

## Configuring the motion

Every animation is in `DOCK` in `dock/motion.rs`, one `Anim` each (`Anim::spring(ms, bounce)`,
`Anim::ease(ms)`, `.off()` to switch one off):

| Field | Moves |
|---|---|
| `reveal`, `hide_delay` | coming up and going down; how long it stays after the pointer leaves |
| `follow_overview` | whether it rises with the overview |
| `page`, `flick`, `rubber_band` | settling on a page; how fast a swipe turns the page; the pull past the ends (`None` stops there) |
| `resize`, `item` | the card widening; icons growing in and neighbours making room |
| `magnify` | the swell's scales and lifts, `smooth` (follow the pointer across an icon, or step like the reference CSS), its `hover` coming and going; `None` turns magnification off |
| `tooltip` | the name fading in; `None` shows no name |

`AMANE_DOCK_ANIMATIONS=off` (or `0`, `false`, `no`) turns every animation off at once. The dock
then jumps instead of moving, but still magnifies, names and hides as configured. The view never
checks the flags: `motion::anim` makes each animation, and gives back one that arrives at once
when it's off.

## How it's built

| File | What |
|---|---|
| `mod.rs` | the surface, per-monitor state and the frame's layout |
| `model.rs` | pages and apps from niri's windows and workspaces; which window a click focuses (pure, tested) |
| `pager.rs` | swipe, wheel and snapping (pure, tested) |
| `magnify.rs` | swell and spread (pure, tested) |
| `item.rs` | an icon, its running dots and its name label (inert) |
| `motion.rs` | `DockMotion` |

It reuses the shared [`EdgeReveal`, `PageDots`, `app_icon`, `Divider::vertical` and
`glass_region`](components.md).

- **One size.** The surface spans the monitor and is as tall as the card plus room for the swell
  and the name, so it never resizes; a resize every frame costs a compositor round-trip. It is
  `Layer::Overlay` with `Zone::Ignore`, reaching the real bottom edge over the bar. While hidden,
  its input region is a 2 px strip along that edge. While out, the region is the card's column too.
- **A still layer takes the pointer.** A clear layer over the whole surface comes first in the
  frame and holds every handler: hover, motion, scroll and click. The pointer is measured
  against the row **at rest**, so the swell can't feed back on itself, and the icons themselves
  are inert. (amane finds hovered and dragged targets by their place in the frame; see
  `quick_settings/edit.rs`.) The dots take their own clicks.
- **The spot under the pointer stays put.** The swollen icons before the pointer move back by
  what swelled before it, and the rest move on by what swelled after. The card grows out at both
  ends.
- **Glass** is Quick Settings': `glass_region::module` frosts behind the card (and the name
  label), and `glass_region::lit` adds the shader's edge where the compositor has no glass
  material. The niri layer rule is `clear` (`xray false` only); the surface says where to frost.
- **Touchpad vs wheel** comes from amane's `Scroll::source`, and the fingers lifting from
  `Scroll::stop` (`wl_pointer` `axis_source`/`axis_stop`). A swipe whose stop never arrives ends
  200 ms after its last movement.

## Known limits

- The name label is drawn inside the surface, not as a [tooltip](components.md#tooltip) popup.
  amane hangs a popup under the first surface with the parent's namespace, and every monitor's
  dock shares `amane.dock`. For the same reason, a window's menu opened from a dock on a second
  monitor may open on the first.
- An app that closes leaves at once; its neighbours slide together, but it doesn't shrink away.
