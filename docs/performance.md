# Keeping It Cheap

The shell runs all day, on every monitor, under everything else. A window that redraws when
nothing changed costs battery and GPU time, and nobody sees what it bought. This guide covers how
amane decides to draw, what a frame costs, the rules that keep both low, and how to check them.
Every rule here comes from a real fix. The commit that made it is named, so you can read the
before and after.

## How a frame happens

amane is immediate-mode. A window's whole view is rebuilt for every frame, then turned into draw
commands and drawn on the GPU (`amane/ARCHITECTURE.md`). A window draws again when one of these
happens:

| Trigger | Which windows | Notes |
|---|---|---|
| **Input** on it (pointer, keys, scroll) | that window | Automatic. Moving the pointer counts only if a target has `on_move`, not `on_track`. |
| **A service it read** is written | the windows that read it | A `Write` that calls `quiet()` wakes nobody. |
| **A source it read** changed | the windows that read it | `Backdrop` per shape, window `Titles` (`amane/src/changes.rs` `Source`). |
| **`moving`** | that window, every refresh | Set by an `Animation` that hasn't arrived, a shader that names `time`, or `amane::request_frame()`. |
| **`mark_all()`** | every window on every monitor | Almost never what you want. |

What a frame costs:

- **The view** runs on the CPU: your component's code, layout and text measuring. It's usually well
  under a millisecond.
- **The GPU work** depends on the commands:
  - **Rectangles, borders, glyphs and plain images** are cheap quads.
  - **Paths, strokes, shadows, inner shadows and gradients** go through vello. Each run of vello
    commands between quads renders into its own texture. A run of fills and strokes takes one the
    size of its box; one with shadows, gradients or images takes the whole surface.
  - **An opacity group** (a container at opacity other than 1) is drawn on its own surface-sized
    canvas and laid back. That's a full extra pass every frame.
  - **A rounded shader** costs three extra full passes. A square one blends straight on.
  - **A vibrant window** may draw on up to three surfaces.
- **Identical frames are skipped.** When a frame's commands equal the frame on screen, the GPU
  isn't touched and the window only commits (`amane/src/graphics/gpu/present.rs` `shows`). This
  never applies while `moving` is set, so a frame that animates is always a full redraw.

The widget desktop, for scale, is a 1646×1029 surface, and one of its frames is about 5–6 ms of
GPU time.

## The rules

### 1. Don't call `mark_all`. Scope the redraw.

Input already redraws the window it landed on. When some *other* window shows the state you
changed, declare a **marker** for it and change that:

```rust
amane_kit::marker! {
    /// The launcher's own state changed: only its window draws again.
    pub Launcher
}

pub fn surface() -> LayerWindow {
    Launcher::watch(); // this window draws again when Launcher changes
    // …
}

fn pick(index: usize) {
    STATE.with_borrow_mut(|state| state.picked = index);
    Launcher::changed(); // not mark_all()
}
```

- For state behind an accessor, put `watch()` *in the accessor*. Then whoever reads it is tracked
  without remembering to be. Examples:
  - `timer::left()` watches `Countdown`;
  - `clipboard_history::clips()` watches `Clips`;
  - `Indicator::shown()` watches `Indicators`.
- A handler that runs from a menu popup lands on the *popup's* window. Its changes still need the
  marker.
- Drag motion and scroll steps on the window that shows them need nothing.
- `kit::motion::redraw_after::<M>(delay)` changes marker `M` after a wait, for output that changes
  with time alone (a delay running out).
- Before: the launcher redrew every bar, dock and desktop for every key, the dock for every drag
  motion, and the bar's timer chip all of them once a second (shell `0c3b8ac`).

`mark_all` is right only for something every window can show: a theme change, a new material
set. Two exceptions remain: `panel::set` (a panel's state shows in the bar) and kit's dropdown
close. Both are rare.

### 2. Remember the pointer with `on_track`

`on_move` redraws the window on every pointer move. Use it only when the position is *drawn* (the
dock's magnification). To remember where the pointer is for a later click or menu, use
`on_track`, which redraws nothing (amane `6b0099b`). `kit::layout::track_cursor` already does.

### 3. Services: off the lock, quiet when unchanged, paused when unseen

- **Do the I/O with no lock held.** The default `listen()` calls `update()` inside
  `Self::write()`, so every view reading the service waits for your `/sys` read, `nmcli` or
  `statvfs`. For anything slower than a cached file read, override `listen()`:

  ```rust
  fn listen() {
      loop {
          thread::sleep(Self::interval());
          let fresh = fetch();          // no lock held
          if *Self::read() == fresh {   // nothing new: wake nobody
              continue;
          }
          *Self::write() = fresh;
      }
  }
  ```

  See `services/connections.rs` (shell `ab23c67`) and `services/system_stats.rs` (shell `dab460e`).
- **Write only what's shown.**
  - Compare at the precision you draw. Network compares signal strength by the Wi-Fi wedge it
    shows (amane `0856be2`).
  - When a write keeps state but shows nothing new, call `quiet()` on the `Write`.
  - Split a field that changes often but is rarely drawn into its own source. Titles are their
    own source, so a terminal's title spinner no longer redraws every dock (amane `45e0edc`). The
    dock reads a title only where it draws one (shell `d20af3a`).
- **Pause when nothing shows it.** Services never stop by themselves. If yours is costly, let the
  views say when they show it, the way `SystemStats::watch` does: the widget windows report
  whether a system card shows, and the reading thread sleeps otherwise.

### 4. Animations: create once, target targets, don't read what isn't drawn

- **Keep animations across frames.** Store them in your component's state and call `to()`. A new
  `Animation` is at rest (amane `527c538`; before that it drew frames for its whole duration).
  Re-creating one every frame still throws away its motion.
- **Drive a target from targets, never from another animated value.** Each different target
  restarts the spring. The dock's card width followed the sliding icons, so it started over every
  frame. Now it springs once to where the row will end (shell `8aa0445`).
- **Don't read `value()` for what isn't drawn.** Reading an unfinished animation sets `moving` and
  draws frames. A dock hidden below the screen edge put its icons where they go instead of sliding
  them unseen (shell `8aa0445`).
- **Hidden means `visible(false)`.** amane draws nothing for a hidden window, and it can't tell a
  transparent or off-screen one apart from a visible one.
- **Honor reduced motion with kit's helpers**:
  - `motion::spring`, `scale`, `travel`, `fade`;
  - the dock's `anim`;
  - `theme::reduced_motion()` for anything custom (a flick's coast, a press pulse).
- **Don't name `time` in a shader** unless it animates. A shader whose source mentions `time` (even
  in a comment) redraws every frame. `kit/src/glass.rs` has a test guarding this.
- **Pulse once.** `on_drag` reports the press and then every motion. Start a press effect on the
  first report only (kit's buttons, shell `c1bef96`).

### 5. Opacity: 1 is free, 0 is free, everything between costs a pass

- **At exactly 1.0** a container draws inline. Clamp eased values so they really land on 1.
- **At 0, or with nothing in it**, it draws nothing at all (amane `3f57058`).
- **A container holding one shape** (a highlight, a dot) takes the opacity in that shape's colour,
  with no group. The exception is a white or black that would turn vibrant when translucent; that
  keeps the group, so it looks the same.
- **Any other faded container** is a full-surface pass on *every* frame, not just while it fades.
  Don't leave a container at a lasting opacity below 1. Fade a single shape's colour (its alpha)
  instead, or fade only while something is moving.
- For a disabled or dimmed look, prefer the dimmed colour tokens to `.opacity()` on the whole
  control.

### 6. Views: no scans, sorts or clones of big lists per frame

The view runs for every frame, including every frame of every animation. Instead of a linear search
over all apps or a sort of all results on every frame:

- **Index**, as `app_icon::entry_for` does: a map built once for each `Apps` list (shell
  `2ff6129`).
- **Memoize on the input**, as `file_search::find` does with its request.
- **Build only what shows.**

### 7. Input that changes nothing sends nothing

A slider reports every drag motion and every scroll step past either end, and each value sent is a
round trip to a daemon plus a redraw. Send only a value other than the one shown (`kit::slider`
`changes_only`). Read a live value in a handler instead of the one captured when the view was
built, as the bar's volume scroll does (shell `c1bef96`).

## Measuring

`AMANE_FRAMES=1` prints a line per frame and per change on stderr:

```
frame amane.dock 1646x150 gap 16.2 view 0.3 draw 0.0 gpu 4.2
change amane::services::windows::Windows
change Titles
change Backdrop 1234
change everything            ← a mark_all
```

`gap` is the time since that window's last frame. `view`, `draw` and `gpu` are milliseconds, and
a `gpu` near 0 means the frame matched the one on screen and was skipped. A `change` line names
what woke the windows that follow it.

Run a second shell beside your bar, with its own runtime dir, so it doesn't take the bar's socket:

```sh
T=$(mktemp -d /tmp/amXXXX)
XDG_RUNTIME_DIR=$T WAYLAND_DISPLAY=/run/user/1000/wayland-1 AMANE_FRAMES=1 \
  timeout 30 nix develop -c ./target/debug/amane-shell 2> frames.log >/dev/null

# frames per window, and what changed
awk '$1=="frame"{n[$2]++} $1=="change"{c[$2]++}
     END{for(k in n) print "frame",k,n[k]; for(k in c) print "change",k,c[k]}' frames.log | sort

# which change each of one window's frames followed
awk '$1=="change"{last=$2} $1=="frame" && $2=="amane.dock"{w[last]++}
     END{for(k in w) print k, w[k]}' frames.log
```

Scenarios worth running before and after a change:

- **Idle, 30 s:** hands off.
- **Hover:** sweep the pointer over the bar, the dock and the empty desktop.
- **Type:** type into the launcher. Look for one frame per key, not a run of them.
- **Drag:** drag a dock icon, a widget, a Quick Settings tile. Only that window should draw.
- **Fade:** open a window over the desktop, so the widgets dim, and open and close a panel.

What a quiet desktop looked like on 2026-10-26 (eDP-1, 30 s idle, system widgets placed):

| Window | Before this work | After |
|---|---|---|
| widgets | 165 | 19 (one per 2 s stats tick, plus startup) |
| dock | 100–706 | 4 |
| bar | ~50 | 4 |

## Checklist for a new component or animation

- [ ] Does any handler call `mark_all`? Use input's own redraw, or a marker.
- [ ] Does anything store the pointer with `on_move` and not draw it? Use `on_track`.
- [ ] Does a service do I/O inside `update()`, report a change it doesn't show, or poll when
      nothing shows it?
- [ ] Is every `Animation` made once and kept? Does any target come from an animated value?
- [ ] Is `value()` read for anything hidden or off-screen?
- [ ] Is any container at a lasting opacity below 1? Is anything at 0 still built for nothing?
- [ ] Does the view scan, sort or clone a big list per frame?
- [ ] Does it honor reduced motion?
- [ ] Idle for 30 s with `AMANE_FRAMES=1`: does your window draw at all?

## Known remaining costs

- **Full-surface groups.** Vello runs of fills and strokes now take only their box (amane
  `bounds.rs`), but groups, and vello runs with shadows, gradients or images, still take a texture
  the size of the whole surface. That's why a dimmed widget card or a fading panel costs as much as
  it does: a clock showing seconds under a dim pays a full-surface group pass on every frame of each
  change. Bounding groups too, and after that damage tracking, are the planned fix. Until then the
  widget dim stays a group per card.
- **The launcher rebuilds** its search, actions and suggestions on every frame of its animations.
  It should memoize on the query.
- **Toasts poll at 10 Hz forever.** It's cheap, and it redraws only on change.
- **Privacy spawns `niri msg casts` every 2 s.**
- **Media's position** is part of its equality, so a playing player writes every second. Only
  Quick Settings reads it, and only while it's open.
- **Bluetooth** builds its device list afresh every 2 s and compares. A failed call reads as "no
  adapter", which is a real change if bluetoothd went away, so it's left as it is.
