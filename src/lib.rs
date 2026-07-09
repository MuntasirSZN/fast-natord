//! # `fast-natord`
//!
//! Natural ordering for Rust.  Compares strings with awareness of numeric
//! subsequences so that `"rfc2"` precedes `"rfc10"`.
//!
//! ```rust
//! let mut files = vec!["rfc2086.txt", "rfc822.txt", "rfc1.txt"];
//! files.sort_by(|&a, &b| fast_natord::compare(a, b));
//! assert_eq!(files, ["rfc1.txt", "rfc822.txt", "rfc2086.txt"]);
//! ```
//!
//! # Panic-free
//!
//! All public functions are guaranteed not to panic for any input.
//! Fallible operations return `Result`.
//!
//! # `no_std`
//!
//! This crate is `#![no_std]` by default.  The core API uses
//! `core::cmp::Ordering` and `&str` / `&[u8]` arguments.
//!
//! # Locale support (optional)
//!
//! Enable the `locale` feature to add locale-aware comparison using
//! ICU4X collation:
//!
//! ```toml
//! fast-natord = { version = "0.1", features = ["locale"] }
//! ```
//!
//! See the [`collator`] module for details.

#![no_std]
#![warn(missing_docs)]
#![warn(clippy::missing_safety_doc)]

extern crate alloc;

mod byte_utils;
mod compare;
mod compare_ignore_case;
mod compare_iter;
mod unicode;

#[cfg(feature = "locale")]
pub mod collator;

#[cfg(feature = "locale")]
pub use collator::{Collator, CollatorError};

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

#[cfg(test)]
mod tests {
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
}
