# Contributing

Thank you for your interest in `fast-natord`. This document covers the
development workflow and conventions.

## Code of Conduct

This project is governed by the [Contributor Covenant](CODE_OF_CONDUCT.md).
By participating you agree to uphold its terms.

## Getting started

**Prerequisites:** Rust 1.91+ and [`just`](https://github.com/casey/just)
(a command runner — see [justfile](./justfile) for all recipes).

```sh
# Clone and enter the repo
git clone https://github.com/MuntasirSZN/fast-natord
cd fast-natord

# Install tools (will use cargo-binstall if
# found, cargo install otherwise)
just install-tools

# Build everything (all features, locked)
just build

# Run the full test suite
just test
```

## Development commands

Most daily commands are wrapped in the `justfile`:

| Command | What it does |
|---|---|
| `just fmt` | Format with `rustfmt` |
| `just lint` | Clippy with `-D warnings` |
| `just test` | Test with `cargo-nextest` across every feature combination |
| `just build` | Build with all features |
| `just check-features` | Compile-check each feature flag in isolation |
| `just docs-ci` | Build docs without default features |
| `just deny` | Run `cargo-deny` license/advisory checks |
| `just pre-commit` | Run format + lint + test + deny (use as a git hook) |
| `just ci-check` | Same sequence as CI: fmt, lint-per-feature, build, test, docs, audit |
| `just bench` | Run benchmarks locally |
| `just coverage` | Generate a coverage summary |
| `just coverage-html` | Generate and open an HTML coverage report |
| `just mutants` | Run mutation tests with `cargo-mutants` |
| `just miri` | Run Miri (nightly) for UB detection |

## Testing

Tests use **nextest**. Run the full suite with:

```sh
just test
```

This invokes `cargo hack nextest run --locked --optional-deps --each-feature`,
which builds and tests every feature combination separately — including the
`no_std` default, the `normalize` feature, and both together.

### Property tests

Property-based tests live in [`tests/proptests.rs`](./tests/proptests.rs) and
use the [`proptest`](https://crates.io/crates/proptest) crate. They verify
reflexivity, transitivity, and ordering invariants. Run them as part of
`just test`.

### Mutation testing

Mutation tests use `cargo-mutants`. Run locally:

```sh
just mutants
```

### UB detection

Run [Miri](https://github.com/rust-lang/miri) (nightly) to detect undefined
behavior:

```sh
just miri
```

### Formal verification

[Kani](https://model-checking.github.io/kani/) proofs live alongside the
implementation. Run:

```sh
cargo kani
```

## Feature flags

The crate has one optional feature, `normalize`, which enables Unicode
normalization and SIMD-accelerated case folding (via `simd-normalizer`).
The default `#![no_std]` build omits it.

When making changes, ensure compilation and tests pass for both `--no-default-features`
(default) and `--all-features`. `just test` and `just check-features` handle this.

## Submitting changes

1. Create a feature branch from `main`.
2. Make your changes — keep them focused on one concern.
3. Commit your changes. Commits must follow 
   [conventional commit](https://www.conventionalcommits.org/) format.
4. Run `just pre-commit` (If you installed the git hook, then wait
   for it to fully complete.)
4. Open a pull request against `main`. The PR title must match the
   [conventional commit](https://www.conventionalcommits.org/) format.
5. A maintainer will review — expect discussion and possibly iteration.

### Commit style

We use [conventional commits](https://www.conventionalcommits.org/).

### PR checklist

Before opening a PR:

- [ ] `just pre-commit` passes locally (If you installed the git hook, then wait for it to fully complete)
- [ ] New behavior has tests (unit, property, or integration as appropriate) or proofs
- [ ] Public API changes are reflected in the crate-level docs and README
- [ ] `no_std` compatibility is preserved (unless the feature explicitly
      requires `alloc`)

## Reporting issues

For bug reports and feature requests use the [Issues](https://github.com/MuntasirSZN/fast-natord/issues) tab and 
click on "New Issue" button, choose an appropriate template and create. For security
vulnerabilities, see [`SECURITY.md`](./SECURITY.md) instead of filing a
public issue.

## License

By contributing, you agree that your contributions will be licensed under
the [MIT License](LICENSE).
