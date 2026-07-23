//! Integration tests for `fast-natord::compare` (case-sensitive).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
extern crate alloc;

use alloc::string::String;
use core::cmp::Ordering;
use fast_natord::compare;

mod common;

#[test]
fn test_numeric() {
    common::check_total_order(&["a", "a0", "a1", "a1a", "a1b", "a2", "a10", "a20"]);
}

#[test]
fn test_multiple_parts() {
    common::check_total_order(&["x2-g8", "x2-y7", "x2-y8", "x8-y8"]);
}

#[test]
fn test_leading_zeroes() {
    common::check_total_order(&["1.001", "1.002", "1.010", "1.02", "1.1", "1.3"]);
}

#[test]
fn test_longer() {
    common::check_total_order(&[
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

// ── Edge cases ───────────────────────────────────────────────────

#[test]
fn test_whitespace_at_start() {
    assert_eq!(compare("  abc", "abc"), Ordering::Equal);
    assert_eq!(compare("abc", "  abc"), Ordering::Equal);
    assert_eq!(compare("\t\nabc", "abc"), Ordering::Equal);
}

#[test]
fn test_digit_vs_non_digit_with_ws() {
    assert_eq!(compare(" 1a", "a"), Ordering::Less);
}

#[test]
fn test_compare_left_aligned_zero_varying_runs() {
    assert_eq!(compare("0015", "015"), Ordering::Less);
    assert_eq!(compare("015", "0015"), Ordering::Greater);
}

#[test]
fn test_compare_word_at_a_time_diff() {
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
fn test_compare_same_pointer() {
    assert_eq!(compare("ab", "ba"), Ordering::Less);
}

#[test]
fn test_compare_same_allocation_diff_len() {
    let s = String::from("ab");
    assert_eq!(compare(&s[..1], &s[..2]), Ordering::Less);
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
    let v = *b"   ";
    let right = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
    assert_eq!(compare("a", right), Ordering::Greater);
}

#[test]
fn test_compare_da2_bound() {
    let v = *b"002";
    let left = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
    assert_eq!(compare(left, "00"), Ordering::Equal);
}

#[test]
fn test_compare_db2_bound() {
    let v = *b"002";
    let right = unsafe { core::str::from_utf8_unchecked(&v[..2]) };
    assert_eq!(compare("00", right), Ordering::Equal);
}

#[test]
fn test_compare_pa_scan_bound() {
    let v = *b"02";
    let left = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
    assert_eq!(compare(left, "0a"), Ordering::Less);
}

#[test]
fn test_compare_pb_scan_bound() {
    let v = *b"02";
    let right = unsafe { core::str::from_utf8_unchecked(&v[..1]) };
    assert_eq!(compare("0a", right), Ordering::Greater);
}

#[test]
fn test_compare_ka_eq() {
    assert_eq!(compare("01a", "01b"), Ordering::Less);
}

#[test]
fn test_compare_word_at_a_time_tail() {
    assert_eq!(compare("12345678", "12345679"), Ordering::Less);
}
