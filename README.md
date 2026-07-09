# `fast-natord`

Natural ordering for Rust.  Compares strings with awareness of numeric
subsequences so that `"rfc2"` precedes `"rfc10"`.

```rust
let mut files = vec!["rfc2086.txt", "rfc822.txt", "rfc1.txt"];
files.sort_by(|&a, &b| fast_natord::compare(a, b));
assert_eq!(files, ["rfc1.txt", "rfc822.txt", "rfc2086.txt"]);
```

## Quick start

| Function | Description |
|---|---|
| `compare(a, b)` | Case-sensitive natural order |
| `compare_ignore_case(a, b)` | Case-insensitive natural order (ASCII fast path; non-ASCII via `char::to_lowercase`) |
| `compare_iter(a, b, skip, cmp, to_digit)` | Fully customizable iterator-based comparison |
| `Collator::try_new(locale)` | Reusable locale-aware collator (requires `locale` feature) |
| `compare_locale(a, b, locale)` | Locale-aware one-shot comparison (requires `locale` feature) |

## `no_std`

`fast-natord` is `#![no_std]` by default.  The core API uses
`core::cmp::Ordering` and `&str` / `&[u8]` arguments.

## SIMD Optimized

Uses cpu specific optimizations via dynamic dispatch, for sse2, sse4.1, sse4.2, avx2 and gfni on x86_64, and neon on arm.

## Locale support (optional)

Enable the `locale` feature to add locale-aware comparison via ICU4X
collation:

```toml
[dependencies]
fast-natord = { version = "0.1", features = ["locale"] }
```

This exposes a [`Collator`] struct that wraps ICU's `CollatorBorrowed`:

```rust
use fast_natord::Collator;

let collator = Collator::try_new("fr").unwrap();
assert_eq!(collator.compare("cote", "côte"), core::cmp::Ordering::Less);
assert_eq!(collator.compare("côte", "coté"), core::cmp::Ordering::Less);
```

Construction is somewhat expensive — reuse the same `Collator` for an
entire sort batch.

### One-shot convenience

```rust
use fast_natord::collator::compare_locale;

// Traditional Spanish: "pollo" sorts after "polvo"
assert!(compare_locale("polvo", "pollo", "es-u-co-trad")
    .unwrap()
    .is_lt());
```

### System locale detection

```rust
use fast_natord::collator::compare_system_locale;

let ord = compare_system_locale("straße", "strasse").unwrap();
```

Falls back to `"en-US"` when the system locale is undetectable.

### Normalization

ICU collation always normalizes strings internally (NFC).  Strings in
different normalization forms (e.g. NFC `"é"` vs NFD `"e\u{301}"`)
compare as equal without preprocessing.

The natural-order functions (`compare`, `compare_ignore_case`) operate on
raw bytes and do **not** normalize.

## Panic-free

All public API functions are guaranteed not to panic for any input.
Fallible operations return `Result`:

- `Collator::try_new` → `Result<Collator, CollatorError>`
- `compare_locale` → `Result<Ordering, CollatorError>`
- `compare_system_locale` → `Result<Ordering, CollatorError>`
- `compare` / `compare_ignore_case` / `compare_iter` are infallible

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

## Origin

Hard-forked from the [`natord`](https://crates.io/crates/natord) crate (MIT License).
Complete rewrite with word-at-a-time prefix scanning, length-based digit
comparison, branchless digit detection, and `#![no_std]` support.

## License

MIT — see [LICENSE](./LICENSE).
