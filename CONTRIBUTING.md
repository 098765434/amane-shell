# Contributing

Thanks for your interest! `amane-shell` is a Wayland bar for the
[niri](https://github.com/YaLTeR/niri) compositor, built on the
[amane](https://github.com/098765434/amane) library fork.

## Development

```sh
nix develop            # Rust toolchain + wayland/vulkan/pulse/fontconfig/pam libs
cargo run              # run the bar (needs a running niri session)
```

`amane` is a **git dependency** pinned to a rev (see `Cargo.toml`). To hack on the shell and the
library together, check `amane` out as a sibling directory and create a git-ignored
`.cargo/local.toml`:

```toml
paths = ["../amane"]
```

and build with it: `cargo --config .cargo/local.toml run` (an alias in your shell profile saves
typing it). Cargo then builds against your local `../amane` checkout instead of the pinned rev.
`.cargo/config.toml` is tracked and holds only the `xtask` alias.

### The workspace

`apps/shell` is the shell, and `cargo run` with no `-p` runs it. The programs beside it are in
`apps/` (the assistant's daemon is the package `amane-assistant-daemon`), the crates they share in
`crates/`: the kit (with the design tokens, `crates/kit/design-tokens/`), rbw's pinentry and the
assistant's library. `cargo xtask` runs the workspace's tasks:

| Task | Does |
|---|---|
| `cargo xtask layer-rules` | prints the niri layer rules from the tokens (`> niri-layer-rule.kdl`) |
| `cargo xtask tokens normalize <file>` | prints a Penpot export sorted as it's committed |
| `cargo xtask tokens check <file>` | runs the manual's rules on a token file |
| `cargo xtask lint-terms` | `scripts/lint-terms` |
| `cargo xtask links` | fails on a relative Markdown link to a file that isn't there |
| `cargo xtask contract` | amane and niri agree at the pinned revisions ([contract.md](docs/integration/contract.md)); reads `../amane` and `../niri` |

### Bumping revs

Bumping amane, vello or niri, the hashes Nix needs, nixos-config and the switch are in
[docs/development/release.md](docs/development/release.md).

`scripts/check-pins` tells you when that local build differs from the Nix build: `../amane` isn't
at the pinned rev, has uncommitted changes, or pins a different vello rev than the shell does.
`--strict` makes it exit 1, for a git hook.

`scripts/idle-report [seconds]` measures what the running shell costs while nothing happens:
wakeups, CPU time, memory and the programs it starts. Run it with `sudo` for an exact count of
programs and PSS instead of RSS. [docs/development/idle-baseline.md](docs/development/idle-baseline.md)
holds the numbers to compare against.

## Before a PR

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same in the devShell (`fmt --check`, and clippy and the tests with `--locked`), then
shellcheck on `gen-layer-rules.sh` and the scripts, then `nix build`, which runs the tests again.

The workspace lints are in the root `Cargo.toml` (`[workspace.lints]`); `unwrap` is allowed only in
tests (`clippy.toml`). In the kit, a file that starts with `#![allow(missing_docs)]` still has
undocumented items: document them and remove the line. Some tests check the code against the
docs and the compositor: every IPC verb is in `docs/integration/ipc.md`, every layer namespace
has a niri rule, every material setting is in niri's range, and the service parsers read the
recorded output in `apps/shell/tests/fixtures/`. When one fails, the message says which side to fix.

`scripts/lint-terms` (also in CI) fails on terms borrowed from another vendor's platform: name a feature for
what it is ("the launcher", "Quick Settings", "glass"). Names kept on purpose are in
`scripts/lint-terms.allow`, or carry a `lint-terms: allow` comment. The forks' CI runs it on the
lines they add, with `--diff`.

Before adding a component, read [docs/development/performance.md](docs/development/performance.md):
what makes a window draw, and the rules that keep an idle desktop from drawing at all. Every doc
is listed in [docs/README.md](docs/README.md).

## Scope notes

- Library-level changes (popups, compositor services) belong in
  [amane](https://github.com/098765434/amane), not here; compositor changes in
  [niri](https://github.com/098765434/niri). Each fork documents what it adds
  ([amane](https://github.com/098765434/amane/blob/ocbk/docs/fork/README.md), [niri](https://github.com/098765434/niri/blob/ocbk/docs/fork/README.md)).
- Design values (sizing/color/type) are Penpot tokens. Change them in Penpot, export, then
  `cargo xtask tokens normalize export.json > design-tokens/tokens.json` and
  `cargo xtask tokens check design-tokens/tokens.json`; never hand-edit the file or hardcode a
  value. Code reads tokens through functions generated from the file
  (`sys::color::surface::popover::bg()`), so a renamed token is a compile error. Rules and
  workflow: [docs/design-system/tokens.md](docs/design-system/tokens.md).

## License

By contributing you agree your changes are licensed under this repository's MIT license
(see [LICENSE](LICENSE)).
