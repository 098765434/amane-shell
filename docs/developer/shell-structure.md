# The shell's structure

How `apps/shell/src` is laid out, and the few rules that keep it that way. Tests hold the rules
(`surfaces/mod.rs`, `ipc.rs`, `services/state.rs`), so breaking one fails `cargo test`.

## The workspace

A module's folder says what kind of thing it is. A new one goes where its kind is.

| Folder | What goes there |
|---|---|
| `apps/shell/src/surfaces/` | a surface of its own, a folder each: the status bar, dock, launcher, notifications, Quick Settings, widgets, alert, on-screen display and the password-fill popup (`fill/`), each with its doc in `docs/user/features/`; how a surface is laid out is [below](#a-surface) |
| `apps/shell/src/chrome/` | what several surfaces share and only the shell has (`panel`, `bar`, `per_output`, `battery`, `workspace_menu`); the rest is the kit's |
| `apps/shell/src/services/` | the shell's services, by domain: `power/`, `network/`, `display/`, `desktop/`, `launcher/`; `vault/` (rbw, for passwords); the assistant's client (`assistant.rs`); the status apps' bus server (`status_apps.rs`); helpers in `process.rs` and `state.rs` |
| `apps/` | the programs beside the shell (`apps/shell`): Emaki Settings, Messages, Phone Screen, Shader Lab (with its effects and saved looks), Drop, the assistant's daemon, and `train-watch`, which follows a training run from the status bar |
| `crates/` | what they share: the kit (`crates/kit`, with the design tokens in `design-tokens/`), the status-item client (`status`), the phone (`phone`), the NixOS model (`nixos`), a process runner (`process`), logging (`log`), rbw's pinentry and the assistant's library |
| `apps/shell/tests/fixtures/` | recorded output the shell's parsers are tested on |
| `xtask/` | the workspace's tasks: `cargo xtask layer-rules`, `tokens`, `lint-terms`, `links` |
| `apps/shell/src/tokens/` | what the shell generates from the tokens (niri's layer rules) |
| `crates/kit/src/controls/` | what's pressed, dragged or picked: buttons, slider, dropdown, color picker, tag, control sizes |
| `crates/kit/src/containers/` | what content sits in: card, list, divider, app bar, dialog, tooltip, scroll view, flow, arrangement |
| `crates/kit/src/look/` | the look of glass: materials, the edge shader, glass regions, Shader Lab's looks |
| `crates/kit/src/animation/` | motion, kinetic, pager, edge reveal |
| `crates/kit/src/foundation/` | layout primitives, icons, color arithmetic |
| `crates/kit/src/platform/` | an app's window, app icons, the settings files, markers, secrets |
| `crates/kit/src/tokens/`, `theme.rs` | the design tokens and the theme they resolve with |

The kit's folders don't show in its paths: every module is re-exported at the root, so it's
`emaki_kit::card`, not `emaki_kit::containers::card`.

## Folders

| Folder | What goes there |
|---|---|
| `main.rs` | startup: logging, the agents and services that start once, the surfaces registered on the `App` |
| `ipc.rs` | the one table of IPC verbs: each read into the kit's `ShellCommand` and handed to its surface |
| `surfaces/<surface>/` | one surface: the bar, the dock, the launcher, notifications, Quick Settings, widgets, the password fill, the alert, the OSD |
| `chrome/` | what several surfaces share and only the shell has: which panel is open (`panel`), where the bar is (`bar`), per-monitor state (`per_output`), the battery glyph, the workspace submenu |
| `services/` | state only the shell has, and what it does with the system: the phone, Do Not Disturb, the clock, media art, the vault, power, network, display; and the helpers `state` (state files), `child_stream` (a program's output, kept going), `launch` |
| `tokens/` | what the shell generates from the design tokens (niri's layer rules) |

The menu, the shell IPC type, file writing and the rest of what apps share are the kit's
(`crates/kit`).

## A surface

A surface is a folder. Its `mod.rs` is its only face to the others: the surface itself (`surface`,
`bar`, `popup`), the functions other surfaces call (`launcher::open(Mode)`, `osd::step_volume`,
`quick_settings::open_alone`), its namespace, and its IPC entry (`run`). What it keeps and draws is
in files of its own, as it needs them:

| File | What |
|---|---|
| `mod.rs` | the surface, its public functions, its state (thread-locals, types) |
| `view.rs` | how it draws |
| `input.rs`, `keys.rs` | the pointer and the keyboard |
| `ipc.rs` | `run(command)`: its part of `ShellCommand` |
| others | what's particular to it (the launcher's `provider/`, the bar's `items.rs`) |

The rules, each a test in `surfaces/mod.rs`:

- No surface names another's modules (`surfaces::launcher::provider::…`): it uses what that
  surface's `mod.rs` offers.
- No surface calls another's IPC: they call each other's functions.
- `services/` and `chrome/` name no surface. A surface registers with `chrome` (as
  `panel::register`) rather than `chrome` calling it.
- No file under `surfaces/` is over 650 lines. Past that, split it along what it does: the
  functions move to a child module (`use super::*;` sees the parent's private items), the types
  and state stay.

## What a surface builds with

Before writing a helper, look for it here; the hygiene test (`xtask/src/hygiene.rs`) fails on some
of them written again.

| Need | Use |
|---|---|
| a box of a size, room to leave | `layout::frame(width, height)` (a `Rectangle` starts transparent) |
| a rectangle on the surface, for hit tests and frost | `emaki_kit::area::Area` |
| opening and closing | `reveal::Reveal` for the motion, `reveal::Swell` for the scale, opacity and frost it draws at, `reveal::showing` for whether to draw at all, `panel::empty` while there's nothing |
| motion timed to something outside the kit | `motion::eased` (still for reduced motion) |
| a click | `buttons::Actions::press(f)` |
| a choice among values | `Dropdown::choice(id, [(label, value)], now, pick)` |
| a field showing its setting | `app::fill_once(id, value)` |
| a sheet things are added from | `chrome::catalog::Sheet` (Customize, the widget gallery) |
| edit mode's marks on a grid item | `chrome::edit_marks` |
| a row that opens more | `list::chevron()` |
| a listener keeping what it read | `service::ServiceExt::replace(fresh)`; a listener reads its own service with `peek()`, never `read()` |
| times, sizes, counts in words | `format::{ago, span, span_exact, bytes, count}` |
| folders, whole files, now, locks, flags, file types, `%` escapes, `'static` ids | `emaki_base::{paths, fs, time, sync, args, mime, percent, intern}`, through the kit |

## Tables

A fact written in several places is written once, as a table the rest is derived from:

- **Quick Settings' controls:** `quick_settings/controls.rs` `SPECS`, one `Spec` per control (name,
  title, sizes, how it's drawn, whether this machine has it), in `ControlId`'s order.
- **Emaki Settings' pages:** `apps/settings/src/pages.rs` `SIDEBAR` (the `--page` names, titles,
  search words, bodies, sub-pages, toolbar buttons, which pages show the Apply bar), with
  `MOVED` for the places old names still lead to.
- **Settings files:** `settings_file!` in `crates/kit/src/platform/settings/mod.rs`.
- **The IPC's keywords:** `words!` in `shell_ipc.rs`, and `handlers![…]` in `ipc.rs`.

## State, settings and logs

- **Settings** are the kit's `Config<T>` files in `~/.config/emaki/`, which Emaki Settings
  writes too.
- **State** the shell keeps for itself is a `State` type (`services/state.rs`): JSON with a
  version, written whole, in `$XDG_STATE_HOME/emaki/`
  ([services.md](integration/services.md#settings-and-state)). A file in an older format is
  read by the type's `legacy()` and left as it was.
- **Logs** go through the `log` crate. Each program calls `emaki_log::init` once; `EMAKI_LOG`
  (`warn`, `debug`, …) sets the level, `info` by default. A fallible call whose result is dropped
  says why in a comment, or logs.
