# Testing

What the tests cover, how to run the ones that need a screen, and how to try a change beside the
running desktop without touching it.

## What runs everywhere

```sh
env -u LD_LIBRARY_PATH nix develop -c cargo test --workspace
```

`env -u LD_LIBRARY_PATH` keeps a terminal's own libraries (VS Code's, for one) out of the dev
shell. CI runs the same tests with `--locked`, and `nix build` runs them again. None of them need a
compositor, a GPU or the network.

- **Parsers against recorded output.** The services that read programs and files are tested on
  what those really printed, kept in `apps/shell/tests/fixtures/`: niri (`outputs.json`,
  `windows.json`, `casts.json`), PipeWire (`pw-dump`), sysfs (LEDs, rfkill), Tailscale, and the
  shell's own state files in their old formats (`state/`), which the migrations must keep reading.
  A new parser gets a fixture recorded from the real program, not one written by hand.
- **Golden tests.**
  - The niri layer rules: `cargo xtask layer-rules` must print what's committed, and every layer
    namespace the shell opens must have a rule (`apps/shell/src/tokens/layer_rules.rs`).
  - The IPC verbs: every verb the shell registers is in
    [ipc.md](integration/ipc.md#commands), and each sends the same lines scripts send today
    (`apps/shell/src/ipc.rs`).
  - The material settings sit inside the ranges niri accepts
    (`tests/fixtures/niri/material-ranges.json`).
- **Rules about the code.** The shell's structure is held by tests in `surfaces/mod.rs`: no
  surface names another's modules, `services/` and `chrome/` name no surface, no surface file is
  over 650 lines ([shell-structure.md](shell-structure.md)). Emaki Settings' search index is
  checked against its pages and rows, and `displays.kdl` and `input.kdl` each have a single writer
  (`apps/settings/src/displays/kdl.rs` and `input/kdl.rs`, which also round-trip the live files'
  shapes). `cargo test -p emaki-settings write_for_niri_validate -- --ignored` with
  `EMAKI_TEST_DIR` set writes a moved copy of the live config and an `input.kdl` with every
  setting changed, for `niri validate -c $EMAKI_TEST_DIR/config.kdl`.
- **Fake programs.** `emaki-nixos` runs its jobs (build, compare, activate, update, roll back)
  against stand-in `nix`, `git` and `pkexec` scripts, whose paths come from its `Config`. Write a
  test for a new step the same way rather than calling the real tools.
- **The token tools.** The kit checks the design tokens' contract and their contrast.

When a test that compares the code with a doc or the compositor fails, its message says which
side to fix.

## Tests that need more

Marked `#[ignore]` with the reason, and run on purpose:

```sh
env -u LD_LIBRARY_PATH nix develop -c cargo test -p <package> -- --ignored
```

| Where | Needs |
|---|---|
| amane's quad tests (`graphics/gpu/quads/draw.rs`, `draw/fork_tests.rs`), in amane | a GPU adapter |
| `apps/settings/src/displays/caps.rs` | a running niri and a screen |
| `apps/mirror/src/decode.rs` | `EMAKI_MIRROR_TEST_H264` pointing at an H.264 file |

amane's GPU tests draw with the quads and with vello and compare the pixels. Run them after a
change to the quads, shadows, arcs or the frame's encoder.

## Beside the running desktop

A second shell can run next to the real one with its own runtime folder, so it doesn't take the
real bar's socket:

```sh
T=$(mktemp -d)
XDG_RUNTIME_DIR=$T WAYLAND_DISPLAY=/run/user/1000/wayland-1 \
  env -u LD_LIBRARY_PATH nix develop -c ./target/debug/emaki
```

- With `XDG_CONFIG_HOME` and `XDG_STATE_HOME` pointed at copies, it doesn't touch your settings
  or state either. The full recipe, with Drop and stand-ins for `wl-copy` and `secret-tool`, is
  in [status-apps.md](integration/status-apps.md#a-test-bar-and-drop-isolated).
- What it still shares: the session bus (only one program owns `org.emaki.StatusBar1`), the
  notification server, and niri itself.
- Stop it by its runtime folder (look for it in `/proc/<pid>/environ`), never with `pkill` on a
  name, which also matches the real shell.
- An app (Emaki Settings, Messages, an amane example) can run beside the real one the same way,
  with its own `XDG_RUNTIME_DIR`, so it doesn't reach the running copy's single-instance socket.

To build against an amane checkout you are changing, use the local override
([CONTRIBUTING](../../CONTRIBUTING.md#development)): `cargo --config .cargo/local.toml run`. Give
clippy the flag after its name: `cargo clippy --config .cargo/local.toml …`.

## Measuring

- `AMANE_FRAMES=1` logs every frame and what woke it ([performance.md](performance.md#measuring)).
- `scripts/idle-report [seconds]` counts what the running shell costs while nothing happens:
  wakeups, CPU time, memory and the programs it starts. Run it under `sudo` for exact program
  counts and PSS. The numbers to compare against are in [idle-baseline.md](idle-baseline.md).
- Memory, and vello's scratch buffers in particular: [memory.md](memory.md).

## A fork rebase

After rebasing amane (or niri) onto upstream, every commit must still build, not only the tip:

```sh
git rebase -x 'cargo check --all-targets; git checkout -q -- Cargo.lock' fork-base
```

`cargo check` rewrites `Cargo.lock` on older commits, and putting it back keeps the rebase going.
The whole routine, with the reference merge the rebase is checked against, is in amane's
[maintaining.md](https://github.com/emaki-desktop/amane/blob/emaki/docs/fork/maintaining.md#rebasing-onto-newer-upstream).
