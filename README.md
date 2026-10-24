# amane-shell

A custom Wayland desktop bar for the [niri](https://github.com/YaLTeR/niri) compositor, built on the
[amane](https://github.com/098765434/amane) shell library (a fork). macOS-flavoured: a clean
opaque bar with rounded pills and generous spacing — compact and ambient in normal use, expanding
into an interactive dock while niri's overview is open. Its floating surfaces (context menus, the
panels, notification toasts and the notification center) are the frosted, compositor-blurred
ones — the bar itself is not.

> **Requires a forked niri** for its signature feature (the overview smooth-expand needs live
> gesture-IPC events): [098765434/niri](https://github.com/098765434/niri). It runs on
> stock niri without that animation.

## Features

- **Taskbar** ([docs/taskbar.md](docs/taskbar.md)) — bottom bar with three zones:
  - left: overview toggle + workspace pips;
  - center: focused-window title (compact) ↔ interactive window dock (overview);
  - right: volume, Bluetooth, network, battery, notifications, clock.
  - **Overview smooth-expand**: the bar height eases between compact and tall in lockstep with
    niri's overview zoom, driven by the amane `Gestures::overview_progress()` value (requires the
    [niri gesture-IPC fork](https://github.com/098765434/niri)).
- **Panels** — the status items open frosted cards: Sound (volume and devices), Bluetooth
  (connect, pair, forget), Wi‑Fi (scan and join) and power (battery, power profile, peripherals).
- **Context menus** ([docs/context-menu.md](docs/context-menu.md)) — right-click a window pill for focus/close/fullscreen/float/move-to-workspace,
  built on amane's xdg-popup support (nested submenus, outside-click dismiss).
- **Notifications** ([docs/notifications.md](docs/notifications.md)) — the shell is the
  notification daemon (through amane): macOS-style toasts at the top-right (hover to hold, swipe
  to put away), a notification center sidebar opened from the clock with iOS-style stacks per
  app, and Do Not Disturb (on, for an hour, until this evening or tomorrow; critical
  notifications still show).
- **Quick Settings** ([docs/quick-settings.md](docs/quick-settings.md)) — GNOME's quick
  settings in the macOS Control Center look, from the bar's slider button. The system row
  (battery, screenshot, lock, power menu), Now Playing, volume, microphone and brightness
  sliders, and tiles for wired, Wi‑Fi (and hotspot), VPN, Bluetooth, power mode, night light,
  dark style, Do Not Disturb, keyboard light, airplane mode and background apps. Each shows only
  when the machine has it, and each opens a page of its own that slides in. The bar shows GNOME's
  privacy icons (screen shared, camera, microphone) and the settings that are on.
- **Buttons** ([docs/buttons.md](docs/buttons.md)) and **components**
  ([docs/components.md](docs/components.md)) — shared push and icon buttons, a switch, a
  segmented button, cards, top app bars, lists, dividers and a slider, each after its Material 3
  counterpart but drawn in the shell's macOS/iOS look.
- **Design tokens** — every colour, size, gap and font is a token designed in Penpot and exported
  to `design-tokens/tokens.json`, compiled in with `include_str!` and resolved the way Penpot does
  (`src/tokens/`). See [docs/design-token-manual.md](docs/design-token-manual.md).

## Build & run

A Nix flake (`flake.nix`) provides a devShell with the Rust toolchain and the required
wayland/vulkan/pulse/fontconfig libraries wired up (no hardcoded store paths), so plain `cargo`
works inside it:

```sh
nix develop -c cargo build          # compile
nix develop -c cargo run            # run  (follows the system light/dark preference)
nix develop -c env AMANE_THEME=light cargo run   # force the light theme
```

The theme follows the system preference (the Settings portal), else dark. `AMANE_THEME=light|dark`
forces one, `AMANE_DENSITY=compact` tightens spacing and rows, and `AMANE_TEXT_SCALE` (0.5–3)
scales text.

For a release binary, `nix build` produces `./result/bin/amane-shell`, wrapped with the runtime
libraries it needs (so it runs standalone).

The `amane` library is a **git dependency** pinned to a rev (`Cargo.toml`). To develop the shell and
the library together, check `amane` out beside this repo and add a git-ignored
`.cargo/config.toml` with `paths = ["../amane"]` — see [CONTRIBUTING.md](CONTRIBUTING.md).

> **Do not** build this shell with the upstream `amane` CLI (`amane dev`/`compile`): it compiles
> against its own bundled library copy, **not** this fork (so it lacks `Gestures` and won't compile
> the bar). The flake is the one canonical build path.

## niri layer-rules (for the frosted popups)

The shell's **floating** surfaces (context menus, the panels, Quick Settings, notification toasts and the center) get their blur + corner
rounding from matching `layer-rule` blocks in your niri config. The bar itself is opaque and needs
no rule. `niri-layer-rule.kdl` holds those blocks, generated from the design tokens
(`cargo run -q -- layer-rules > niri-layer-rule.kdl`), so the blur's corners follow the cards'
radii. `./gen-layer-rules.sh` injects the blocks into a target niri config between marker
comments, so the two never drift (no hand-copying):

```sh
./gen-layer-rules.sh /path/to/your/niri/config.kdl   # e.g. ~/.config/niri/config.kdl
```

See [docs/layer-rules.md](docs/layer-rules.md) — the namespaces must match the Rust
`LayerWindow::namespace(...)` calls.

## Running it persistently

Autostart the bar with your compositor/session. A **systemd user service** is the robust option
(crash recovery + logs); as a home-manager snippet:

```nix
systemd.user.services.amane-shell = {
  Unit = { Description = "amane-shell bar"; PartOf = [ "graphical-session.target" ]; After = [ "graphical-session.target" ]; };
  Service = { ExecStart = "${pkgs.amane-shell}/bin/amane-shell"; Restart = "on-failure"; };
  Install.WantedBy = [ "graphical-session.target" ];
};
```

Or, more simply, a niri `spawn-at-startup "amane-shell"` line (no crash recovery).

## Layout

```
src/
  main.rs                       App builder: windows, popups, IPC handlers, `tokens` CLI
  theme.rs                      runtime inputs: mode, accent, density, text scale
  tokens/                       Penpot token resolver and the manual's checks
  components/
    taskbar/                    the bar (mod.rs), its panels (panel.rs: audio, bluetooth,
                                wifi, power) and the clock
    context_menu/               reusable nested context menu on xdg-popups
    notifications/              toasts, the notification center and Do Not Disturb
    buttons/                    push, icon and segmented buttons, and a switch
    card.rs app_bar.rs list.rs  shared M3-style components (docs/components.md)
    divider.rs slider.rs
    layout.rs icons.rs          layout primitives, glyphs, colour arithmetic, motion
    color.rs motion.rs
build.rs                        generates `tokens::sys::…`, `tokens::bar::…`, … from tokens.json
design-tokens/tokens.json       Penpot export (normalized); edit in Penpot, not here
tests/fixtures/                 token exports the resolver is tested against
niri-layer-rule.kdl             layer rules generated from the tokens (see docs/layer-rules.md)
gen-layer-rules.sh              injects those rules into the niri config
flake.nix                       devShell (Rust toolchain + wayland/vulkan/pulse libs)
```

## License

MIT — see [LICENSE](LICENSE). The bar links the `amane` library (also MIT) and talks to niri over
**IPC**, so the GPL-3.0 niri fork imposes no copyleft obligation on this crate.
