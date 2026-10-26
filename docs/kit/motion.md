# Motion

How the shell moves (`amane_kit::motion`, `kinetic` and `pager`, in `kit/src/`). amane animates
with `Animation`s (a duration and an easing, springs included); the kit picks the shell's springs
for them, keeps lists of cards moving between rebuilds, and adds the momentum a touchpad flick
leaves, which the compositor doesn't draw.

## Springs and fades (`motion.rs`)

Every control moves on one of these, so the shell moves alike everywhere:

| Function | Spring | For |
|---|---|---|
| `spring(value)` | 450 ms, bounce 0.15 | a knob, a thumb, a card moving |
| `sheet(value)` | 500 ms, bounce 0.1 | a whole sheet sliding in |
| `zoom(value)` | 500 ms, bounce 0.05 | a page growing out of the control that opens it |
| `pop(value)` | 380 ms (`POP`), bounce 0.08 | a popover swelling in over the bar |
| `fade(value, duration)` | an ease-out | fades and exits: `FADE` 200 ms, `EXIT` 300 ms, `HOVER` 120 ms, `PRESS` 220 ms, `CLOSE` 160 ms (a popover going, quicker than it came) |

`ENTER_SCALE` (0.9) is how small a card enters. `settle()` and `spring_curve(progress)` give the
spring's length and curve for motion that runs on its own clock rather than an `Animation`. The
values are constants until Penpot has `sys.motion.*` tokens
([penpot-backlog.md](../design-system/penpot-backlog.md)).

**Reduced motion.** Where motion is reduced (`reduce_motion` in `appearance.json`, else the
system's; `amane_kit::theme::reduced_motion`), every spring is a short fade instead, and what swells
or travels in arrives in place: `scale(from)` gives 1 and `travel(distance)` 0. An `Animation` takes
its curve when it's made, so one kept for the shell's life follows a change after a restart;
`scale` and `travel` follow it at once.

**A redraw later.** amane has no timers. `redraw_after::<M>(delay)` changes the marker `M` once
`delay` has passed (a thread sleeps it), so the windows that watch it draw again: the bar's timer
chip ticks this way ([performance.md](../development/performance.md)).

## Keyed list motion

For a list of cards (the toasts, the notification center's stacks), the view says where each item
should be every time it runs, and `animate(scope, items, arrive)` springs each there from wherever
it is, keeping its `Animation<Pose>` across rebuilds under its key:

- A `Pose` is where one card is and how it looks: moved by `x`/`y`, scaled about its centre, the
  whole card at `opacity` and its content at `content` (a stacked card behind another is a blank
  plate).
- An `Item` is one entry: its `key`, `value`, `height`, and its `pose` now, the `enter` pose it
  comes in from the first time and the `exit` pose it goes to once it's no longer listed.
- `Arrive::Enter` brings new items in from their `enter` pose; `Arrive::Rest` puts them in place
  (opening a panel shouldn't animate everything in it).
- What it gives back (`Shown`) is the leaving items first, so they pass under the rest, each still
  drawn with its last value until it reaches its exit pose.
- `jump(scope, key, pose)` puts an item at a pose at once, springing on from there (where a swipe
  let go); `forget(prefix)` drops the lists whose scope starts with `prefix`.

## Kinetic and AxisLock (`kinetic.rs`)

`Kinetic` is one axis of scrolling from 0 to `set_max(max)`, in pixels:

- `follow(moved, stop, now)` follows fingers one to one, pulling on a rubber band past the ends.
- When the fingers lift, it coasts as far as their speed carries it (an iOS-like projection eased
  out over a short coast) or springs back from past an end. Fingers that rested before lifting
  leave no momentum.
- `scroll(&scroll, axis, wheel_step)` does either from an amane `Scroll`: fingers are followed, a
  wheel eases on a notch at a time.
- `go(to)` eases there (bringing something into view), `jump(to)` puts it there at once.
- `value()` is where it is this frame; call `settle_stale()` once a frame for a compositor that
  never says the fingers lifted.

`Tracker` is the smoothed finger velocity underneath, shared with `Pager`. `AxisLock` lets one
two-finger gesture either scroll or turn the page: the first few pixels pick the axis, and it
keeps to it until the fingers lift.

## Pager (`pager.rs`)

Turning pages with the fingers: `Pager::new(axis, &motion)`, then `scroll(…)` or
`follow(moved, stop, page_px, count, &motion)` while two fingers slide. The page follows them one
to one and settles on the nearest page when they lift, or the next one on a flick (never more
than one). A wheel steps a page a notch (`step`), and `go(page)` / `jump(page)` move there with or
without the spring. `offset(count, &motion)` is the fractional page to draw, rubber-banded past
the ends. `PagerMotion` holds the settle's duration and easing, the flick speed and the rubber
band. The launcher turns it sideways through its categories.

## The dock's `Anim`

The dock keeps every animation it runs in one table, `DOCK` in `src/components/dock/motion.rs`
(shell code, not the kit's): an `Anim` each (`Anim::spring(ms, bounce)`, `Anim::ease(ms)`, `.off()`),
made through `dock::motion::anim`, which gives a motion that arrives at once when it's off, so the view
never asks. `AMANE_DOCK_ANIMATIONS=off` turns them all off. See
[dock.md](../shell/dock.md#configuring-the-motion).
