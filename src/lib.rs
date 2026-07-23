//! # `fast-natord`
//!
//! Natural ordering for Rust — compares strings with awareness of numeric
//! subsequences so that `"rfc2"` precedes `"rfc10"`.
//!
//! ```rust
//! let mut files = vec!["rfc2086.txt", "rfc822.txt", "rfc1.txt"];
//! files.sort_by(|&a, &b| fast_natord::compare(a, b));
//! assert_eq!(files, ["rfc1.txt", "rfc822.txt", "rfc2086.txt"]);
//! ```
//!
//! ## Quick Start
//!
//! | Function / type | Description | Feature |
//! | --- | --- | --- |
//! | [`compare`][compare] | Case-sensitive natural order | — |
//! | [`compare_ignore_case`][compare_ignore_case] | Case-insensitive (ASCII fast; non-ASCII via `char::to_lowercase()`) | — |
//! | [`compare_iter`][compare_iter] | Fully customizable iterator-based comparison | — |
//! | [`Normalizer`] | Configurable pre-normalization (NFC, case folding, etc.) | `normalize` |
//! | [`compare_normalized`] | NFC + case-fold convenience | `normalize` |
//!
//! ## Configurable Normalization
//!
//! The [`Normalizer`] type preprocesses strings before comparison in a separate
//! step, keeping the hot comparison loop free of per-character normalization
//! overhead.
//!
//! ```
//! use fast_natord::{Normalizer, Normalization, CaseMode};
//!
//! // NFC normalization + case folding
//! let norm = Normalizer::new()
//!     .normalization(Normalization::Nfc)
//!     .case(CaseMode::Fold);
//!
//! // Case-insensitive natural ordering
//! assert_eq!(norm.compare("ABC", "abc"), std::cmp::Ordering::Equal);
//! assert_eq!(norm.compare("pic10", "pic2"), std::cmp::Ordering::Greater);
//!
//! // With the `normalize` feature, canonically equivalent strings
//! // like `é` (U+00E9) and `e\u{0301}` compare equal under NFC.
//! ```
//!
//! Normalization happens **once per string**, not once per character inside the
//! comparison loop:
//!
//! 1. [`Normalizer::normalize`] applies the configured Unicode normalization
//!    and/or case folding, returning a [`alloc::borrow::Cow<str>`] (borrowed when no
//!    transformation is needed).
//! 2. [`Normalizer::compare`] normalizes both inputs, then delegates to the
//!    same SIMD-accelerated case-sensitive comparator used by [`compare`][compare].
//!
//! On all-ASCII inputs the normalizer short-circuits via SIMD with zero
//! allocation regardless of the configured normalization form.
//!
//! ### Feature Flags
//!
//! | Feature | Default | Description |
//! | --- | --- | --- |
//! | `normalize` | off | Enables NFC, NFD, NFKC, NFKD normalization and SIMD-accelerated case folding via [`simd-normalizer`][simd_normalizer] (Unicode 17). |
//!
//! Without `normalize`:
//! * [`Normalization::Nfc`] / [`Normalization::Nfd`] / [`Normalization::Nfkc`] / [`Normalization::Nfkd`] silently behave as [`None`][None].
//! * [`CaseMode::Fold`] falls back to `char::to_lowercase()` (no SIMD).
//! * [`CaseMode::AsciiOnly`] and [`CaseMode::Sensitive`] are unaffected.
//!
//! ## [`no_std`][no_std]
//!
//! `fast-natord` is [`#![no_std]`][no_std] by default. The core API uses
//! [`core::cmp::Ordering`] and `&str` / `&[u8]` arguments.
//! The `normalize` feature additionally requires [`alloc`].
//!
//! ## SIMD Optimized
//!
//! All core comparison paths use SIMD where available via dynamic dispatch and compile-time
//! feature detection:
//!
//! | Operation | x86_64 | AArch64 | WASM32 |
//! | --- | --- | --- | --- |
//! | Prefix skip (`simd_skip_equal`) | SSE2, SSE4.1, SSE4.2, AVX2, **AVX-512BW** | NEON | simd128 |
//! | ASCII detection (`simd_is_ascii`) | SSE2, SSE4.1, AVX2, **AVX-512BW** | NEON | simd128 |
//! | Digit-run end scan (`simd_skip_while_digit`) | SSE2, AVX2, **AVX-512BW** | NEON | simd128 |
//!
//! WASM SIMD is enabled at compile time via `-Ctarget-feature=+simd128`. Without this flag,
//! WASM32 targets use the portable scalar fallback. x86_64 dispatch is ordered by priority:
//! AVX-512BW → AVX2 → SSE4.2 → SSE4.1 → SSE2; only features the CPU supports are used.
//!
//! The normalizer additionally delegates to [`simd-normalizer`][simd_normalizer]'s 64-byte single-pass
//! SIMD-guided architecture when the `normalize` feature is enabled.
//!
//! ## Panic-Free
//!
//! All public functions are guaranteed not to panic for any input.
//! The normalizer returns [`alloc::borrow::Cow::Owned`][CowOwned] only when a transformation is
//! actually applied; it never panics on allocation failure.
//!
//! ## Safety
//!
//! As this crate contains SIMD, it has a lot of unsafe. To ensure safety, we do:
//!
//! - Extensive unit and integration tests for correctness and panic-freedom.
//! - Fuzz testing with [`afl.rs`](https://github.com/rust-fuzz/afl.rs).
//! - Prove code is correct via formal verification using [Kani](https://github.com/model-checking/kani).
//! - Use [Miri](https://github.com/rust-lang/miri) to check for undefined behavior.
//! - Extensive property tests via [`proptest`](https://crates.io/crates/proptest).
//!
//! ## [`compare_iter`][compare_iter]
//!
//! For fully custom natural ordering (different digit bases, whitespace rules, etc.),
//! use [`compare_iter`][compare_iter]:
//!
//! ```
//! use fast_natord::compare_iter;
//! use std::cmp::Ordering;
//!
//! let result = compare_iter(
//!     "pic10".chars(),
//!     "pic2".chars(),
//!     |c| c.is_whitespace(),
//!     |a, b| a.cmp(b),
//!     |c| c.to_digit(10).map(|v| v as isize),
//! );
//! assert_eq!(result, Ordering::Greater);
//! ```
//!
//! ## MSRV
//!
//! Rust 1.91.0 edition 2024.
//!
//! ## Origin
//!
//! Hard-forked from the [`natord`](https://crates.io/crates/natord) crate (MIT License).
//! Complete rewrite with word-at-a-time prefix scanning, length-based digit
//! comparison, branchless digit detection, SIMD prefix skipping, configurable
//! Unicode normalization, and [`#![no_std]`][no_std] support.
//!
//! ## License
//!
//! MIT — see [LICENSE](https://github.com/MuntasirSZN/fast-natord/blob/main/LICENSE).
//!
//! [simd_normalizer]: https://crates.io/crates/simd-normalizer
//! [compare]: https://docs.rs/fast-natord/latest/fast_natord/fn.compare.html
//! [compare_ignore_case]: https://docs.rs/fast-natord/latest/fast_natord/fn.compare_ignore_case.html
//! [compare_iter]: https://docs.rs/fast-natord/latest/fast_natord/fn.compare_iter.html
//! [None]: https://doc.rust-lang.org/core/option/enum.Option.html#variant.None
//! [CowOwned]: https://doc.rust-lang.org/alloc/borrow/enum.Cow.html#variant.Owned
//! [no_std]: https://docs.rust-embedded.org/book/intro/no-std.html

#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::missing_safety_doc)]
#![allow(rustdoc::redundant_explicit_links)]

extern crate alloc;

mod byte_utils;
mod compare;
mod compare_ignore_case;
mod compare_iter;
mod normalizer;
mod unicode;

/// Compare two strings case-sensitively using natural ordering.
///
/// Operates on byte slices — UTF-8 byte-order preservation guarantees
/// correct results for case-sensitive comparison without decoding.
#[inline(always)]
pub fn compare(left: &str, right: &str) -> core::cmp::Ordering {
    compare::compare_impl(left.as_bytes(), right.as_bytes())
}

/// Compare two strings case-insensitively using natural ordering.
///
/// ASCII case folding via `to_ascii_lowercase`; non-ASCII chars are
/// decoded and lowercased via [`char::to_lowercase`].
///
/// For better performance on non-ASCII data (especially repeated
/// comparisons of the same strings), consider [`Normalizer`] with
/// [`CaseMode::Fold`] instead — it pre-processes case folding once
/// and avoids per-character decoding in the hot loop.
#[inline(always)]
pub fn compare_ignore_case(left: &str, right: &str) -> core::cmp::Ordering {
    compare_ignore_case::compare_ignore_case_impl(left.as_bytes(), right.as_bytes())
}

/// Iterate over all T and compare each sequentially
/// The skip callback skips any characters that does not affect the comparison,
/// the cmp callback compares two characters' ordering,
/// and the to_digit callback converts a character into a numeric digit.
///
/// Example:
/// ```rust
/// use fast_natord::compare_iter;
/// use core::cmp::Ordering;
/// let result = compare_iter(
///     "pic10".chars(),
///     "pic2".chars(),
///     |c| c.is_whitespace(),
///     |a, b| a.cmp(b),
///     |c| c.to_digit(10).map(|v| v as isize)
/// );
/// assert_eq!(result, Ordering::Greater);
/// ```
pub use compare_iter::compare_iter;

// ── Configurable normalizer ───────────────────────────────────────────

pub use normalizer::{CaseMode, Normalization, Normalizer, compare_normalized};
