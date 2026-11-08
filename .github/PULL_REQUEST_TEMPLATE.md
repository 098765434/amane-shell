## What

<!-- what this changes, in a sentence or two -->

## Why

<!-- the problem it solves, or the issue it closes (`Closes #…`) -->

## Checks

Before asking for review ([CONTRIBUTING.md](../CONTRIBUTING.md#before-a-pr)):

- [ ] `cargo fmt --all` leaves nothing to do
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] `cargo test --workspace` passes
- [ ] `cargo xtask links`: every relative link names a file, and every doc is in `docs/README.md`
- [ ] `scripts/lint-terms`: no other vendor's platform terms
- [ ] the docs that describe what changed are updated (a new surface or verb has its doc)
- [ ] a line under `## [Unreleased]` in `CHANGELOG.md`
