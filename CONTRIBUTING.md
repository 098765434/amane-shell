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

## Scope notes

- Library-level changes (popups, compositor services) belong in
  [amane](https://github.com/098765434/amane), not here.
- Design values (sizing/colour/type) are Penpot tokens. Change them in Penpot, export, then
  `cargo run -q -- tokens normalize export.json > design-tokens/tokens.json` and
  `cargo run -q -- tokens check design-tokens/tokens.json`; never hand-edit the file or hardcode a
  value. Code reads tokens through functions generated from the file
  (`sys::color::surface::popover::bg()`), so a renamed token is a compile error. Rules and
  workflow: [docs/design-system/tokens.md](docs/design-system/tokens.md).

## License

By contributing you agree your changes are licensed under this repository's MIT license
(see [LICENSE](LICENSE)).
