//! Integration tests for `fast-natord::compare_ignore_case`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
extern crate alloc;

use core::cmp::Ordering;
use fast_natord::compare_ignore_case;

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
fn test_compare_ignore_case_left_aligned_zeros_equal_run() {
    assert_eq!(compare_ignore_case("000", "000"), Ordering::Equal);
    assert_eq!(compare_ignore_case("ABC000", "abc000"), Ordering::Equal);
}

#[test]
fn test_compare_ignore_case_left_aligned_zero_mixed() {
    assert_eq!(compare_ignore_case("015", "12"), Ordering::Less);
    assert_eq!(compare_ignore_case("12", "015"), Ordering::Greater);
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
        compare_ignore_case("266666666666666612345678", "666666666666666612345678"),
        Ordering::Less,
    );
}

#[test]
fn test_compare_same_allocation_diff_len() {
    let s = alloc::string::String::from("ab");
    assert_eq!(compare_ignore_case(&s[..1], &s[..2]), Ordering::Less);
}

// ── Non-ASCII edge cases ──────────────────────────────────────────

#[test]
fn test_ignore_case_mixed_ascii_non_ascii() {
    assert_eq!(compare_ignore_case("a", "é"), Ordering::Less);
    assert_eq!(compare_ignore_case("é", "a"), Ordering::Greater);
}

#[test]
fn test_ignore_case_both_non_ascii_different_lead_byte() {
    assert_eq!(compare_ignore_case("Ω", "ω"), Ordering::Equal);
    assert_eq!(compare_ignore_case("ω", "Ω"), Ordering::Equal);
}

#[test]
fn test_ignore_case_non_ascii_both_lt() {
    assert_eq!(compare_ignore_case("é", "あ"), Ordering::Less);
    assert_eq!(compare_ignore_case("あ", "é"), Ordering::Greater);
}

#[test]
fn test_ignore_case_non_ascii_numeric() {
    assert_eq!(compare_ignore_case("café10", "café2"), Ordering::Greater);
    assert_eq!(compare_ignore_case("café2", "café10"), Ordering::Less);
}

#[test]
fn test_ignore_case_whitespace_non_ascii() {
    assert_eq!(compare_ignore_case("Café 10", "café 2"), Ordering::Greater);
    assert_eq!(compare_ignore_case("café 2", "Café 10"), Ordering::Less);
}

#[test]
fn test_ignore_case_non_ascii_leading_zeros() {
    assert_eq!(compare_ignore_case("Café015", "café12"), Ordering::Less);
    assert_eq!(compare_ignore_case("café12", "Café015"), Ordering::Greater);
}

// ── Boundary edge cases ───────────────────────────────────────────

#[test]
fn test_compare_ignore_case_pa_scan_bound() {
    let v = *b"02";
    let left = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
    assert_eq!(compare_ignore_case(left, "0a"), Ordering::Less);
}

#[test]
fn test_compare_ignore_case_pb_scan_bound() {
    let v = *b"02";
    let right = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
    assert_eq!(compare_ignore_case("0a", right), Ordering::Greater);
}

#[test]
fn test_compare_ignore_case_pbr_bound() {
    let v = *b"abc39x";
    let right = unsafe { core::str::from_utf8_unchecked(&v[..4]) };
    assert_eq!(compare_ignore_case("abc29", right), Ordering::Greater);
}

#[test]
fn test_compare_ignore_case_ws_pb_bound() {
    let v = *b"a  ";
    let right = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
    assert_eq!(compare_ignore_case("ab", right), Ordering::Greater);
}

// ── Leading-zero long-run SIMD path ────────────────────────────────

#[test]
fn test_ic_left_aligned_long_run() {
    assert_eq!(
        compare_ignore_case(" 0X0123456789012345ABCDEFG", " 0X1123456789012345ABCDEFG",),
        Ordering::Less
    );
}

#[test]
fn test_ic_left_aligned_a_longer_run() {
    assert_eq!(compare_ignore_case(" 0X001A", " 0X00B"), Ordering::Greater);
}

#[test]
fn test_ic_left_aligned_b_longer_run() {
    assert_eq!(compare_ignore_case(" 0X00A", " 0X001B"), Ordering::Less);
}

#[test]
fn test_ic_left_aligned_long_run_equal_digits() {
    // Equal-length, equal digits → word-at-a-time None, continue past.
    assert_eq!(
        compare_ignore_case(" 0X0012345678901234xAAAA", " 0X0012345678901234xBBBB",),
        Ordering::Less
    );
}

#[test]
fn test_ic_right_aligned_different_lengths() {
    assert_eq!(
        compare_ignore_case(" 0X12345A", " 0X123B"),
        Ordering::Greater
    );
    assert_eq!(compare_ignore_case(" 0X123A", " 0X12345B"), Ordering::Less);
}

// ── Case-fold non-digit path ───────────────────────────────────────

#[test]
fn test_ic_diff_eq_then_case_fold() {
    // ca == cb (both non-digit) but not whitespace → advance past.
    assert_eq!(compare_ignore_case("a10", "a2"), Ordering::Greater);
}

#[test]
fn test_ic_non_digit_digit_boundary() {
    // ca is digit, cb not digit, and pa is preceded by digit
    assert_eq!(compare_ignore_case(" 1A", " AB"), Ordering::Less);
}
