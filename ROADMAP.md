# Roadmap: a clean, maintainable amane-shell

This plan turns amane-shell and its forks (amane, niri, vello) into one deliberate system, in phases that each leave the shell working on the daily machine. It was written on 2026-11-07 from a review of all four repos at these revisions:

| Repo | Branch | Revision | Notes |
|---|---|---|---|
| amane-shell (`shell/`) | main | `3f683d7` | plus uncommitted bar-sizes work in the tree |
| amane (`amane/`) | ocbk | `74b5ddb` | 3 commits ahead of the rev the shell pins (`711ed5f`), not pushed |
| niri (`niri/`) | ocbk | `16384e00` | 2 commits ahead of origin, not pushed |
| vello (`vello/`) | ocbk | `46585523` | |

After Phase 0 (2026-11-07), all of it is pushed: shell `main` `859debc`, which pins amane `14eb0dd` and vello `46585523` by rev; niri `ocbk` `72d679c5`. After Phase 1 (2026-11-07), shell `main` pins amane `db47c38`. Phase 2 changed docs only in amane (`77b63cc`) and niri (`ac07c307`). After Phase 3, shell `main` pins amane `292d486`. After Phase 4, shell `main` pins amane `112da4d`, in the new layout (`apps/shell`, `crates/`, `xtask`). After Phase 5, niri `ocbk` is `745e72d5` (history tidied; the old branch is `ocbk-pre-tidy`). After Phase 6a, shell `main` pins amane `af52270`.

## Ground rules

These hold for every phase.

- **The shell keeps working after every phase.** Each phase ends with `nix build`, a NixOS switch, and the live checks listed in its acceptance criteria. No phase depends on a later one to be usable.
- **The Nix flake is the canonical build.** `cargo` is for development; a phase is done only when `nix build` passes.
- **Formats change only with a migration.** Settings (`~/.config/amane-shell/*.json`), state files (`$XDG_STATE_HOME/amane-shell/*`) and niri's `displays.kdl` may change shape, but the shell reads the old form once and rewrites it, so nothing is lost on a switch. The `amane.sock` IPC verbs and the `org.amane.StatusBar1` D-Bus interface stay wire-compatible; additions are fine, removals are not.
- **The forks are never upstreamed. They must stay cheap to rebase.** niri follows upstream **release tags**; amane follows **upstream `main`**; vello follows upstream `main` when a rebase is wanted. "Cheap to rebase" means few hunks in hot upstream files, one group of commits per feature, and fork code in new files where it can be.
- **No mentions of macOS or other Apple platform terms** in code, identifiers, comments, docs or the README/CONTRIBUTING. Features are named for what they are: "the launcher", "Quick Settings", "glass". Commit history is not rewritten, but new commit messages follow the rule.
- **New UI stays on amane's quad pipeline.** vello only where it can't be avoided.

Sizes: **S** is an evening or two, **M** about a week of part-time work, **L** several weeks. Tasks are labelled by repo: **[shell]**, **[amane]**, **[niri]**, **[vello]**, **[nixos-config]**.

---

## Where things stand

### Layers and how they talk

```
niri fork ── IPC socket (JSON events + actions) ───────────────┐
   │         private protocols: niri-blur-shapes-v1 (v2),      │
   │         niri-vibrancy-v1, niri-secure-fill-v1             │
   │         config: materials {}, background-effect {material},│
   │         desktop layer, mat, gaps, hdr, sdr-brightness      │
   ▼                                                            │
~/nixos-config/niri/config.kdl ◄── generated layer rules ───────┤ gen-layer-rules.sh splices
                                                                │ `amane-shell layer-rules`
amane fork: Wayland, wgpu quads + vello (vello fork), widgets,  │
   Services, niri client; also KDE Connect, adb, polkit and     │
   Bluetooth agents, the status-item protocol                   │
   ▲                                                            │
amane-kit (kit/): tokens from design-tokens/tokens.json (build.rs),
   controls, containers, look, settings schemas                 │
   ▲                                                            │
amane-shell (src/): surfaces ◄── services       apps/*: settings, drop, messages, mirror,
   ├─ D-Bus: serves org.amane.StatusBar1, polkit agent,         shader-lab, train-watch,
   │         BlueZ agent, rbw pinentry                          assistant, pinentry
   ├─ line IPC: $XDG_RUNTIME_DIR/amane.sock (15 verbs) ◄── apps, as untyped strings
   ├─ settings: ~/.config/amane-shell/*.json (amane Config<T>: watched, atomic)
   ├─ state: $XDG_STATE_HOME/amane-shell/* (three formats, not atomic)
   └─ niri: amane's socket for some calls, `niri msg` subprocesses for others
```

Surfaces are immediate-mode `LayerWindow` builders. Each frame they read services, which records what to redraw on. UI state lives in `thread_local`s. Services start on their first read and never stop.

### The main problems

Details and file references are in the analysis that preceded this roadmap; each phase below names the files it touches.

1. **CI is red and covers too little.** Shell CI has failed on all 30 recent runs: magic-nix-cache gets throttled and FlakeHub refuses access before the toolchain is fetched. Behind that, clippy fails at `kit/src/platform/app_icon.rs:122`. `cargo clippy` and `cargo test` run without `--workspace`. amane's CI doesn't run on `ocbk`. The local `.cargo` override builds against `../amane`'s working tree, not the pinned rev.
2. **Idle cost.** 74 process spawns a minute and 116 wakeups a second with nothing happening, as measured in [docs/development/idle-baseline.md](docs/development/idle-baseline.md): Toasts polls every 100 ms, `nmcli` runs every 3 s, `niri msg` every 2–5 s, `rbw` every 15 s, sysfs every 250 ms to 1 s. `motion::redraw_after` spawns an OS thread per call. In the kit, every token read takes four locks and a string-keyed lookup.
3. **Code in the wrong layer.** About 9.5k lines of product code in amane: KDE Connect, phone messages, adb, the `org.amane.StatusBar1` protocol, UI strings and timings. The shell runs `niri msg`, `nmcli`, `gdbus` and `rfkill` subprocesses for data amane has or should have. Generic UI lives in the shell (context menu, search field, text editing, grid, fade and reveal helpers). The apps have a third niri client and their own process runners. `displays.kdl` has two writers.
4. **Duplication and leftovers.** XDG paths copied 8 times, logind code 3 times, child-stream loops 4 times. The kit's `macform` set has become the real theme, but four of its controls duplicate unused twins. M3 `lists` has no users. There's the `components/mod.rs` shim, a demo context menu in `main.rs`, empty folders, migration shims and a broken example.
5. **Inconsistent patterns.** Errors are swallowed (`let _ =` ×60, `eprintln!` ×31, no logging). There are three process APIs, three state-file formats, and surfaces that reach into each other's internals. Some files are very large: launcher 1,683 lines, dock 1,317, widgets 1,224.
6. **Fork drift.** amane conflicts in 6 files against upstream `main` today. niri is 263 commits behind upstream `main`, with hot spots in `input/mod.rs`, `niri.rs`, `backend/tty.rs` and `layout/*`, and Smithay's Dispatch2 refactor ahead. Both forks' `maintaining.md` understate conflicts. The cross-repo contract is unversioned: on stock niri the shell can't tell which fork features exist.
7. **Docs out of sync.** README layout and features, `layer-rules.md`, `ipc.md`, `quick-settings.md` and CONTRIBUTING's CI claims are out of date. There's no architecture, contract, release or testing doc.
8. **Apple platform terms.** 664 occurrences: shell 621; lines the forks added, amane 18 and niri 25; plus 16 file names and a few nixos-config comments. Every line is listed in `docs/development/macos-references.md` (deleted in Phase 3).

### What to keep

- Penpot tokens compiled into typed functions, with contract and contrast tests.
- The golden layer-rules test and the splice script's sentinel check.
- The `org.amane.StatusBar1` design.
- amane `Config<T>`, and `Latest<T>`.
- The kit's flat `amane_kit::<module>` paths.
- Fork docs kept in `docs/fork/`, with upstream docs untouched and niri's fork-docs parse test.
- Fork features mostly in new files.
- 278 shell tests and 186 kit tests, few `unwrap`s, a `//!` doc on nearly every module.
- The event-driven services to copy: tailscale, updates, vault/pinentry, and `SystemStats` pausing while nothing shows it.

---

## Target architecture

### What goes where

| Layer | Owns | Never contains |
|---|---|---|
| **niri fork** | Compositor rendering and policy: materials, blur shapes, vibrancy, HDR and colour management, the mat, the desktop layer, gaps, the IPC additions, and a `Features` request that says which of these exist. | Names or behaviour specific to amane-shell. |
| **vello fork** | Memory tuning only (scratch sizes, releasing idle buffers). Pinned by `rev`. | Anything else. |
| **amane fork** | A generic shell library: Wayland, rendering, widgets, text, a complete niri client, generic system services (UPower, BlueZ, NetworkManager, logind, rfkill, tray, polkit agent mechanics, clipboard), an event-loop timer, change subscriptions, services that stop when unwatched, and launching apps in their own systemd scope. Code that needs the niri fork sits behind a `niri-fork` feature. | UI strings, display timings, `org.amane.*` interfaces, product features (phone, status apps). |
| **`crates/*`** in the shell repo | Libraries shared by the shell and apps: `amane-kit`, `amane-status`, `amane-phone`, `amane-nixos`, `amane-assistant`, `amane-pinentry`. | Surfaces or windows of their own. |
| **`apps/shell`** | The surfaces and the state only the shell has. | Generic UI, generic system clients. |
| **`apps/*`** | One app each, built on the kit. | Their own niri, IPC or process clients. |

How to decide: if a second program could use it and it has no amane-shell look, it goes in amane. If it has the look, it goes in the kit. If two of our binaries share it but it isn't UI, it gets a crate. Otherwise it stays in the binary.

### Workspace

```
amane-shell/
├── Cargo.toml            virtual workspace: [workspace.package], [workspace.dependencies], [workspace.lints]
├── flake.nix
├── xtask/                cargo xtask layer-rules | tokens check | tokens normalize | lint-terms
├── crates/
│   ├── kit/              amane-kit
│   ├── status/           amane-status: the org.amane.StatusBar1 wire types and client
│   ├── phone/            amane-phone: KDE Connect, phone messages, adb
│   ├── nixos/            amane-nixos: the NixOS model and job pipeline, no UI
│   ├── assistant/        amane-assistant (library; the daemon feature is off by default)
│   └── pinentry/         amane-pinentry
├── apps/
│   ├── shell/            amane-shell
│   ├── settings/  drop/  messages/  mirror/  shader-lab/  train-watch/
│   └── assistant/        the assistant daemon
├── design-tokens/
├── docs/
└── tests/fixtures/
```

### Kit

The flat re-exports in `lib.rs` stay. `macform` is merged in and removed.

```
crates/kit/src/
├── foundation/   color, icons, symbol, layout, paths (XDG config/state/cache/runtime)
├── look/         theme (including the former macform theme), hover, crossfade,
│                 material, glass, glass_region, lab/
├── tokens/       generated tokens, resolve (cached per frame); lint, check and
│                 normalize behind a `tools` feature
├── animation/    motion, reveal (new: open/close), kinetic, pager, edge_reveal
├── controls/     buttons/, slider, dropdown (with the pop-up button), menu (with the
│                 shell's context menu), text_field, search_field, stepper,
│                 color_picker (with color wells), thumbnail_picker, avatar, spinner, tag
├── containers/   app_bar, card, dialog, divider, flow, form, list, grid (from the shell),
│                 scroll_view (with the scrollbar fade), tooltip
└── platform/     app, app_icon, marker, secrets, process, shell_ipc, displays_kdl,
                  settings/{appearance, status_bar, dock, launcher, quick_settings, drop, …}.rs
```

### Shell

```
apps/shell/src/
├── main.rs        startup only: agents, services, the surface list
├── ipc.rs         one verb table that sends each command to its surface (types from kit::shell_ipc)
├── chrome/        what several surfaces share and only the shell has: panel exclusivity, screen edges
├── services/      state only the shell has: launcher index, clipboard history, vault, usage,
│                  status_apps (the D-Bus server), phone (over amane-phone), media art, do-not-disturb
└── surfaces/
    └── <surface>/ mod.rs   surface() and the surface's public API (open, close, ipc)
                   state.rs view.rs input.rs ipc.rs   as the surface needs them
```

### Key internal APIs

- **Services.** Push first: D-Bus signals, netlink or `/dev/rfkill`, inotify or udev, the niri event stream. Poll only sysfs that has no events, and only while something watches. amane gains:
  - `Service::subscribe(callback)`, so services react to other services without polling or doing it in views;
  - `Timer::after(duration)` on the event loop, replacing every sleeping thread;
  - stopping a service when nothing has read it for a while.
- **Surfaces.** A surface's `mod.rs` is its only public face. Data two surfaces share goes in `services/`. Side effects happen in services, never in a view.
- **Status apps.** `amane-status` holds the wire types and the client, used by drop, train-watch and anyone else. `apps/shell/src/services/status_apps.rs` is the server. The bus name and methods stay the same.
- **Shell IPC.** `kit::platform::shell_ipc` defines a typed `ShellCommand`. It writes the same line verbs as today, so old and new clients both work. The shell's `ipc.rs` parses into the same type.
- **Fork detection.** niri answers a `Features` request (fork only), amane exposes it as `Compositor::features()`, and the shell degrades on stock niri instead of misbehaving. `cargo xtask layer-rules` checks material values against the ranges niri accepts.
- **Errors and logging.** The `log` crate, set up once in each binary. amane services report one `ServiceError { kind, message }`; the UI decides how long to show a failure. A program that won't start is logged, never a panic.
- **Persisted data.** Settings use `Config<T>`, as now. State uses one serde helper with atomic writes. Each file has a version field so a migration knows what it's reading.

### Alternatives considered

- **Vendoring amane into the shell, or one repo for everything.** Rejected: it ends cheap rebases from upstream, and the repos are kept separate on purpose.
- **Moving KDE Connect into the shell's own source.** Rejected: messages and mirror use it too, so it becomes a crate.
- **Keeping `macform` as a second look.** Rejected: two themes to keep in step, and the core controls already depend on it.
- **A retained or reactive UI rewrite** instead of fixing amane's services. Rejected: a big-bang change for the same idle gains.
- **A patch-queue tool (quilt, stgit) for the forks.** Rejected: feature-grouped commits on `ocbk`, a `fork-base` tag and a `git range-diff` check after each rebase get most of the benefit with no new tooling.

---

## Phases

Phases 0–3 are low-risk and make everything after them safer. Phase 4 is a single mechanical move, done early so later work lands in the final layout. Phases 5–7 pay down idle cost on a new foundation in amane. Phases 8–11 fix the boundaries. Phases 12–13 make the forks cheap to rebase and finish the docs.

| # | Phase | Size | Changes the cross-repo contract |
|---|---|---|---|
| 0 | Baseline and green CI | S | no |
| 1 | Workspace lints and safety-net tests | M | no |
| 2 | Leftovers and docs that tell the truth | S | no |
| 3 | Apple platform terms, text pass | M | no |
| 4 | Workspace layout | M | build paths only |
| 5 | niri fork: tidy history, `Features`, contract checks | M | **yes**, additive |
| 6 | amane foundations: timer, subscriptions, niri client, system services | L | **yes**, amane API |
| 7 | Shell and kit idle pass | M | no |
| 8 | Kit consolidation | L | kit API |
| 9 | Product code out of amane | L | **yes**, amane API shrinks |
| 10 | Shell internal structure | L | file formats, with migration |
| 11 | Apps cleanup | M | `displays.kdl` ownership |
| 12 | amane fork: restructure and rebase | L | no, if done right |
| 13 | Docs and rustdoc completion | M | no |

---

### Phase 0: Baseline and green CI (S)

**Goal.** A trustworthy signal and numbers to measure against. Nothing later can be judged without them.

**Tasks**
- [shell] Commit or finish the in-flight bar-sizes work, so the roadmap starts from a clean tree.
- [amane] [niri] Push the local commits the shell will need (amane `74b5ddb`, niri `16384e00`), or note they stay local.
- [shell] Fix CI:
  - Drop `magic-nix-cache-action` (throttled, FlakeHub login). Use `cachix/install-nix-action` with the nix-community cache, or no cache.
  - Run `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked`.
  - Fix `kit/src/platform/app_icon.rs:122` with a type alias.
- [amane] Run `test.yml` on pushes to `ocbk` and add `cargo clippy` and `cargo doc --no-deps` steps.
- [niri] Make the existing `ci.yml` run on `ocbk` pushes (build, tests, the fork-docs parse test). Skip the release jobs.
- [shell] [amane] Pin vello by `rev` instead of `branch = "ocbk"`. Remove the duplicate `outputHashes` entry in `flake.nix`, since one entry per git rev is enough.
- [shell] Add `scripts/check-pins`: warns when `../amane`'s `HEAD` isn't the rev in `Cargo.toml`, so a local build that differs from the Nix build is visible. Mention it in CONTRIBUTING.
- [shell] Add `scripts/idle-report`. With the shell running beside the live session, for 60 idle seconds it records:
  - wakeups per second, from the voluntary context switches of all threads;
  - CPU time;
  - PSS, from `smaps_rollup`;
  - child processes started, via `bpftrace` on `sched_process_exec` or `strace -f -e execve`.

  Save the first report as `docs/development/idle-baseline.md`.

**Acceptance**
- CI is green on shell `main`, amane `ocbk` and niri `ocbk`.
- `nix build` works and the shell switches and runs as before.
- The idle baseline is written down.

**Risks.** Low. A cache change can slow CI; acceptable.

**Status (2026-11-07).** Done. The idle baseline is in [docs/development/idle-baseline.md](docs/development/idle-baseline.md): 116 wakeups a second, 74 programs started a minute (40 `nmcli`, 30 `niri`, 4 `rbw`), 72 MiB PSS.
- **Clippy** over the whole workspace found errors in the apps too; all are fixed.
- **Shell CI** is green.
- **amane** has its CI in a new `.github/workflows/fork.yml`, so upstream's `test.yml` is untouched. Upstream's image filters trip `needless_range_loop`, so CI allows that one lint.
- **niri** couldn't use upstream's `ci.yml`: the fork's HDR output needs libdisplay-info 0.2, which Ubuntu, Fedora and FreeBSD packages don't have. Its CI is `fork.yml` too, run in the flake's dev shell and skipping the EGL tests as the flake's own build does. Upstream's `ci.yml` should be turned off on the fork in the Actions settings.
- **Idle numbers.** `scripts/idle-report` gives exact numbers only under `sudo`. The shell's memory maps and its children's `exe` links are closed to the user, so without root it reports RSS instead of PSS, and its count of programs started is a lower bound.

---

### Phase 1: Workspace lints and safety-net tests (M)

**Goal.** Catch regressions in what the big refactors will touch: token generation, the layer rules, the service parsers and the IPC verbs. This has to be in place before code starts moving.

**Tasks**
- [shell] Add `[workspace.package]` (version, edition, rust-version, license, repository, authors) and inherit it in all 10 manifests.
- [shell] Move shared deps into `[workspace.dependencies]`: `serde_json` (declared 9 times, as both `"1"` and `"1.0"`), `chrono`, `libc`, `libpulse-*`, `zbus`, `amane-kit`.
- [shell] Add `[workspace.lints]`:
  - rust: `unsafe_op_in_unsafe_fn`, `unused_must_use = deny`;
  - clippy: `all = warn` plus a short list (`dbg_macro`, `todo`, `unwrap_used` in non-test code, `let_underscore_must_use`).
  - Turn on `missing_docs = warn` for amane-kit, with an `#[allow]` on each module still missing docs, so the list of allows is the backlog.
- [amane] Add a `[lints]` table matching the shell's, scoped so upstream's files aren't flagged. That means `#[allow]` at the root of upstream modules; fork modules opt in.
- [shell] Token pipeline tests:
  - unit tests for `build.rs`'s `emit_module` and `ident`, by moving them into a module both `build.rs` and the tests include;
  - make the `report()` test in `kit/src/tokens/rules.rs:803` assert instead of only printing;
  - a test that every generated material value lies within the range niri accepts, with the ranges copied from niri-config into a fixture.
- [shell] Fixture tests for the parsers the idle phases will replace. Feed recorded output into the current parsing code, so the replacement must produce the same model:
  - `nmcli` output;
  - `niri msg --json` outputs, casts and windows;
  - `/sys/class/rfkill` and LED listings;
  - `pw-dump`.

  Keep them next to the existing `tests/fixtures/{pipewire,tailscale}`.
- [shell] A test that every IPC verb registered in `main.rs` has a row in `docs/integration/ipc.md`, and the reverse.
- [shell] A test that the `layer_rules.rs` namespace list covers every `LayerWindow` namespace the surfaces create, including `amane.fill`, which has no rule today.

**Acceptance**
- `cargo clippy --workspace -- -D warnings` and `cargo test --workspace` pass in CI.
- `missing_docs` warns in the kit.
- The new tests fail when their subject is deliberately broken (try once by hand).

**Risks.** Low. The clippy list may surface many warnings; fix them or allow them per module, never globally.

**Status (2026-11-07).** Done: shell `main` pins amane `db47c38`.
- **Lints.**
  - `let_underscore_must_use` was left out on purpose: 144 hits, nearly all deliberate discards (`let _ = writeln!` into a `String`, cleanup removes, a child's `kill`). Instead, the writers that save settings, secrets and the Drop Bar's state now log a failed save.
  - Only two unwraps outside tests needed fixing.
- **Missing docs.** `missing_docs` is on in the kit, and 69 files start with `#![allow(missing_docs)]`. That list is Phase 13's backlog.
- **amane** doesn't put allows at the root of upstream modules. 14 of its top-level modules mix upstream and fork code, so those allows would conflict on rebases. Instead:
  - `Cargo.toml` holds only the lints the whole crate passes;
  - every module file the fork added opts into `unwrap_used`, `dbg_macro` and `todo` at its top;
  - see amane's `docs/fork/maintaining.md`.
- **Token tier budgets.** The manual's budgets (sys about 70, component about 15) are far below today's counts (133 and 163). The test caps each tier at today's size, so a tier can shrink but not grow, until Phase 8 brings them down.
- **Fixtures.** They live in `tests/fixtures/{nmcli,niri,sysfs}`, with made-up names. The niri material ranges are copied from niri-config at `72d679c5`, into `tests/fixtures/niri/material-ranges.json`.
- **`amane.fill`** is an input-method popup, not a layer surface, so it needs no rule. It's in the namespace test's `POPUPS` list.

---

### Phase 2: Leftovers and docs that tell the truth (S)

**Goal.** Remove what misleads before refactoring, so later diffs don't carry dead weight. Cheap, and a good warm-up.

**Tasks**
- [shell] Remove the demo context menu built in `src/main.rs`, and the empty `src/components/controls/` and `src/components/sidebar/` folders.
- [shell] Register `alert-test` only in debug builds.
- [shell] Fix `examples/liquid_glass.rs:248` and its doc comment to point at `kit/src/look/glass.wgsl`.
- [shell] Narrow the module-wide `#![allow(dead_code)]` in `src/surfaces/context_menu/mod.rs:5` and `kit/src/controls/buttons/mod.rs:24`. Delete what's actually dead.
- [shell] Fix the docs found stale:
  - README: replace the "Layout" block with a link to `docs/README.md`'s source-layout table, and describe the bar as it is now (per monitor, top or bottom, three sizes, four styles).
  - `docs/integration/layer-rules.md`: `APP_IDS`, blur shapes instead of blur regions, the `amane.dock` and `amane.fill` rows, the `amane-mirror` window rule, the bar's styles.
  - `docs/integration/ipc.md`: the `passwords` verb, and the `tray`, `weather` and `keyboard` icons.
  - `docs/development/performance.md:72`: the `Indicators` marker is gone.
  - `docs/shell/quick-settings.md:291`: only `Card` and `Round` tile styles exist.
  - CONTRIBUTING: what CI runs; the vello patch and its hash when bumping revs.
  - `kit/README.md` and `docs/README.md`: list `lab.md`, `color-picker.md`, `lists-m3.md`, `apps/pinentry`, `apps/train-watch`.
  - `kit/build.rs:8`: the parser now lives in the kit.
  - `kit/src/controls/macform/mod.rs:4` and `kit/src/lib.rs:18`: no longer experimental.
- [amane] `docs/fork/maintaining.md`:
  - say what a rebase really conflicts on;
  - fill in the commit map (KDE Connect, tray, status items, polkit, Bluetooth agent, adb, video, input method and secure fill, MimeApps, HDR brightness, colour emoji);
  - sync `origin/main` with `upstream/main`, or drop the claim that it's kept in sync;
  - delete the `power` branch the doc says can go.
- [amane] Fix `docs/fork/system/polkit.md:50`: the prompt now lives in `shell/src/surfaces/alert/polkit.rs`.
- [niri] `docs/fork/README.md` and `maintaining.md`: add `Window.is_fullscreen`, the secure-fill commit and its protocol XML, and the missing commits (`b0ffbd0d`, `b25a5a8d`, `c7a113d6`, `5adb24b8`, `16384e00`).
- [shell] Move the active plans in `~/Projects/desktop/` into the repos:
  - `idle-ram-plan.md` goes to `docs/development/` and merges with `memory.md`'s "Planned" section;
  - `shell-feature-suggestions.md` goes to `docs/development/backlog.md`.

  Leave the finished research notes where they are.

**Acceptance**
- Every doc claim the review flagged is fixed.
- The Phase 1 IPC-doc test passes.
- The shell runs without the demo menu.

**Risks.** None to behaviour.

**Status (2026-11-07).** Done. Shell `main` still pins amane `db47c38`, since amane's change was docs only (`77b63cc`); niri `ac07c307` is docs only too.
- **Demo menu and `alert-test`.** The demo menu wasn't removed outright. It was the only named menu, and `arm` and `open` can't go under the IPC rule. So the menu and `alert-test` are now registered only in debug builds, and in a release build `arm`/`open` answer `error: no menu <name>`. The niri bind that armed the menu (Mod+Shift+M) is gone from nixos-config.
- **Dead code.** Neither module-wide `dead_code` allow hid anything dead, so both are simply gone.
- **amane branches.** amane's `main` now mirrors upstream's (`544cbfc`). Deleting the merged `power` and `origin/gesture-services` branches is left to do by hand.
- **Plans moved.** The idle plan's open items are in `docs/development/memory.md` under Planned. The feature survey is `docs/development/backlog.md`, with the four shipped items marked.

---

### Phase 3: Apple platform terms, text pass (M)

**Goal.** Remove every mention of macOS and other Apple platform terms from text that is free to change: comments, docs, descriptions, keywords, search aliases, user-facing strings. Done early because it's independent and touches many files that later phases move. Identifier renames wait for Phase 8, where `macform` is merged anyway.

The list to work from is `docs/development/macos-references.md` (deleted in Phase 3). Terms covered:
- macOS and its version names, Mac;
- iOS, iPad, iPhone, Apple;
- Spotlight, Control Center, Launchpad, AirDrop, Finder;
- Liquid Glass, SF Pro, SF Symbols;
- AppKit, Aqua, `NSStatusItem`, Live Activities "as in macOS";
- Dropzone and Aptonic.

**Tasks**
- [shell] Rewrite every occurrence outside identifiers, describing the feature on its own terms. For example:
  - "Launcher", not "macOS Tahoe's Spotlight";
  - "Quick Settings" and "the Quick Settings button", not "Control Center";
  - "a drop shelf: a grid of folders, apps and actions to drop files on", not "Dropzone-style";
  - "glass", not "Liquid Glass";
  - "the bar's symbol set", not "SF Symbols".
- [shell] Touch these points too:
  - `Cargo.toml` descriptions (root, drop);
  - `apps/drop/amane-drop.desktop` keywords;
  - the Shell Settings search aliases (`apps/settings/src/search.rs`, e.g. `"control center"`, `"dropzone"`);
  - the assistant's training phrases in `apps/assistant/train/gen.py`;
  - `niri-layer-rule.kdl` comments, and through it the generated region in the niri config;
  - user-facing strings such as "About".
- [shell] Rename `docs/archive/spec-macos27-variables.md` (a neutral name like `spec-system-colour-variables.md`) and reword its contents; it's archived, but still committed text.
- [shell] Replace the macOS peer in `tests/fixtures/tailscale/status.json` with another OS value. Check that the test still covers what it covered.
- [shell] Design tokens: change the `"SF Pro Text"` font family in Penpot (Inter, or the font actually installed). Re-export `design-tokens/tokens.json`. Update `kit/src/tokens/font.rs:105` and `values.rs:591` tests to the new family.
- [amane] Reword the 18 fork-added lines, including:
  - `status_item.rs` and `status_item/{client,protocol}.rs` ("a program's item in the bar", "a Live Activity: something going on now"; no `NSStatusItem`, no "macOS 27.3");
  - the "Apple's vibrant labels" comments in `window.rs`, `popup.rs`, `layer_window.rs`, `graphics/vibrancy.rs`;
  - the scrolling comment in `scroll_area/offsets.rs`;
  - `docs/fork/**`;
  - `protocols/niri-vibrancy-v1.xml`.

  Keep the XML byte-identical with niri's (change both together).
- [niri] Reword the 25 fork-added lines:
  - `docs/fork/**`, including "the recovered macOS 26" in `effects/materials.md:119`;
  - `niri-config/src/appearance.rs`;
  - `src/render_helpers/shaders/glass.frag`;
  - `resources/niri-vibrancy-v1.xml`.

  Put the reword into the feature commits in Phase 5's history tidy, not a separate commit.
- [nixos-config] Reword the comments in `home/home.nix` (icons, drop, assistant) and the hand-written part of `niri/config.kdl` (the assistant bind).
- [shell] Add `cargo xtask lint-terms` (or a script until Phase 4) that greps for the term list. It runs in CI in all three repos and allows only:
  - `docs/development/macos-references.md`;
  - this roadmap, which has to name the terms it removes (reworded in Phase 13);
  - the `macform` identifiers still waiting for Phase 8;
  - upstream text in the forks, judged by `git blame` against the fork base.

  Delete the references file when the list is empty.

**Acceptance**
- `lint-terms` passes in CI.
- The only remaining hits are `macform` identifiers (Phase 8) and upstream text in the forks.
- Shell Settings search still finds the pages by their new words.

**Risks.** Low. Search aliases and the assistant's phrases are behaviour: re-run the assistant's tests, and check search by hand. The Penpot font change alters text metrics; look at the bar, dock and Shell Settings after the switch.

**Status (2026-11-07).** Done. Shell `main` pins amane `292d486`; niri `ocbk` carries the reword as `fixup!` commits.
- **The lint.** `scripts/lint-terms` runs in the shell's CI on every tracked file. In each fork's CI it runs with `--diff fork-base` (amane) or `--diff v26.04` (niri), so it checks only the lines the fork added.
  - What it lets through is listed in `scripts/lint-terms.allow`: `ROADMAP.md`, `Cargo.lock`, and the identifiers left for Phase 8 (`macform`, the `mac::` alias, `show_in_spotlight`, the `spotlight` page id and widget ids).
  - Lines kept for compatibility carry a `lint-terms: allow` marker, such as Shell Settings still accepting `spotlight` as a page name.
  - `docs/development/macos-references.md` is deleted.
- **Names.**
  - Live Activities are **Activities**.
  - Shell Settings' Spotlight page is the **Launcher** page, and the launcher's placeholder is "Search".
  - Drop is "a drop shelf".
- **The font** was already Atkinson Hyperlegible Next in `tokens.json`. Only two tests still named the old one.
- **niri.** The reword is in 12 `fixup!` commits on `ocbk` (11 for text, 1 for the CI lint step), which Phase 5's autosquash folds into their feature commits. Two of the commits they fix have Apple terms in their subjects (`ada326f0` and `78620a8c`), so Phase 5 should reword those subjects as it squashes.
- **The assistant's training phrases** are reworded; the shipped model is unchanged until the next retrain.

---

### Phase 4: Workspace layout (M)

**Goal.** Move to the target layout in one purely mechanical step, before the refactors. Doing it early means each later change lands once, in its final place.

**Tasks**
- [shell] Turn the root `Cargo.toml` into a virtual workspace. Move with `git mv`, so history follows:
  - `src/` → `apps/shell/src/`;
  - `kit/` → `crates/kit/`;
  - `apps/pinentry` → `crates/pinentry`;
  - the library half of `apps/assistant` → `crates/assistant`. The daemon stays in `apps/assistant`, and its `daemon` feature is no longer a default, so libpulse isn't built for the shell.
- [shell] Add `xtask/` with `layer-rules`, `tokens check`, `tokens normalize` and `lint-terms`. Keep the `amane-shell layer-rules` and `amane-shell tokens …` subcommands for this one phase, printing a note that points to `cargo xtask`. Remove them in Phase 10.
- [shell] Update:
  - `flake.nix`: `src` filters, `cargoTestFlags`, binary names and installed files;
  - `gen-layer-rules.sh`: calls `cargo xtask layer-rules`;
  - `.github/workflows/ci.yml`;
  - the `.cargo/config.toml` override instructions;
  - every doc path, with a `rg` pass and the Phase 1 link checks.
- [shell] `kit/build.rs` stops embedding `apps/shader-lab/{effects,configs}`. shader-lab embeds its own data, and the kit's lab module takes it as input. Same for `include_str!` paths that climb out of the crate: the kit must build on its own.
- [nixos-config] Update any path that points into the shell repo (the flake package outputs, the gen script), then bump the shell input.

**Acceptance**
- `nix build` produces the same set of binaries.
- `cargo xtask layer-rules` output is byte-identical to the committed `niri-layer-rule.kdl` (the golden test).
- `cargo test --workspace` passes.
- The switch works and the live session looks unchanged.
- `cargo package -p amane-kit --no-verify` lists no files outside the crate.

**Risks.** Medium, all mechanical: Nix `src` filters missing a moved file, and a stale path in a doc. In-flight branches conflict on every file, so land any open work first.

**Status (2026-11-07).** Done. Shell `main` pins amane `112da4d`. The moves are one commit of renames, so `git log --follow` works.
- **The design tokens** moved into the kit (`crates/kit/design-tokens/`), so the kit packages with nothing from outside it.
- **Shader Lab's data.** Shader Lab embeds its effects and saved looks in its own `build.rs`, and hands them to `amane_kit::lab::register`. That's a smaller change than passing the files to every lab function. Its tests and the `lab_look` example moved with the data.
- **Token tests.** The two that read the shell's code (tokens read, and components reading only `sys`) moved into the shell (`apps/shell/src/tokens/contract.rs`).
- **The assistant's daemon** is the package `amane-assistant-daemon`, and its binary is still `amane-assistant`. The library has no features and no libpulse.
- **Workspace defaults.** `default-members = ["apps/shell"]`, so `cargo run` still means the shell.
- **The cargo config.** `.cargo/config.toml` is tracked and holds only the `xtask` alias. A local amane override goes in the ignored `.cargo/local.toml`, used with `cargo --config .cargo/local.toml`; `check-pins` reads it there.
- **The link check.** The "Phase 1 link checks" this phase leaned on didn't exist. `cargo xtask links` is new, runs in CI, and the move left no broken link.

---

### Phase 5: niri fork: tidy history, `Features`, contract checks (M)

**Goal.**
- Make the next niri release rebase cheap.
- Make the shell's dependence on the fork explicit, so stock niri degrades instead of misbehaving.

This comes before the amane work because amane's new niri client (Phase 6) uses `Features`.

**Tasks**
- [niri] Rewrite `ocbk`'s history into feature groups on the same base (`v26.04`), with the same final tree apart from the Phase 3 reword:
  - squash the fixups (`9d5bf9fc` into `59fe93e4`, `c7a113d6` into `b25a5a8d`, `b0ffbd0d` into `cf559b95`);
  - fold every `docs(fork)` commit into the feature it documents;
  - drop the 12 commits that edit `docs/wiki/*` together with their revert in `7f48af01`, since the net change is none;
  - group the materials commits;
  - order the groups from least to most invasive: IPC additions, `MoveColumnToIndex`, `is_fullscreen`, gaps, desktop layer, mat, mod swipe, blur shapes and materials, vibrancy, secure fill, HDR.

  Verify with `git diff old-ocbk new-ocbk` (only the reword) and `git range-diff`. Keep the old branch as `ocbk-pre-tidy` until a switch has run on the new one.
- [niri] Reduce hunks in hot upstream files by moving logic into fork files and leaving one-line hooks:
  - `backend/tty.rs` (+410 lines in 26 hunks for HDR): into `src/backend/tty_hdr.rs`;
  - `niri.rs`: the effect and desktop-obscured plumbing into a `fork` module;
  - the effect rework in `render_helpers/{framebuffer_effect,background_effect,xray}.rs`, where it can be separated without changing behaviour.

  Measure with `git diff v26.04 --stat` on those files before and after.
- [niri] Add `Request::Features`, answered with a list of fork feature names and versions (`blur-shapes 2`, `vibrancy 1`, `secure-fill 1`, `materials`, `mat`, `desktop-layer`, `hdr`, `move-column-to-index-window-id`, `window-fullscreen`, `overview-progress`, `view-offset`, `workspace-switch-progress`, `desktop-obscured`). Stock niri answers it with an error, which is the detection.
- [niri] Fix `DesktopObscured` keeping outputs that were unplugged (it replays them to new clients). Add tests for event emission and state replay of the fork's IPC events.
- [niri] `docs/fork/maintaining.md`: write the real rebase routine:
  1. tag `fork-base-<tag>`;
  2. `git rebase --onto <new tag>`;
  3. `git range-diff`;
  4. build, tests, fork-docs parse test;
  5. a live session.

  Add a contract section listing what amane-shell relies on.
- [niri] Prepare for the next tag: write in `maintaining.md` how each fork protocol (`blur_shapes`, `vibrancy`, `secure_fill`, `color_management`) will be ported to Smithay's Dispatch2. Do the port when the tag that needs it arrives.
- [shell] `cargo xtask contract`, also in CI: checks that the protocol XMLs in amane and niri are byte-identical at the pinned revs, and that the niri IPC variants amane parses exist. Runs from checkouts (`../amane`, `../niri`), or from flake inputs in CI.
- [shell] `docs/integration/contract.md`: every niri, amane and kit interface the shell relies on, with the minimum fork revision for each. It becomes the place to look before a rebase.
- [nixos-config] Point at the rewritten niri revision; re-login.

**Acceptance**
- The rewritten branch has the same tree as before, apart from the reword.
- Each feature is one commit group.
- Hunk counts in `tty.rs` and `niri.rs` are lower, and written in `maintaining.md`.
- `niri msg` sends `Features` and gets the list.
- `xtask contract` passes.
- The session runs on the new build, with HDR, glass and the mat checked by eye.

**Risks.** Medium. A history rewrite on a branch the live system builds from: keep `ocbk-pre-tidy`, and switch back if anything looks different. Contract: `Features` is additive.

**Status (2026-11-07).** Done in code; the session check by eye is the user's. niri `ocbk` is `745e72d5`, 43 commits on `v26.04` (from 67), and nixos-config builds it (`dc96e86`).
- **History: folded, not reordered.** Fixups squashed, each feature's docs folded into it, the rustfmt commit split into its features, the `docs/wiki` edits and their reset dropped, the vendor terms reworded out of three subjects and their bodies. Commits keep their order, since a full reorder meant hand-resolving conflicts for no change in the tree. The tree equals `ocbk-pre-tidy` apart from one comment, and every commit builds.
- **Hunks.** `tty.rs` 36 → 30 hunks, 374 → 119 lines added (HDR in `src/backend/tty/hdr.rs`); `niri.rs` 38 → 36, 163 → 101 (`src/niri/fork.rs`). The render helpers stay: they're mostly fork additions already.
- **`Features`** lists the fork's features with contract versions; the protocol versions come from the globals' constants. `niri msg features` prints it.
- **`contract.toml`** holds the niri revision the shell is checked against, since the shell's flake doesn't pin niri; `cargo xtask contract` also checks it against nixos-config's lock.
- **Dispatch2.** The porting note in niri's `maintaining.md` lists what each protocol dispatches; the port waits for the tag that needs it.

---

### Phase 6: amane foundations: timer, subscriptions, niri client, system services (L)

**Goal.** Give the shell what it needs to stop polling and stop starting programs. This is the root of the idle cost: amane has no timer on its event loop, no change subscription, and services that never stop.

**Tasks**
- [amane] `Timer::after(duration)` and `Timer::at(instant)` on the event loop (a timerfd in the calloop loop), redrawing or calling back on the UI thread. Tests without a compositor.
- [amane] `Service::subscribe(callback)`: called after a write that changed the value, so one service can follow another without polling and without side effects in views.
- [amane] Stop a service's worker when nothing has read it for a configurable time. Restart on the next read. Polling services poll only while watched.
- [amane] Complete the niri client (`compositor/niri.rs`) and make it public:
  - outputs (already queried privately);
  - windows with `pid`;
  - casts;
  - keyboard layouts, from the existing event stream rather than a second one;
  - focused window;
  - `Features` (Phase 5).

  Gate the fork-only fields behind a `niri-fork` feature, on by default in the shell.
- [amane] Generic services:
  - `Rfkill`, from `/dev/rfkill` events; the Bluetooth service's rfkill writes move onto it;
  - `NetworkManager` connections over D-Bus (wired, VPN, hotspot), extending `Network`;
  - `Logind` (session, lock, sleep signals, inhibitors);
  - a data-control `ClipboardWatcher`;
  - `Leds` and `Backlight` from udev, or inotify on sysfs.
- [amane] Brightness: react to udev or inotify instead of a 500 ms sysfs read and a 2 s IPC call. MimeApps: `watch_file` instead of re-parsing every 3 s. Bluetooth and KDE Connect: drop or lengthen the 30 s resync.
- [amane] Launching:
  - one `launch(argv)` that never panics (today `spawn` panics through `expect`);
  - it passes the activation token;
  - it puts the program in its own transient systemd scope (`StartTransientUnit` over D-Bus), so restarting the shell no longer kills apps started from the dock or launcher.

  Keep `spawn(&str)` as a thin wrapper until Phase 10, then remove it.
- [amane] Rustdoc (`///`) on every public item added in this phase, and a `docs/fork/` page for each new service.
- [shell] Bump the amane rev and `outputHashes`, with no shell code changes yet.

**Acceptance**
- amane's tests pass in its CI.
- Each new service has fixture tests.
- With the new rev the shell behaves exactly as before; nothing uses the new APIs yet.
- An app started from the dock survives `systemctl --user restart amane-shell`.

**Risks.** High, in amane's core loop: a timer or subscription bug freezes redraws. Land behind tests, try the shell for a day before Phase 7. Contract: amane's API grows; nothing removed.

**Status (2026-11-07).** Done, in two steps. 6a: amane `af52270`. 6b: amane `808b695` (services `9d953dc`, `Outputs` with Phase 7) and niri `f8c4e94e` (`989d475e`: the `KeyboardLockKeysChanged` and `OutputsChanged` events, features `lock-keys 1` and `outputs-changed 1`).
- **6b in place of the list above:** `Leds` became `LockKeys` (from niri's event; the sysfs LEDs only on stock niri) and `Backlight` folded into `Brightness`, which reads the kernel's uevents (`services/uevent.rs`) and niri's `OutputsChanged` instead of polling. The keyboard light stays a shell service polled once a second: an LED sends no event, and this ThinkPad's has no `brightness_hw_changed`. `Outputs` (niri's outputs, read again on `OutputsChanged`) was added for the refresh rate on battery.
- **`ClipboardWatcher`** uses `ext-data-control-v1` only; niri has it, so the wlr fallback wasn't written.
- **NetworkManager bursts** settle on the listener's own channel (250 ms quiet, a second at most) rather than a `Timer`.
- **No `niri-fork` cargo feature.** The fork's parts check `amane::niri::features()` at run time; one build runs on stock niri and the fork.
- **Watched, not just read.** An idle bar draws no frames, so reads alone would stop a service the bar shows. A service is watched while an open window's last frame read it, or something read it within its idle time. `Brightness` keeps polling: the OSD reads it once per key press.
- **Subscriptions run on their own thread,** one at a time, so a callback never runs inside a writer's lock, and a burst of writes costs one call.
- **One niri stream**: a late listener gets the state so far replayed, as niri does for a new connection. It doesn't reconnect: niri closing the stream ends the session.
- **Scopes** are `app-amane-<app>-<pid>.scope` under `app.slice`, named for the desktop id when there is one.
- **The internal timers** stay calloop timers; the sleeps inside Bluetooth's D-Bus sequences stay on the worker thread.

---

### Phase 7: Shell and kit idle pass (M)

**Goal.** Use Phase 6 to bring idle cost down to near zero. This is the most visible improvement to the daily machine.

**Tasks**
- [shell] Replace polling and process starts:

  | Today | After |
  |---|---|
  | Toasts every 100 ms | `Timer::at(next expiry)` |
  | Clock every 1 s | a timer to the next minute (seconds only while a seconds clock shows) |
  | Connections: 2 × `nmcli` every 3 s | amane NetworkManager |
  | Privacy: `niri msg casts` every 2 s | amane niri casts (event-driven if niri reports casts as events; else poll only while the indicator is wanted) |
  | `power_refresh`: `niri msg outputs` every 5 s | amane outputs + `subscribe` to power |
  | `hdr_white` every 1 s | `subscribe` to outputs and brightness |
  | `bt_battery` every 60 s | `subscribe` to Bluetooth |
  | `LockKeys` 250 ms, `KeyboardBacklight` 500 ms | amane `Leds` / `Backlight` |
  | `Rfkill` every 1 s + the `rfkill` CLI | amane `Rfkill` |
  | Vault: `rbw unlocked` every 15 s | check when a fill or the Passwords page opens, and on the agent's lock timeout; no background poll |
  | NightLight `try_wait` every 5 s | a thread waiting on the child |
  | Weather every 5 s settings check | `subscribe` to the settings |
  | `system_stats` re-reading `/proc` | amane `Cpu` / `Memory` |
  | `gdbus` in `background_apps.rs`, `file_search.rs` | amane `Bus` |
  | `niri msg` in `status_bar/chips.rs`, `clipboard_history.rs`, `launcher/entries.rs`, `keyboard_layout.rs`, `session.rs`, `keep_awake.rs` | amane niri client |
  | `wl-paste --watch` | amane `ClipboardWatcher` |

- [shell] Replace `motion::redraw_after` (a thread per call, 22 call sites) and the sleeping threads in `activities.rs`, `timer.rs` and `keep_awake.rs` with `Timer`.
- [shell] Move UI-thread IO out of views:
  - the `/proc/*/cmdline` scan and the `/sys/class/video4linux` read in `phone_screen.rs`;
  - the art-cache `stat` in `media.rs`;
  - `remember_status_app` re-reading JSON on every `UpdateItem`;
  - `text_edit::paste` running `wl-paste` in `on_key`.
- [shell] Move `alert::surface`'s `sync()` calls into services subscribed to their sources, so the hidden alert window stops redrawing on every Bluetooth or KDE Connect change.
- [shell] Kit:
  - cache the resolved tokens in a thread-local, replaced when appearance settings change. Better still, have `build.rs` emit an index per token, so a read is a vector index;
  - cache font-stack picks per token;
  - cache `wrapped_lines` by text, style and width;
  - prune `buttons::STATES` of ids not seen for a while;
  - make `app_icon::entry_for` look up without cloning;
  - write the shared `glass.wgsl` atomically.
- [shell] Shell Settings: read the "seen" lists (`activities_seen`, `tray_seen`, `status_apps_seen`) through `Config<T>` instead of parsing JSON on every rebuild.
- [shell] Re-run `scripts/idle-report` and record the result next to the baseline.

**Acceptance**
- Idle for 60 s with no windows open, `idle-report` shows **no child processes started**, **under 2 wakeups a second**, and lower CPU time than the baseline. PSS is no higher.
- Toasts expire on time, the clock turns on the minute, the network, VPN and privacy indicators react within a second.
- Fixture tests from Phase 1 pass with the new sources.

**Risks.** Medium: missed updates where a poll used to paper over a missing signal. The fixture tests and a day of daily use catch most. No contract changes.

**Status (2026-11-07).** Done in code (shell `2f959b4`, docs `68ec7c7`); the `idle-report` after the switch is the user's, against the baseline.
- **Every row of the table** follows an amane service, a subscription or its own deadline; the shell starts no `nmcli`, `niri msg`, `rfkill`, `gdbus` or `wl-paste` of its own. The one clock left while idle is the keyboard light, once a second.
- **Waits:** a small `services::Wake` (a deadline or a nudge, no ticks) serves toasts, Do Not Disturb, the clock (which also wakes on `Logind`'s wake-ups), weather, Keep Awake, power refresh, HDR white and Bluetooth batteries.
- **The vault** is looked at after each command, when a fill or the launcher's passwords want it, and once the agent's lock timeout could have run out; an outside `rbw lock` shows at the next look.
- **Found along the way:** the bar's paused Now Playing spawned a `redraw_after` thread every frame; `text_edit::paste` could deadlock after the vault copied a password (the shell owned the selection that `wl-paste` waited on), now an asynchronous `amane::Clipboard::paste`; reads from background threads kept services watched, now subscriptions.
- **Kit tokens:** the resolved table is cached per thread until the appearance changes (subscriptions bump a generation and redraw every window); `build.rs` still emits functions that look up by name, so the per-token index is left for later.
- **Launching:** `amane::spawn` and `spawn_activated` already put each program in its own scope (`Launch`), so the call sites stayed.
- `scripts/idle-report` now counts threads that end inside the window (looked at every 0.2 s).

---

### Phase 8: Kit consolidation (L)

**Goal.** One implementation of each control, in the kit, with one theme. It comes after the idle pass so the moved code is already event-driven, and before the shell's internal restructure so the shell imports the final kit.

**Tasks**
- [shell] Merge `macform` into the kit proper, and with it remove the last Apple platform identifiers:
  - `macform::theme` → `look::theme`, with its literal colours becoming tokens or a `TODO(token)` each;
  - `hover_fill` and `crossfade` → `look`;
  - `symbol` → `foundation::symbol`;
  - `menu` → `controls::menu`;
  - `MacLeadingIcon` → `controls::leading_icon`;
  - `MacStepper`, `ThumbnailPicker` and `ColorWells` → ordinary controls (`stepper`, `thumbnail_picker`, wells inside `color_picker`);
  - `MacPushButton`, `MacSegmentedNav` and `MacPopUpButton`, one call site each → fold into `PushButton`, `SegmentedButton` and `Dropdown`, then delete;
  - delete `MacSwitch`, `MacSlider`, `MacComboBox` and `MacTextField`, which have no users;
  - rename every `mac::` alias in the apps (about 109 sites), and the example `kit/examples/macform.rs` to `controls.rs`.
- [shell] Choose one list row: `list::ListItem` (Quick Settings), `lists::ListItem` (M3, no users) and `form::FormRow` (Shell Settings). Either migrate Quick Settings' rows to `lists` and delete `list`, or delete `lists` and its example. **Decide at the start of the phase.** The M3 module was built as a showcase, so keep it only if Quick Settings will use it.
- [shell] Move generic UI from the shell into the kit:
  - `surfaces/context_menu` → `controls::menu`, one menu for the shell and apps;
  - `components/search_field.rs` → `controls::search_field`; Shell Settings drops its own `search_field()`;
  - `components/text_edit.rs` → rebuilt on amane's text field, and on `amane::Clipboard` instead of `wl-copy`/`wl-paste`;
  - `components/grid/` → `containers::grid`;
  - the scrollbar fade (launcher, Quick Settings customize) → `scroll_view`;
  - the `unfold(open)` animation (alert, launcher, Quick Settings) → `animation::reveal`;
  - `services/process.rs` (`run`, `run_now`, `Latest`) → `platform::process`, built on amane's `launch`;
  - the 8 XDG path copies → `foundation::paths`.
- [shell] Remove the `src/components/mod.rs` re-export shim. Point the roughly 200 imports at `amane_kit::…` directly.
- [shell] Split `platform/settings.rs` (1,792 lines) into `platform/settings/<schema>.rs`. Move UI helpers out of it: `bar_item_icon` to the status-bar code, the "seen" caches to their own module. File names and formats stay.
- [shell] The token tools (`lint`, `check`, `normalize`) go behind a `tools` feature used by `xtask`; rename `tokens::rules` to `tokens::lint` so it isn't confused with the layer rules.
- [shell] `missing_docs`: remove the per-module allows for every module touched in this phase.

**Acceptance**
- `lint-terms` passes with no exceptions left for `macform`.
- Each control has one implementation; `rg 'macform|Mac[A-Z]'` finds nothing.
- The shell and all apps look the same in a side-by-side check of the bar, Quick Settings, launcher, Shell Settings, Messages and Drop.
- Every moved module has rustdoc on its public items.

**Risks.** Medium. Visual regressions while folding controls: compare screenshots before and after. Kit API breaks only inside this workspace. No cross-repo contract change.

**Status (2026-11-07).** Done, but for the one menu, which moves to Phase 10 (shell `1c21202`…`1d13689`). The counts were 63 `mac::` uses (not 109), about 275 imports through the shim (not 200), and about 25 XDG copies (not 8).
- **Lists:** the M3 `lists` module and its example are gone; Quick Settings keeps `list`, Settings `form`.
- **`macform`:** `look::style` (not `look::theme`, which would clash with the kit's root `theme`), `look::hover`, `controls::parts`, `foundation::symbol`, and the controls as ordinary kit modules. `MacSegmentedNav` and `MacPopUpButton` became `NavButtons` and `PopUpButton` rather than folding into `SegmentedButton` and `Dropdown`: they're different controls (a back/forward pair; an icon-only menu button), and folding them would change how Settings looks. Shell Settings' pages drew pixel for pixel the same before and after.
- **One menu: moved to Phase 10.** The shell's context menu is a layer surface with submenus and its own item type used by 15 callers; the kit's is an xdg popup. Giving them one model and two hosts is a rewrite of the shell's menu, which belongs with the shell's restructure.
- **Moved into the kit:** `search_field`, `text_edit` (on `amane::Clipboard`, kept as the `Field` model rather than rebuilt on amane's text field), `grid`, `process`, `scroll_view::ScrollbarFade`, `reveal::Reveal` and `paths`. The launcher's header and Shell Settings' sidebar still draw their own search fields, which look different from `SearchField`; unifying them is a design change. `train-watch`, `amane-assistant` and `amane-pinentry` don't depend on the kit and keep their own XDG lookups.
- **Settings** are `platform/settings/<file>.rs` and `seen.rs`; `bar_item_icon` lives in Shell Settings' Status Bar page.
- **Tokens:** `tokens::rules` is `tokens::lint`, behind a `tools` feature that `xtask` turns on; `amane-shell tokens …` only points at `cargo xtask`.

---

### Phase 9: Product code out of amane (L)

**Goal.** amane becomes a generic library again, which also shrinks the fork by about 9.5k lines before its rebase in Phase 12. It comes after the kit work so the new crates can use the final kit.

**Tasks**
- [shell] New crate `crates/status` (`amane-status`): the status-item and Live Activity wire types and client from `amane/src/status_item/`, with their tests. drop, train-watch and the shell's server use it. The bus name, object path, methods and errors stay as they are.
- [shell] New crate `crates/phone` (`amane-phone`): KDE Connect and phone messages from `amane/src/services/kdeconnect/`, adb and its mDNS discovery from `amane/src/services/adb*`. The `caelestia` GUIDs and run-commands become configuration in the shell. The shell, Messages and Mirror depend on it.
- [amane] Remove `status_item/`, `services/kdeconnect/`, `services/adb*`, the `mdns-sd` dependency, and `pub use serde`.
- [amane] Bluetooth: move the user-facing strings (`errors.rs:72-100`, `label()`, `description()`) and the 10 s failure timers out to the shell. The service reports a `ServiceError { kind, message }` and the time it happened.
- [amane] One error model for fork services: a `ServiceError` with typed kinds, returned by every blocking call and kept as the last failure. Each service exposes "in progress" the same way.
- [amane] polkit: make the helper socket path (`/run/polkit/agent-helper.socket`) a setting with the current value as default; put the agent behind a `polkit-agent` feature.
- [amane] `docs/fork/README.md`: remove the extracted features; note where they went.
- [shell] Bump the amane rev; `outputHashes`.

**Acceptance**
- amane builds with no reference to `org.amane`, KDE Connect or adb.
- The fork diff against upstream is about 9k lines smaller.
- Drop and train-watch items appear and update in the bar.
- Phone pairing, messages, mirroring, Bluetooth pairing and failures, and the polkit prompt work live.

**Risks.** Medium–high, **cross-repo contract**: amane's API shrinks, and the shell must move in step, so land the shell crates first, then the amane removal and the rev bump together. D-Bus wire format unchanged.

**Status (2026-11-07).** Done in four steps that each built: amane `8efa0b2`/`8e5c110` (additive), shell `aff6e3c`/`88893b5` (the crates), amane `3a119c5`/`e7bf47a` (removal and the error model), then the shell's pin to `e7bf47a`.
- **Size.** The moved code was about 4.9k lines (`status_item` 915, `kdeconnect` 3,438, `adb` 634), not 9.5k. The fork diff went from +40,957 to +36,348 lines.
- **`pub use serde` stays.** The kit and the apps derive settings through `amane::serde` in about 20 files; it's one line, and generic.
- **Helpers made public, all generic:** `run_in_order` (the ordered control thread), `Service::watched_within`, `Service::peek` (a read that doesn't count as watching) and `IDLE_STOP`. `amane-phone` needs nothing else from amane's insides.
- **Run commands.** `KdeConnect::set_run_command_list` takes the program's list once; the shell's four, with their old ids, are in `services/phone.rs`. A list was simpler than passing it to each call, since reading a device also checks whether its commands are on.
- **Errors.** `ServiceError { kind, message, at }`. Bluetooth's words are in `tiles/bluetooth_text.rs`, polkit's "Sorry, that didn't work" in its alert, and Quick Settings shows any service's failure for 10 s (`tiles/failure.rs`). Network now keeps failed joins, a wrong password among them, and the Wi-Fi rows show them. `Lock` and `Pam` are upstream and keep their own types.
- **polkit.** `serve_agent_with(PolkitAgent { helper_socket })`; the agent is behind `polkit-agent`, off by default and on in the shell's workspace dependency.
- **Open: the `niri-fork` feature.** The target architecture (above) puts fork-only code behind a `niri-fork` cargo feature; Phase 6 chose a run-time `features()` check instead. Decide in Phase 12.

---

### Phase 10: Shell internal structure (L)

**Goal.** Make the shell consistent: same module shape for every surface, explicit boundaries between surfaces, one way to persist state, one way to report errors.

**Tasks**
- [shell] Split the large surfaces into `mod.rs` (surface and public API), `state.rs`, `view.rs`, `input.rs` and `ipc.rs`:
  - launcher (1,683 lines; the 185-line `on_key` and 180-line `panel()`);
  - dock (1,317; the 250-line `build`);
  - widgets (1,224);
  - Quick Settings' 320-line `surface()`;
  - `fill.rs` becomes a folder like its peers.
- [shell] Boundaries:
  - `chrome/panel.rs` (already `chrome/`, since Phase 8) stops importing the surfaces; surfaces register with it;
- [shell] One menu, carried over from Phase 8: `controls::menu` holds one item model (icons, submenus, separators) and one drawing, with two hosts: the xdg popup app windows use, and the shell's layer surface from `surfaces/context_menu`, whose 15 callers move onto the kit's types;
  - phone code spread over 5 surfaces moves into `services/phone`;
  - do-not-disturb (`surfaces/notifications/dnd.rs`) and media art (`tiles/media.rs`) move to `services/`;
  - the clock moves from `services/launcher/` to `services/`;
  - nothing imports another surface's internals. Add a test that greps for `surfaces::<x>::<private module>` across surfaces.
- [shell] The typed shell IPC: `kit::platform::shell_ipc::ShellCommand` with the same line verbs. `ipc.rs` in the shell holds the one verb table. Apps use the type instead of the string `"amane"` and hand-built verbs (in `quick_settings.rs`, `displays/night.rs`, `desktop.rs`, `passwords.rs`).
- [shell] Logging: the `log` crate with one init per binary (stderr, so the journal still gets it). Replace `eprintln!`. Each `let _ =` on a fallible call either logs or says in a comment why the error doesn't matter.
- [shell] State files: one serde helper with atomic writes and a version field. **Migrate with a fallback**: on first start, read each old text format and write the new one:
  - `dock/pins.rs`, `night_light.rs`, `dnd.rs`, `vpn`;
  - the hand-built `serde_json::Value` files in `widgets/store.rs` and `quick_settings/layout.rs`.

  Keep the old reader for two releases.
- [shell] Thread-local state: one `surface_state!` macro (or a small type) for the "per-monitor `HashMap<String, T>` behind `with`/`with_mut`" pattern written in the dock, widgets and status bar.
- [shell] Remove the shims:
  - the `taskbar` migration in `status_bar::migrate()` and the `taskbar` IPC alias, after checking the live files are migrated;
  - the `amane-shell layer-rules` and `tokens` subcommands left from Phase 4;
  - amane's `spawn(&str)` wrapper.
- [shell] Shared helpers for the remaining copies: one logind client (vault, keep-awake, session) on amane `Logind`; one supervised child-stream helper with restart and back-off for the streams that stay (tailscale `watch-ipn`, `pw-dump`).

**Acceptance**
- No surface file over about 600 lines.
- The boundary test passes.
- All persisted state is in the new format after one start; deleting the new file and starting again re-migrates from the old one, if present.
- The live session keeps dock pins, widget layout, Quick Settings layout, night light and do-not-disturb across the switch.

**Risks.** Medium–high: **file formats change**, guarded by the migration. Back up `~/.config/amane-shell` and `~/.local/state/amane-shell` before the switch.

---

### Phase 11: Apps cleanup (M)

**Goal.** The apps use the shared pieces instead of their own, and `displays.kdl` gets one writer.

**Tasks**
- [shell] `displays.kdl`: one `kit::platform::displays_kdl` module that parses and writes the file atomically. Shell Settings owns it. The shell's HDR white-point code stops editing it and either asks niri directly (an IPC action) or asks Shell Settings' service, so there is no race between two writers.
- [shell] Shell Settings uses amane's niri client instead of `apps/settings/src/displays/niri.rs`, which is removed.
- [shell] New crate `crates/nixos` (`amane-nixos`): the NixOS model, catalog and job pipeline from `apps/settings/src/system/` (about 8.8k lines), with no UI. More tests for the job pipeline. Merge the two `about.rs` files.
- [shell] Apps use `platform::process` instead of their own runners (`system/nixos.rs`, `passwords.rs`, `storage.rs`, `displays/ddc.rs`).
- [shell] Shell Settings search:
  - share the shell's fuzzy matching (`launcher/rank.rs`), moved to the kit;
  - add a test that every page and section has an entry in the search index, so the 771-line static table can't drift.
- [shell] `train-watch`: read Tailscale state through the shell's service (or `amane-status`), not its own `tailscale` CLI calls, if it runs beside the shell.
- [shell] The hand-typed ids in `layer_rules.rs` (`DROP_NS`, `APP_IDS`) come from the apps' crates. The test covers every app, and says whether shader-lab gets a rule.

**Acceptance**
- One writer of `displays.kdl` (grep-checked).
- HDR brightness, displays arrangement and night light work after the switch.
- Shell Settings' System pages run an update and a rollback end to end.
- `amane-nixos` has its own tests in CI.

**Risks.** Medium: `displays.kdl` ownership is a format and behaviour change on the niri side; the existing file must be read without loss.

---

### Phase 12: amane fork: restructure and rebase (L)

**Goal.** A small, well-shaped fork rebased on upstream `main`. It comes after Phase 9, when the fork is about 9.5k lines smaller and no longer changing shape. Upstream reopens in mid-December 2026, so there may be more to rebase onto by then.

**Tasks**
- [amane] Rewrite `ocbk` into feature groups on the current base, like niri in Phase 5:
  - split the bundled commits (`19d012d`: popups, windows, gestures; `af0f854`: video, adb, KDE Connect, now partly gone; `74b5ddb`: input method, secure fill, zeroize);
  - fold the 22 interleaved `docs(fork)` commits into their features;
  - order the groups from additive to invasive.

  Same tree check, `range-diff`, and an `ocbk-pre-tidy` branch.
- [amane] Reduce hunks in the hot upstream files where it can be done without behaviour change, by moving fork code into new files behind hooks:
  - `widgets/text_input/field.rs` (+782);
  - `gpu/paint.rs` (+764);
  - `quads/{draw,batch}.rs`;
  - `compositor/niri.rs`;
  - `wayland/{normal,keyboard,dropped}.rs`;
  - `graphics/font.rs`.
- [amane] Rebase onto `upstream/main`. Resolve the 6 conflicting files:
  - `graphics/gpu.rs`, `services/apps/parse.rs`, `graphics/renderer.rs`, `services/apps.rs`, `wayland/dropped.rs`;
  - by hand-porting, `text_input/targets.rs`: upstream's `submit()` clears password fields, the fork restructured that code with zeroize.
- [amane] Tag `fork-base-<date>`. Update `maintaining.md` with the routine (same as niri's) and the hunk counts.
- [vello] Rebase onto upstream `main` (clean today) and bump the pinned rev. Express the scratch-size override as an option instead of an environment variable while at it.
- [shell] Bump the amane and vello revs and `outputHashes`.

**Acceptance**
- The fork is based on current upstream `main`.
- `git log upstream/main..ocbk` reads as one group per feature.
- CI is green; the shell runs on the new rev, with the text field, password fill, popups, gestures and glass checked live.

**Risks.** High for the text-field port (password clearing is security-relevant: test fill, submit and clear by hand). Keep `ocbk-pre-tidy` until a week of use.

---

### Phase 13: Docs and rustdoc completion (M)

**Goal.** Docs that a newcomer, or you in six months, can trust. This comes last so it describes the finished structure. The docs touched in each earlier phase are kept true as that phase lands.

**Tasks**
- [shell] `docs/architecture.md`:
  - the layers and the rules for what goes where (from this roadmap);
  - the runtime: event loop, markers and redraws, threads, services, settings files and live reload;
  - how a surface is built;
  - how status apps plug in.
- [shell] `docs/integration/contract.md` (from Phase 5) brought up to date. `docs/development/release.md`:
  - bumping amane, niri and vello revs;
  - `outputHashes`;
  - nixos-config inputs, and when a niri change needs a re-login;
  - the `scripts/check-pins` and `xtask contract` checks.

  Move the "Shipping" part of `development/status-apps.md` here.
- [shell] `docs/development/testing.md`:
  - fixtures, golden tests and `--ignored` live tests;
  - running a second shell beside the live one;
  - `idle-report`.
- [shell] Remove duplicated docs:
  - the status-app method table: keep it in `integration/status-apps.md`, and remove the copies now that `amane-status` lives here;
  - the memory write-ups: one in amane's `docs/fork/internals/memory.md`, the shell's points to it;
  - `lists-m3.md`, depending on the Phase 8 decision.
- [shell] Terminology, written in `docs/README.md`:
  - "Shell Settings", with `amane-settings` the first time in each doc;
  - "Quick Settings";
  - a status app is the program, a status item is its thing in the bar;
  - "notification center" lowercase;
  - one spelling of colour/color in prose.

  Apply across the docs.
- [shell] `missing_docs` becomes `deny` for amane-kit, `amane-status`, `amane-phone` and `amane-nixos`.
- [amane] Rustdoc (`///`) on all public items in fork modules; upstream's own files keep their style. A `cargo doc` warnings check in CI.
- [niri] Make sure `docs/fork/README.md` describes each feature group as Phase 5 left it.
- [shell] Reword this roadmap's mentions of the removed terms (the ground rule, Phases 3 and 8), and drop it from `lint-terms`' allowlist, which is then empty.

**Acceptance**
- `cargo doc --workspace` builds without warnings, and `missing_docs` is `deny` in the named crates.
- Each repo's docs index lists every doc.
- The link and IPC-doc tests pass.
- A read-through of README → architecture → contract → release gives an accurate picture with no stale paths.

**Risks.** None to behaviour.

---

## After the roadmap

- **niri releases:** follow `maintaining.md`'s routine for each tag. The first one will likely include Smithay's Dispatch2 refactor; budget an M for porting the four fork protocols.
- **amane upstream:** rebase when `upstream/main` moves; with the fork restructured, a rebase should be an S.
- **Watch the idle numbers:** re-run `idle-report` after any change to services and keep the result in `docs/development/idle-baseline.md`.
