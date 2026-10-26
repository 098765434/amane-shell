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

Cargo then builds against your local `../amane` checkout instead of the pinned rev. Bump the
`rev` (and the `outputHashes` in `flake.nix`) when you want the published build to pick up library
changes.

## Before a PR

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs these three inside the devShell.

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
