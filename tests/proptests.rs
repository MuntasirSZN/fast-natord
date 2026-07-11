//! Property-based tests for `fast-natord`.
//!
//! These verify that the natural-ordering comparators satisfy the laws of a
//! total order and maintain documented invariants across a broad range of
//! randomly generated inputs.

#![cfg(test)]

use core::cmp::Ordering;
use fast_natord::{compare, compare_ignore_case, compare_iter, compare_normalized};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

// ── String strategies ────────────────────────────────────────────────

/// Any valid UTF-8 string, 0–64 chars (full Unicode repertoire).
fn any_string() -> impl Strategy<Value = String> {
    prop::collection::vec(proptest::char::any(), 0..64).prop_map(|cs| cs.into_iter().collect())
}

/// ASCII-only string (bytes 0x00–0x7f), 0–64 chars.
fn ascii_string() -> impl Strategy<Value = String> {
    prop::collection::vec(proptest::char::range('\x00', '\x7f'), 0..64)
        .prop_map(|cs| cs.into_iter().collect())
}

/// Printable ASCII (bytes 0x20–0x7e), 0–64 chars.
fn printable_ascii() -> impl Strategy<Value = String> {
    prop::collection::vec(proptest::char::range(' ', '~'), 0..64)
        .prop_map(|cs| cs.into_iter().collect())
}

/// Alphanumeric ASCII (a-zA-Z0-9), 0–64 chars.
/// No whitespace — avoids the compare/compare_iter disagreement on
/// whitespace-between-digits splitting digit-run boundaries.
fn alphanumeric() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(&[
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
            'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
            'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
        ]),
        0..64,
    )
    .prop_map(|cs| cs.into_iter().collect())
}

// ── Total-order helpers ──────────────────────────────────────────────

/// Assert reflexivity, antisymmetry, and the three transitivity cases.
///
/// Returns `Ok(())` when all checks pass, necessary because the
/// `prop_assert_*` macros return `Result`.
fn check_total_order<F>(f: F, a: &str, b: &str, c: &str) -> Result<(), TestCaseError>
where
    F: Fn(&str, &str) -> Ordering,
{
    // Reflexivity
    prop_assert_eq!(f(a, a), Ordering::Equal, "reflexivity a={:?}", a);
    prop_assert_eq!(f(b, b), Ordering::Equal, "reflexivity b={:?}", b);
    prop_assert_eq!(f(c, c), Ordering::Equal, "reflexivity c={:?}", c);

    // Antisymmetry
    let ab = f(a, b);
    let ba = f(b, a);
    prop_assert_eq!(
        ab,
        ba.reverse(),
        "antisymmetry: a={:?} b={:?}  f(a,b)={:?} f(b,a)={:?}",
        a,
        b,
        ab,
        ba
    );

    // Transitivity: if a < b < c then a < c
    let bc = f(b, c);
    let ac = f(a, c);
    if ab == Ordering::Less && bc == Ordering::Less {
        prop_assert_eq!(
            ac,
            Ordering::Less,
            "transitivity (<): a={:?} b={:?} c={:?}",
            a,
            b,
            c
        );
    }
    // Transitivity: if a == b then f(a, c) == f(b, c)
    if ab == Ordering::Equal {
        prop_assert_eq!(
            ac,
            bc,
            "transitivity (== left): a={:?} b={:?} c={:?}",
            a,
            b,
            c
        );
    }
    // Transitivity: if b == c then f(a, b) == f(a, c)
    if bc == Ordering::Equal {
        prop_assert_eq!(
            ab,
            ac,
            "transitivity (== right): a={:?} b={:?} c={:?}",
            a,
            b,
            c
        );
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
//  compare
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_compare_reflexive(s in any_string()) {
        prop_assert_eq!(compare(&s, &s), Ordering::Equal);
    }

    #[test]
    fn prop_compare_antisymmetric(a in any_string(), b in any_string()) {
        let ab = compare(&a, &b);
        let ba = compare(&b, &a);
        prop_assert_eq!(ab, ba.reverse(), "a={:?} b={:?}", a, b);
    }

    #[test]
    fn prop_compare_transitive(a in any_string(), b in any_string(), c in any_string()) {
        let ab = compare(&a, &b);
        let bc = compare(&b, &c);
        let ac = compare(&a, &c);
        if ab == Ordering::Less && bc == Ordering::Less {
            prop_assert_eq!(ac, Ordering::Less,
                "transitivity (<): a={:?} b={:?} c={:?}", a, b, c);
        }
        if ab == Ordering::Equal {
            prop_assert_eq!(ac, bc,
                "transitivity (== left): a={:?} b={:?} c={:?}", a, b, c);
        }
        if bc == Ordering::Equal {
            prop_assert_eq!(ab, ac,
                "transitivity (== right): a={:?} b={:?} c={:?}", a, b, c);
        }
    }
}



// ── Numeric properties ───────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_compare_embedded_numeric(prefix in "[a-z]{0,8}", n1: u64, n2: u64) {
        // Numeric parts compare by natural order when both have the same
        // number of digits (different-length runs compare by run length
        // under right-aligned comparison, which is correct natural
        // ordering but not equivalent to raw numeric comparison).
        prop_assume!(format!("{n1}").len() == format!("{n2}").len());
        let a = format!("{prefix}{n1}");
        let b = format!("{prefix}{n2}");
        let expected = n1.cmp(&n2);
        let actual = compare(&a, &b);
        prop_assert_eq!(actual, expected, "a={:?} b={:?}", a, b);
    }

    #[test]
    fn prop_compare_leading_zero_makes_smaller(prefix in "[a-z]{0,4}", n: u64) {
        // Prepending a '0' to the digit run makes it smaller (except n=0
        // where "00" > "0" under left-aligned comparison).
        prop_assume!(n > 0);
        let base = format!("{prefix}{n}");
        let zero_padded = format!("{prefix}0{n}");
        let cmp = compare(&zero_padded, &base);
        prop_assert_eq!(cmp, Ordering::Less,
            "expected zero-padded < base: zp={:?} base={:?}", zero_padded, base);
    }

    #[test]
    fn prop_compare_right_aligned_nums(a in "[1-9][0-9]{0,10}", b in "[1-9][0-9]{0,10}") {
        // Pure decimal strings without leading zeros compare by numeric value.
        let a_val = a.parse::<u64>().unwrap();
        let b_val = b.parse::<u64>().unwrap();
        let expected = a_val.cmp(&b_val);
        let actual = compare(&a, &b);
        prop_assert_eq!(actual, expected, "a={:?} ({}) b={:?} ({})", a, a_val, b, b_val);
    }
}

// ── Empty string ─────────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_compare_empty_le_any(s in any_string()) {
        let cmp = compare("", &s);
        prop_assert!(cmp != Ordering::Greater, "empty > {:?}", s);
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  compare_ignore_case
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_ignore_case_reflexive(s in any_string()) {
        prop_assert_eq!(compare_ignore_case(&s, &s), Ordering::Equal);
    }
}

// Total-order properties hold for ASCII (the byte-level case-folding path
// is sound for ASCII; non-ASCII has known limitations with shared leading
// bytes, so we restrict to ASCII for the full order check).
proptest! {
    #[test]
    fn prop_ignore_case_ascii_total_order(a in ascii_string(), b in ascii_string(), c in ascii_string()) {
        check_total_order(compare_ignore_case, &a, &b, &c)?;
    }
}

// ── Consistency with compare ─────────────────────────────────────────

proptest! {
    #[test]
    fn prop_ignore_case_matches_compare_lowercased(a in ascii_string(), b in ascii_string()) {
        // For ASCII, lowering both sides then compare should match
        // compare_ignore_case.
        let expected = compare(
            &a.to_ascii_lowercase(),
            &b.to_ascii_lowercase(),
        );
        let actual = compare_ignore_case(&a, &b);
        prop_assert_eq!(actual, expected, "a={:?} b={:?}", a, b);
    }
}

// ── Empty string ─────────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_ignore_case_empty_le_any(s in any_string()) {
        let cmp = compare_ignore_case("", &s);
        prop_assert!(cmp != Ordering::Greater, "empty > {:?}", s);
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  compare_normalized
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_normalized_reflexive(s in any_string()) {
        prop_assert_eq!(compare_normalized(&s, &s), Ordering::Equal);
    }

    #[test]
    fn prop_normalized_matches_ignore_case_ascii(a in ascii_string(), b in ascii_string()) {
        // For ASCII, NFC is the identity and case folding matches
        // to_lowercase, so compare_normalized must agree with
        // compare_ignore_case.
        let expected = compare_ignore_case(&a, &b);
        let actual = compare_normalized(&a, &b);
        prop_assert_eq!(actual, expected, "a={:?} b={:?}", a, b);
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  compare_iter
// ═══════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_compare_iter_matches_compare(a in alphanumeric(), b in alphanumeric()) {
        // Alphanumeric only (no whitespace): `compare` skips whitespace
        // transparently (merging adjacent digit runs), while
        // `compare_iter` reads inside digit-run loops without whitespace
        // skip.  Without whitespace the two agree.
        let expected = compare(&a, &b);
        let actual = compare_iter(
            a.chars(),
            b.chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        prop_assert_eq!(actual, expected, "a={:?} b={:?}", a, b);
    }

    #[test]
    fn prop_compare_iter_identity(a in printable_ascii(), _b in printable_ascii()) {
        let result = compare_iter(
            a.chars(),
            a.chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        );
        prop_assert_eq!(result, Ordering::Equal, "a={:?}", a);
    }
}
