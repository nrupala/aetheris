# Contributing to Aetheris

## DCO

All commits must include `Signed-off-by: Your Name <email>`.
This certifies compliance with the [Developer Certificate of Origin 1.1](https://developercertificate.org/).

## Code Standards

- Rust: `rustfmt` + `clippy` with deny warnings
- Python: `ruff` + `mypy` with strict mode
- All code must pass CI before merge

## Build and test

```bash
cd core
cargo fmt --check
cargo clippy -- -D warnings
cargo build --release --target x86_64-unknown-linux-musl
cargo test
```

Native deploy runbook: `docs/DEPLOY_NATIVE.md`.

## PR-flow discipline

- **Draft PR → CI green → owner merges.** No direct pushes to `main`, ever.
- Every PR adds its CHANGELOG entry under `## [Unreleased]` and bumps the
  version — `core/Cargo.toml` **and** `VERSION` (they mirror each other) —
  patch for fixes/chores, minor for features.
- Merge commits reference the PR number (e.g. `Merge pull request #42 ...`).
- Releases are tagged `vX.Y.Z` after merge (tagging `v*.*.*` triggers the
  release-binary workflow).

## License

MIT OR Apache-2.0 — see LICENSE file.
