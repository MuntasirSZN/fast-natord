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

## `no_std`

`fast-natord` is `#![no_std]` by default.  The core API uses
`core::cmp::Ordering` and `&str` / `&[u8]` arguments.

## SIMD Optimized

Uses cpu specific optimizations via dynamic dispatch, for sse2, sse4.1, sse4.2, avx2 and gfni on x86_64, and neon on arm.


## Panic-free

All public functions are guaranteed not to panic for any input.

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

Rust 1.89.0 (`gfni` extension stabilized in this version) edition 2024.

## Origin

Hard-forked from the [`natord`](https://crates.io/crates/natord) crate (MIT License).
Complete rewrite with word-at-a-time prefix scanning, length-based digit
comparison, branchless digit detection, and `#![no_std]` support.

## License

MIT — see [LICENSE](./LICENSE).
