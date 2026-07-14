# `fast-natord`

Natural ordering for Rust — compares strings with awareness of numeric
subsequences so that `"rfc2"` precedes `"rfc10"`.

```rust
let mut files = vec!["rfc2086.txt", "rfc822.txt", "rfc1.txt"];
files.sort_by(|&a, &b| fast_natord::compare(a, b));
assert_eq!(files, ["rfc1.txt", "rfc822.txt", "rfc2086.txt"]);
```

## Quick Start

| Function / type | Description | Feature |
|---|---|---|
| `compare(a, b)` | Case-sensitive natural order | — |
| `compare_ignore_case(a, b)` | Case-insensitive (ASCII fast; non-ASCII via `char::to_lowercase`) | — |
| `compare_iter(a, b, skip, cmp, to_digit)` | Fully customizable iterator-based comparison | — |
| `Normalizer` | Configurable pre-normalization (NFC, case folding, etc.) | `normalize` |
| `compare_normalized(a, b)` | NFC + case-fold convenience | `normalize` |

## Configurable Normalization

The [`Normalizer`] type preprocesses strings before comparison in a separate
step, keeping the hot comparison loop free of per-character normalization
overhead.

```rust
use fast_natord::{Normalizer, Normalization, CaseMode};

// NFC normalization + case folding
let norm = Normalizer::new()
    .normalization(Normalization::Nfc)
    .case(CaseMode::Fold);

// Canonically equivalent strings compare equal
assert_eq!(norm.compare("\u{00E9}", "e\u{0301}"), std::cmp::Ordering::Equal);
assert_eq!(norm.compare("ABC", "abc"), std::cmp::Ordering::Equal);
assert_eq!(norm.compare("pic10", "pic2"), std::cmp::Ordering::Greater);
```

Normalization happens **once per string**, not once per character inside the
comparison loop. On all-ASCII inputs the normalizer short-circuits via SIMD
with zero allocation.

### Feature Flags

| Feature | Default | Description |
|---|---|---|
| `normalize` | off | Enables NFC, NFD, NFKC, NFKD normalization and SIMD-accelerated case folding via [`simd-normalizer`](https://crates.io/crates/simd-normalizer) (Unicode 17). |

Without `normalize`:
- `Normalization::Nfc` / `Nfd` / `Nfkc` / `Nfkd` silently behave as `None`.
- `CaseMode::Fold` falls back to `char::to_lowercase()` (no SIMD).
- `CaseMode::AsciiOnly` and `CaseMode::Sensitive` are unaffected.

## `no_std`

`fast-natord` is `#![no_std]` by default. The core API uses
`core::cmp::Ordering` and `&str` / `&[u8]` arguments.
The `normalize` feature additionally requires `alloc`.

## SIMD Optimized

All core comparison paths use SIMD where available via dynamic dispatch and compile-time
feature detection:

| Operation | x86_64 | AArch64 | WASM32 |
|---|---|---|---|
| Prefix skip (`simd_skip_equal`) | SSE2, SSE4.1, SSE4.2, AVX2, **AVX-512BW** | NEON | simd128 |
| ASCII detection (`simd_is_ascii`) | SSE2, SSE4.1, AVX2, **AVX-512BW** | NEON | simd128 |
| Digit-run end scan (`simd_skip_while_digit`) | SSE2, AVX2, **AVX-512BW** | NEON | simd128 |

WASM SIMD is enabled at compile time via `-Ctarget-feature=+simd128`. Without this flag,
WASM32 targets use the portable scalar fallback. x86_64 dispatch is ordered by priority:
AVX-512BW → AVX2 → SSE4.2 → SSE4.1 → SSE2; only features the CPU supports are used.

The normalizer additionally delegates to `simd-normalizer`'s 64-byte single-pass
SIMD-guided architecture when the `normalize` feature is enabled.

## Panic-Free

All public functions are guaranteed not to panic for any input.

## Safety

As this crate contains SIMD, it has a lot of unsafe. To ensure safety, we do:

- Extensive unit and integration tests for correctness and panic-freedom.
- Fuzz testing with `afl.rs`.
- Prove code is correct via formal verification using Kani.
- Use `miri` to check for undefined behavior.
- Extensive property tests via `proptest`.

## `compare_iter`

For fully custom natural ordering (different digit bases, whitespace rules, etc.),
use `compare_iter`:

```rust
use fast_natord::compare_iter;
use std::cmp::Ordering;

let result = compare_iter(
    "pic10".chars(),
    "pic2".chars(),
    |c| c.is_whitespace(),
    |a, b| a.cmp(b),
    |c| c.to_digit(10).map(|v| v as isize),
);
assert_eq!(result, Ordering::Greater);
```

## MSRV

Rust 1.91.0 edition 2024.

## Origin

Hard-forked from the [`natord`](https://crates.io/crates/natord) crate (MIT License).
Complete rewrite with word-at-a-time prefix scanning, length-based digit
comparison, branchless digit detection, SIMD prefix skipping, configurable
Unicode normalization, and `#![no_std]` support.

## License

MIT — see [LICENSE](./LICENSE).
