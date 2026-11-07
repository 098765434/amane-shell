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
`.cargo/config.toml`:

```toml
paths = ["../amane"]
```

Cargo then builds against your local `../amane` checkout instead of the pinned rev.

### Bumping revs

When the published build should pick up library changes, push amane (and vello, if it changed)
first, then:

1. Set amane's `rev` in `[workspace.dependencies]`. vello is patched in `[patch.crates-io]`
   (`vello` and `vello_encoding`), and the patch only works from the root manifest, so its `rev`
   must be the one amane's own `Cargo.toml` uses.
2. In `Cargo.lock`, change only the `source` line of the bumped crate to the new rev.
   `cargo update -p amane` re-resolves unrelated crates as well.
3. In `flake.nix`, `cargoLock.outputHashes` has one entry per git rev, not per crate:
   `amane-0.1.2` and `vello-0.11.0`. Set the bumped one to `sha256-AAAA…=` (44 characters), run
   `nix build`, and copy the hash it prints after `got:`.
4. `scripts/check-pins` is silent when the local checkouts match.

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
recorded output in `tests/fixtures/`. When one fails, the message says which side to fix.

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
- Design values (sizing/colour/type) are Penpot tokens. Change them in Penpot, export, then
  `cargo run -q -- tokens normalize export.json > design-tokens/tokens.json` and
  `cargo run -q -- tokens check design-tokens/tokens.json`; never hand-edit the file or hardcode a
  value. Code reads tokens through functions generated from the file
  (`sys::color::surface::popover::bg()`), so a renamed token is a compile error. Rules and
  workflow: [docs/design-system/tokens.md](docs/design-system/tokens.md).

## License

By contributing you agree your changes are licensed under this repository's MIT license
(see [LICENSE](LICENSE)).
