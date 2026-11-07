# The contract with niri, amane and the kit

What amane-shell relies on from the repositories it is built with, and the oldest revision of
each that has it. Read this before a rebase of either fork, or before changing an interface the
shell or its apps use: if a row changes, the shell changes with it.

The pins live in two places:

| Repository | Pinned in | Checked by |
|---|---|---|
| amane (`098765434/amane`, branch `ocbk`) | the root `Cargo.toml`, `[workspace.dependencies]` | `cargo build`, `scripts/check-pins` |
| niri (`098765434/niri`, branch `ocbk`) | `contract.toml`, and nixos-config's `flake.lock` (`niri-fork`), which builds it | `cargo xtask contract` |
| vello (`098765434/vello`, branch `ocbk`) | the root `Cargo.toml`, `[patch.crates-io]` | `scripts/check-pins` |

`cargo xtask contract` (also in CI) reads both forks at their pinned revisions, from checkouts at
`../amane` and `../niri` (or `--amane <dir>`, `--niri <dir>`). It checks that:
- the protocol XMLs niri adds are in amane's `protocols/`, byte for byte, and the other way round;
- every niri IPC request, event and action, and every field, that amane's niri client sends or
  parses exists in `niri-ipc`;
- `contract.toml`'s niri revision is the one `~/nixos-config/flake.lock` builds, when that lock is
  there.

## niri

The fork's own list is in niri's
[`docs/fork/maintaining.md`](https://github.com/098765434/niri/blob/ocbk/docs/fork/maintaining.md#what-amane-shell-relies-on).
Since `bebe757d` niri answers `Features` with each one's contract version (stock niri answers with
an error), so a client can tell what is there before using it.

| Interface | Used for | Since | Feature |
|---|---|---|---|
| `niri-blur-shapes-v1` v2 | rounded, fading blur behind the bar, dock and panels; materials by name | `379f1071` | `blur-shapes 2` |
| `niri-vibrancy-v1` v1 | labels blended with the backdrop | `9cdf1c8d` | `vibrancy 1` |
| `niri-secure-fill-v1` v1 | password fill into the field that was checked | `85a7be33` | `secure-fill 1` |
| `materials {}` and `background-effect { material }` | the glass looks; the shell's layer rules name them (`niri-layer-rule.kdl`) | `5f986fdf` | `materials 1` |
| `layer-rule { desktop true }` | the desktop widgets layer | `12509ec6` | `desktop-layer 1` |
| `layout { mat {} }`, `workspace-gap`, overview gaps | the workspace look the settings write | `9165052c`, `193f7663` | `mat 1` |
| `OverviewProgress`, `ViewOffsetChanged`, `WorkspaceSwitchProgress` events | the bar and dock following gestures | `759c9c3f` | `overview-progress 1`, … |
| `DesktopObscured` event | widgets stop drawing while covered | `12509ec6`; unplugged outputs forgotten since `bd862d07` | `desktop-obscured 1` |
| `Window.is_fullscreen` | the bar hides for a fullscreen window | `26290100` | `window-fullscreen 1` |
| `MoveColumnToIndex { window_id }` | dock reordering without moving focus | `3a984abc` | `move-column-to-index-window-id 1` |
| `Output.hdr_*`, `sdr_brightness`, `peak_luminance`; `Output { Hdr, SdrBrightness }` | the Displays page and the brightness slider in HDR | `ae549528` | `hdr 1` |
| `wp-color-management-v1` | HDR video in apps | `ddd3381d` | `hdr 1` |
| `Features` | detecting the fork | `bebe757d` | — |

The layer namespaces and materials the shell's surfaces use are generated into
`niri-layer-rule.kdl` ([layer-rules.md](layer-rules.md)); a test checks every namespace has a rule
and every material value is in the range niri accepts.

## amane

The shell is built only against the pinned revision, so for amane the last column is the
revision these were last checked at, apart from the protocol copies, whose history is short
enough to follow.

| Interface | Used for | Checked at |
|---|---|---|
| `amane::niri` (`features()`, `outputs()`, `windows()`, `casts()`), `Timer`, `Service::subscribe`, `Launch` | added for the shell's idle pass (Phase 7); nothing uses them yet | `af52270` |
| `protocols/niri-*.xml` | the client side of the three niri protocols above, kept identical to niri's | all three as niri has them since `292d486` |
| `Service` and its services: `Battery`, `Apps`, `KdeConnect`, `Bluetooth`, `Windows`, `Workspaces`, `Brightness`, `PowerProfile`, `Notifications`, `Audio`, `Network`, `PhoneMessages`, `Media`, `Lock`, `Idle`, `Adb`, `Tray`, `WorkspaceSwitch`, `FocusedField`, `Desktop`, `Appearance`, `Polkit`, `Overview`, `MimeApps`, `Memory`, `Cpu`, `Backdrop` | everything the bar, Quick Settings and the apps show ([services.md](services.md)) | `af52270` |
| `Config<T>` | settings files, watched and written atomically | `af52270` |
| `status_item` (`org.amane.StatusBar1`) | status apps ([status-apps.md](status-apps.md)) | `af52270` |
| `spawn`, `spawn_activated` | launching apps with an activation token | `af52270` |
| popups, `open_popup`, `close_window`, windows, the quad and vello renderers, `material_available`, `blur_fades` | every surface | `af52270` |

Apart from `amane::niri`, amane's services, not the shell, talk to niri's socket, except for the shell
services that still run `niri msg` (listed in [services.md](services.md); Phase 7 of the
[roadmap](../../ROADMAP.md) moves them onto amane).

## The kit and the shell's own interfaces

These are inside this repository, so they change together with their users, but other programs
see them:

| Interface | Who else uses it | Documented in |
|---|---|---|
| `$XDG_RUNTIME_DIR/amane.sock` line verbs | the apps, scripts, niri binds | [ipc.md](ipc.md) |
| `org.amane.StatusBar1` on the session bus | status apps (amane-drop, train-watch, others) | [status-apps.md](status-apps.md) |
| `~/.config/amane-shell/*.json` | Shell Settings, the shell, the apps | [docs/shell/](../shell/) pages, per setting |
| the generated layer rules | niri's config (`gen-layer-rules.sh` splices them) | [layer-rules.md](layer-rules.md) |
