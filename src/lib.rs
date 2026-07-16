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
//! |—|—|—|
//! | `compare(a, b)` | Case-sensitive natural order | — |
//! | `compare_ignore_case(a, b)` | Case-insensitive (ASCII fast; non-ASCII via `char::to_lowercase`) | — |
//! | `compare_iter(a, b, skip, cmp, to_digit)` | Fully customizable iterator-based comparison | — |
//! | `Normalizer` | Configurable pre-normalization (NFC, case folding, etc.) | `normalize` |
//! | `compare_normalized(a, b)` | NFC + case-fold convenience | `normalize` |
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
//!    and/or case folding, returning a `Cow<str>` (borrowed when no
//!    transformation is needed).
//! 2. [`Normalizer::compare`] normalizes both inputs, then delegates to the
//!    same SIMD-accelerated case-sensitive comparator used by [`compare`].
//!
//! On all-ASCII inputs the normalizer short-circuits via SIMD with zero
//! allocation regardless of the configured normalization form.
//!
//! ### Feature Flags
//!
//! | Feature | Default | Description |
//! |—|—|—|
//! | `normalize` | off | Enables NFC, NFD, NFKC, NFKD normalization and SIMD-accelerated case folding via [`simd-normalizer`](https://crates.io/crates/simd-normalizer) (Unicode 17). |
//!
//! Without `normalize`:
//! * `Normalization::Nfc` / `Nfd` / `Nfkc` / `Nfkd` silently behave as `None`.
//! * `CaseMode::Fold` falls back to `char::to_lowercase()` (no SIMD).
//! * `CaseMode::AsciiOnly` and `CaseMode::Sensitive` are unaffected.
//!
//! ## `no_std`
//!
//! `fast-natord` is `#![no_std]` by default. The core API uses
//! `core::cmp::Ordering` and `&str` / `&[u8]` arguments.
//! The `normalize` feature additionally requires `alloc`.
//!
//! ## SIMD Optimized
//!
//! All core comparison paths use SIMD where available via dynamic dispatch and compile-time
//! feature detection:
//!
//! | Operation | x86_64 | AArch64 | WASM32 |
//! |—|—|—|—|
//! | Prefix skip (`simd_skip_equal`) | SSE2, SSE4.1, SSE4.2, AVX2, **AVX-512BW** | NEON | simd128 |
//! | ASCII detection (`simd_is_ascii`) | SSE2, SSE4.1, AVX2, **AVX-512BW** | NEON | simd128 |
//! | Digit-run end scan (`simd_skip_while_digit`) | SSE2, AVX2, **AVX-512BW** | NEON | simd128 |
//!
//! WASM SIMD is enabled at compile time via `-Ctarget-feature=+simd128`. Without this flag,
//! WASM32 targets use the portable scalar fallback. x86_64 dispatch is ordered by priority:
//! AVX-512BW → AVX2 → SSE4.2 → SSE4.1 → SSE2; only features the CPU supports are used.
//!
//! The normalizer additionally delegates to `simd-normalizer`'s 64-byte single-pass
//! SIMD-guided architecture when the `normalize` feature is enabled.
//!
//! ## Panic-Free
//!
//! All public functions are guaranteed not to panic for any input.
//! The normalizer returns `Cow::Owned` only when a transformation is
//! actually applied; it never panics on allocation failure.
//!
//! ## Safety
//!
//! As this crate contains SIMD, it has a lot of unsafe. To ensure safety, we do:
//!
//! - Extensive unit and integration tests for correctness and panic-freedom.
//! - Fuzz testing with `afl.rs`.
//! - Prove code is correct via formal verification using Kani.
//! - Use `miri` to check for undefined behavior.
//! - Extensive property tests via `proptest`.
//!
//! ## `compare_iter`
//!
//! For fully custom natural ordering (different digit bases, whitespace rules, etc.),
//! use `compare_iter`:
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
//! Unicode normalization, and `#![no_std]` support.
//!
//! ## License
//!
//! MIT — see [LICENSE](./LICENSE).

#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_errors_doc)]
#![warn(clippy::missing_panics_doc)]
#![warn(clippy::missing_safety_doc)]

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

#[cfg(test)]
mod tests {
    // On wasm32, `#[test]` invokes wasm_bindgen_test so the same
    // tests run under `wasm-pack test --node`.
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[cfg(feature = "normalize")]
    use super::Normalizer;
    use super::compare;
    use super::compare_ignore_case;
    use super::compare_iter;
    use alloc::string::String;
    use core::cmp::Ordering;

    fn check_total_order(strs: &[&str]) {
        fn ordering_to_op(ord: Ordering) -> &'static str {
            match ord {
                Ordering::Greater => ">",
                Ordering::Equal => "=",
                Ordering::Less => "<",
            }
        }

        for (i, &x) in strs.iter().enumerate() {
            for (j, &y) in strs.iter().enumerate() {
                let actual = compare(x, y);
                let expected = i.cmp(&j);
                assert!(
                    actual == expected,
                    "expected x {} y, got x {} y (x = `{x}`, y = `{y}`)",
                    ordering_to_op(expected),
                    ordering_to_op(actual),
                );
            }
        }
    }

    #[test]
    fn test_numeric() {
        check_total_order(&["a", "a0", "a1", "a1a", "a1b", "a2", "a10", "a20"]);
    }

    #[test]
    fn test_multiple_parts() {
        check_total_order(&["x2-g8", "x2-y7", "x2-y8", "x8-y8"]);
    }

    #[test]
    fn test_leading_zeroes() {
        check_total_order(&["1.001", "1.002", "1.010", "1.02", "1.1", "1.3"]);
    }

    #[test]
    fn test_longer() {
        check_total_order(&[
            "1-02",
            "1-2",
            "1-20",
            "10-20",
            "fred",
            "jane",
            "pic1",
            "pic2",
            "pic2a",
            "pic3",
            "pic4",
            "pic4   alpha",
            "pic 4 else",
            "pic4  last",
            "pic5",
            "pic5.07",
            "pic5.08",
            "pic5.13",
            "pic5.113",
            "pic 5 something",
            "pic 6",
            "pic   7",
            "pic100",
            "pic100a",
            "pic120",
            "pic121",
            "pic2000",
            "tom",
            "x2-g8",
            "x2-y7",
            "x2-y8",
            "x8-y8",
        ]);
    }

    #[test]
    fn test_identical_strings() {
        assert_eq!(compare("hello", "hello"), Ordering::Equal);
        assert_eq!(compare("", ""), Ordering::Equal);
        assert_eq!(compare("123", "123"), Ordering::Equal);
    }

    #[test]
    fn test_empty_strings() {
        assert_eq!(compare("", "a"), Ordering::Less);
        assert_eq!(compare("a", ""), Ordering::Greater);
    }

    #[test]
    fn test_non_ascii() {
        assert_eq!(compare("café", "café"), Ordering::Equal);
        assert_eq!(compare("café", "cafe"), Ordering::Greater);
        assert_eq!(compare("cafe", "café"), Ordering::Less);
    }

    #[test]
    fn test_ignore_case() {
        assert_eq!(compare_ignore_case("ABC", "abc"), Ordering::Equal);
        assert_eq!(compare_ignore_case("AbC", "aBc"), Ordering::Equal);
        assert_eq!(compare_ignore_case("ABC", "abd"), Ordering::Less);
        assert_eq!(compare_ignore_case("ABC", "ABB"), Ordering::Greater);
    }

    #[test]
    fn test_ignore_case_numeric() {
        assert_eq!(compare_ignore_case("a10", "A2"), Ordering::Greater);
        assert_eq!(compare_ignore_case("A2", "a10"), Ordering::Less);
        assert_eq!(compare_ignore_case("pic10", "PIC2"), Ordering::Greater);
    }

    #[test]
    fn test_leading_zeros_ordering() {
        assert_eq!(compare("015", "12"), Ordering::Less);
        assert_eq!(compare("12", "015"), Ordering::Greater);
        assert_eq!(compare("0015", "015"), Ordering::Less);
    }

    #[test]
    fn test_same_pointer_optimization() {
        let s = String::from("some fairly long string that might be interned");
        let r = &s;
        assert_eq!(compare(r, &s), Ordering::Equal);
    }

    #[test]
    fn test_whitespace_skipping() {
        assert_eq!(compare("pic4   alpha", "pic 4 else"), Ordering::Less);
        assert_eq!(compare("pic 4 else", "pic4   alpha"), Ordering::Greater);
        assert_eq!(compare("pic 4 else", "pic4  last"), Ordering::Less);
        assert_eq!(compare("pic 5 something", "pic 6"), Ordering::Less);
    }

    #[test]
    fn test_digit_vs_non_digit() {
        assert_eq!(compare("a", "1"), Ordering::Greater);
        assert_eq!(compare("1", "a"), Ordering::Less);
    }

    #[test]
    fn test_long_digit_runs() {
        assert_eq!(compare("123456789", "123456788"), Ordering::Greater);
        assert_eq!(compare("99999", "100000"), Ordering::Less);
        assert_eq!(compare("100000", "99999"), Ordering::Greater);
    }

    #[test]
    fn test_very_long_prefix() {
        let a = "abcdefghijklmnopqrstuvwxyz00001";
        let b = "abcdefghijklmnopqrstuvwxyz00002";
        assert_eq!(compare(a, b), Ordering::Less);
    }

    #[test]
    fn test_whitespace_variants() {
        assert_eq!(compare("\ta", "a"), Ordering::Equal);
        assert_eq!(compare("a\tb", "ab"), Ordering::Equal);
        assert_eq!(compare("a\nb", "a\tb"), Ordering::Equal);
    }

    #[test]
    fn test_compare_iter() {
        let result = compare_iter(
            "a10".chars(),
            "a2".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Greater);

        let result = compare_iter(
            "a2".chars(),
            "a10".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);

        let result = compare_iter(
            "abc".chars(),
            "abc".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Equal);
    }

    // ── compare_ignore_case edge cases ───────────────────────────────

    #[test]
    fn test_ignore_case_mixed_ascii_non_ascii() {
        // One ASCII, one non-ASCII — byte order decides
        assert_eq!(compare_ignore_case("a", "é"), Ordering::Less);
        assert_eq!(compare_ignore_case("é", "a"), Ordering::Greater);
    }

    #[test]
    fn test_ignore_case_both_non_ascii_different_lead_byte() {
        // Ω (U+03A9, 0xCE 0xA9) vs ω (U+03C9, 0xCF 0x89).
        // Different leading bytes, both ≥128.  Decoded codepoints
        // case-fold to the same character.
        assert_eq!(compare_ignore_case("Ω", "ω"), Ordering::Equal);
        assert_eq!(compare_ignore_case("ω", "Ω"), Ordering::Equal);
    }

    #[test]
    fn test_ignore_case_non_ascii_both_lt() {
        // é (U+00E9, 0xC3 0xA9) < あ (U+3042, 0xE3 0x81 0x82).
        // Different leading bytes, different codepoints.
        assert_eq!(compare_ignore_case("é", "あ"), Ordering::Less);
        assert_eq!(compare_ignore_case("あ", "é"), Ordering::Greater);
    }

    #[test]
    fn test_ignore_case_non_ascii_numeric() {
        // Non-ASCII prefix with numeric runs (non-ASCII bytes are identical on both).
        assert_eq!(compare_ignore_case("café10", "café2"), Ordering::Greater);
        assert_eq!(compare_ignore_case("café2", "café10"), Ordering::Less);
    }

    #[test]
    fn test_ignore_case_whitespace_non_ascii() {
        // Whitespace skip with non-ASCII content (identical non-ASCII bytes).
        assert_eq!(compare_ignore_case("Café 10", "café 2"), Ordering::Greater);
        assert_eq!(compare_ignore_case("café 2", "Café 10"), Ordering::Less);
    }

    #[test]
    fn test_ignore_case_non_ascii_leading_zeros() {
        // Left-aligned zeros after identical non-ASCII prefix.
        assert_eq!(compare_ignore_case("Café015", "café12"), Ordering::Less);
        assert_eq!(compare_ignore_case("café12", "Café015"), Ordering::Greater);
    }

    // ── compare_iter edge cases ─────────────────────────────────────

    #[test]
    fn test_compare_iter_left_aligned_zeros() {
        // Left-aligned: first digit is 0 → compare char-by-char
        let result = compare_iter(
            "015".chars(),
            "12".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_compare_iter_right_aligned_accum() {
        // Right-aligned: lastcmp accumulates over digit run
        // "1243" vs "1234": 1==1, 2==2, 4>3, then 3>_ → Greater
        let result = compare_iter(
            "1243".chars(),
            "1234".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Greater);
    }

    #[test]
    fn test_compare_iter_right_aligned_tie() {
        // Same length, same digits → Equal
        let result = compare_iter(
            "1234".chars(),
            "1234".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Equal);
    }

    #[test]
    fn test_compare_iter_left_longer() {
        // Left has more chars after digits
        let result = compare_iter(
            "abc".chars(),
            "ab".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Greater);

        let result = compare_iter(
            "ab".chars(),
            "abc".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_compare_iter_empty() {
        let result = compare_iter(
            "".chars(),
            "".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Equal);

        let result = compare_iter(
            "".chars(),
            "a".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_compare_iter_skip_whitespace() {
        // Whitespace skip between non-digits
        let result = compare_iter(
            "a b".chars(),
            "ab".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Equal);
    }

    #[test]
    fn test_compare_iter_zero_length_digits() {
        // to_digit returns 0, triggers left-aligned path
        let result = compare_iter(
            "a0b".chars(),
            "a00b".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        // 0 == 0, then 0 < 0 (both digits), then run lengths differ
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_compare_iter_right_aligned_longer_run_wins() {
        // Right-aligned, same prefix but one run is longer
        let result = compare_iter(
            "a123".chars(),
            "a12".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Greater);

        let result = compare_iter(
            "a12".chars(),
            "a123".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_compare_iter_left_aligned_zero_vs_zeros() {
        // Left-aligned zeros: "0" vs "00"
        let result = compare_iter(
            "0".chars(),
            "00".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);
    }

    #[test]
    fn test_compare_iter_skip_at_start() {
        // Whitespace at start of one string
        let result = compare_iter(
            " a".chars(),
            "a".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Equal);

        // Whitespace on both sides
        let result = compare_iter(
            "  ".chars(),
            "".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Equal);
    }

    #[test]
    fn test_compare_iter_right_aligned_then_non_digit() {
        // Equal right-aligned digit run, then non-digit decides
        let result = compare_iter(
            "a123b".chars(),
            "a123c".chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        assert_eq!(result, Ordering::Less);
    }

    // ── compare edge cases ──────────────────────────────────────────

    #[test]
    fn test_whitespace_at_start() {
        assert_eq!(compare("  abc", "abc"), Ordering::Equal);
        assert_eq!(compare("abc", "  abc"), Ordering::Equal);
        assert_eq!(compare("\t\nabc", "abc"), Ordering::Equal);
    }

    #[test]
    fn test_digit_vs_non_digit_with_ws() {
        // Whitespace before digit-vs-letter boundary
        assert_eq!(compare(" 1a", "a"), Ordering::Less);
    }

    #[test]
    fn test_compare_left_aligned_zero_varying_runs() {
        // "0015" vs "015" — left-aligned with the second digit 0 == 0,
        // then left has more digits
        assert_eq!(compare("0015", "015"), Ordering::Less);
        assert_eq!(compare("015", "0015"), Ordering::Greater);
    }

    #[test]
    fn test_compare_word_at_a_time_diff() {
        // Equal-length run that differs in the second u64 chunk
        assert_eq!(
            compare("12345678901234567890", "12345678901234567891"),
            Ordering::Less
        );
    }

    #[test]
    fn test_compare_left_aligned_zeros_equal_run() {
        assert_eq!(compare("000", "000"), Ordering::Equal);
        assert_eq!(compare("00", "00"), Ordering::Equal);
    }

    #[test]
    fn test_compare_ignore_case_left_aligned_zeros_equal_run() {
        // Same for compare_ignore_case
        assert_eq!(compare_ignore_case("000", "000"), Ordering::Equal);
        assert_eq!(compare_ignore_case("ABC000", "abc000"), Ordering::Equal);
    }

    #[test]
    fn test_compare_ignore_case_left_aligned_zero_mixed() {
        // la0 true, lb0 false: `||` vs `&&` in left-aligned trigger
        assert_eq!(compare_ignore_case("015", "12"), Ordering::Less);
        assert_eq!(compare_ignore_case("12", "015"), Ordering::Greater);
    }

    #[test]
    fn test_compare_same_pointer() {
        assert_eq!(compare("ab", "ba"), Ordering::Less);
    }

    #[test]
    fn test_compare_ignore_case_da2_or() {
        assert_eq!(compare_ignore_case("00a", "000x"), Ordering::Less);
    }

    #[test]
    fn test_compare_ignore_case_xor_and() {
        assert_eq!(compare_ignore_case("22222222", "22232222"), Ordering::Less);
    }

    #[test]
    fn test_compare_ignore_case_diff_eq() {
        assert_eq!(
            compare_ignore_case("22222222x", "22232222x"),
            Ordering::Less
        );
    }

    #[test]
    fn test_compare_same_allocation_diff_len() {
        let s = String::from("ab");
        assert_eq!(compare(&s[..1], &s[..2]), Ordering::Less);

        // Also test the compare_ignore_case codepath at line 13.
        assert_eq!(compare_ignore_case(&s[..1], &s[..2]), Ordering::Less);
    }

    #[test]
    fn test_compare_last_eq_digit() {
        assert_eq!(compare("12345678a", "123456789a"), Ordering::Less);
    }

    #[test]
    fn test_compare_da2_or() {
        assert_eq!(compare("00a", "000x"), Ordering::Less);
    }

    #[test]
    fn test_compare_whitespace_pb_bound() {
        let v = alloc::vec![b' ', b' ', b' '];
        let right = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
        assert_eq!(compare("a", right), Ordering::Greater);
    }

    #[test]
    fn test_compare_da2_bound() {
        let v = alloc::vec![b'0', b'0', b'2'];
        let left = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
        assert_eq!(compare(left, "00"), Ordering::Equal);
    }

    #[test]
    fn test_compare_db2_bound() {
        let v = alloc::vec![b'0', b'0', b'2'];
        let right = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
        assert_eq!(compare("00", right), Ordering::Equal);
    }

    #[test]
    fn test_compare_pa_scan_bound() {
        let v = alloc::vec![b'0', b'2'];
        let left = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
        assert_eq!(compare(left, "0a"), Ordering::Less);
    }

    #[test]
    fn test_compare_pb_scan_bound() {
        let v = alloc::vec![b'0', b'2'];
        let right = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
        assert_eq!(compare("0a", right), Ordering::Greater);
    }

    #[test]
    fn test_compare_ka_eq() {
        assert_eq!(compare("01a", "01b"), Ordering::Less);
    }

    #[test]
    fn test_compare_word_at_a_time_tail() {
        // Tail bytes (0–7) after u64 chunk comparison
        assert_eq!(compare("12345678", "12345679"), Ordering::Less);
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_normalizer_nfd() {
        // U+00E9 (NFC) → U+0065 U+0301 (NFD).
        let nfd = Normalizer::default().nfd().normalize("\u{e9}");
        let raw = Normalizer::default().normalize("\u{e9}");
        assert_ne!(
            nfd.as_ref(),
            raw.as_ref(),
            "nfd() should decompose precomposed characters"
        );
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_normalizer_nfkc() {
        // U+2460 (CIRCLED DIGIT ONE) NFKC → "1"
        let nfkc = Normalizer::default().nfkc().normalize("\u{2460}");
        let raw = Normalizer::default().normalize("\u{2460}");
        assert_ne!(
            nfkc.as_ref(),
            raw.as_ref(),
            "nfkc() should compatibility-decompose"
        );
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_normalizer_nfkd() {
        let nfkd = Normalizer::default().nfkd().normalize("\u{2460}");
        let raw = Normalizer::default().normalize("\u{2460}");
        assert_ne!(
            nfkd.as_ref(),
            raw.as_ref(),
            "nfkd() should compatibility-decompose"
        );
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_normalizer_case_sensitive() {
        let norm = Normalizer::default().nfc().case_fold().case_sensitive();
        assert_eq!(
            norm.compare("\u{e9}", "e\u{301}"),
            Ordering::Equal,
            "case_sensitive should preserve NFC setting"
        );
    }

    #[test]
    fn test_compare_ignore_case_pa_scan_bound() {
        let v = alloc::vec![b'0', b'2'];
        let left = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
        assert_eq!(compare_ignore_case(left, "0a"), Ordering::Less);
    }

    #[test]
    fn test_compare_ignore_case_pb_scan_bound() {
        let v = alloc::vec![b'0', b'2'];
        let right = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
        assert_eq!(compare_ignore_case("0a", right), Ordering::Greater);
    }

    #[test]
    fn test_compare_ignore_case_db2_or() {
        assert_eq!(compare_ignore_case("000x", "00a"), Ordering::Greater);
    }

    #[test]
    fn test_compare_ignore_case_xor_div() {
        assert_eq!(compare_ignore_case("01345678", "02345678"), Ordering::Less);
    }

    #[test]
    fn test_compare_ignore_case_xor_mul() {
        assert_eq!(
            compare_ignore_case("266666666666666612345678", "666666666666666612345678",),
            Ordering::Less,
        );
    }

    #[test]
    fn test_compare_ignore_case_pbr_bound() {
        let v = alloc::vec![b'a', b'b', b'c', b'3', b'9', b'x'];
        let right = unsafe { core::str::from_utf8_unchecked(&v[..4]) };
        assert_eq!(compare_ignore_case("abc29", right), Ordering::Greater);
    }

    #[test]
    fn test_compare_ignore_case_ws_pb_bound() {
        let v = alloc::vec![b'a', b' ', b' '];
        let right = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
        assert_eq!(compare_ignore_case("ab", right), Ordering::Greater);
    }
}
