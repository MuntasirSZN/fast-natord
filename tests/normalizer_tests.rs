//! Integration tests for `fast_natord::Normalizer` public API.
//!
//! These tests only use the public API and are placed in `tests/`
//! so they run as integration tests (separate crate).
//!
//! On wasm32, `#[test]` invokes wasm_bindgen_test so the same
//!
//! tests run under `wasm-pack test --node`.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
extern crate alloc;

use alloc::borrow::Cow;
use core::cmp::Ordering;
#[cfg(feature = "normalize")]
use fast_natord::{CaseMode, Normalization};
use fast_natord::{Normalizer, compare_normalized};

#[test]
fn test_noop_normalizer() {
    let n = Normalizer::new();
    assert_eq!(n.normalize("hello"), Cow::Borrowed("hello"));
    assert_eq!(n.compare("a", "b"), Ordering::Less);
    assert_eq!(n.compare("b", "a"), Ordering::Greater);
    assert_eq!(n.compare("a", "a"), Ordering::Equal);
}

#[test]
fn test_nfc_ascii_borrowed() {
    // All-ASCII → NFC is a no-op → borrowed.
    let n = Normalizer::new().nfc();
    assert_eq!(n.normalize("hello world"), Cow::Borrowed("hello world"));
}

#[test]
fn test_ascii_only_borrowed_when_no_fold_needed() {
    // Mixed ASCII/non-ASCII where all ASCII is already lowercase:
    // fold_ascii should return Borrowed (no allocation).
    let n = Normalizer::new().case_ascii_only();
    let result = n.normalize("hello\u{00E9}");
    match result {
        Cow::Borrowed(s) => assert_eq!(s, "hello\u{00E9}"),
        _ => panic!("expected Borrowed but got Owned"),
    }
}

#[test]
fn test_ascii_case_fold_ascii_only() {
    let n = Normalizer::new().case_ascii_only();
    let norm = n.normalize("HelloWorld");
    assert_eq!(norm, "helloworld");
}

#[test]
fn test_ascii_case_fold() {
    let n = Normalizer::new().case_fold();
    let norm = n.normalize("Hello World");
    assert_eq!(norm, "hello world");
}

#[test]
#[cfg(feature = "normalize")]
fn test_nfc_decomposed() {
    // É (U+00C9) decomposed = E (U+0045) + combining acute (U+0301).
    // NFC should recompose to U+00C9.
    let n = Normalizer::new().nfc();
    let norm = n.normalize("E\u{301}");
    assert_eq!(norm, "\u{00C9}");
}

#[test]
#[cfg(feature = "normalize")]
fn test_compare_normalized_nfc() {
    // NFC makes é == e + combining accent.
    let n = Normalizer::new().nfc();
    assert_eq!(n.compare("\u{00E9}", "e\u{301}"), Ordering::Equal);
}

#[test]
#[cfg(feature = "normalize")]
fn test_compare_normalized_nfc_case_fold() {
    let n = Normalizer::new().nfc().case_fold();
    // Case folding + NFC: uppercase with composition vs lowercase decomposed.
    assert_eq!(
        n.compare("Caf\u{00C9}", "caf\u{0065}\u{0301}"),
        Ordering::Equal
    );
}

#[test]
fn test_case_fold_equivalent() {
    let n = Normalizer::new().case_fold();
    assert_eq!(n.compare("ABC", "abc"), Ordering::Equal);
    assert_eq!(n.compare("ABC", "abd"), Ordering::Less);
}

#[test]
fn test_compare_normalized_numeric() {
    let n = Normalizer::new().nfc().case_fold();
    assert_eq!(n.compare("pic10", "pic2"), Ordering::Greater);
    assert_eq!(n.compare("pic2", "pic10"), Ordering::Less);
}

#[test]
fn test_ascii_only_keeps_non_ascii() {
    let n = Normalizer::new().case_ascii_only();
    let result = n.normalize("ABC\u{00E9}");
    // Uppercase ASCII → lowercased; non-ASCII → untouched (é stays é).
    assert_eq!(result, "abc\u{00E9}");
}

#[test]
fn test_compare_empty_strings() {
    let n = Normalizer::new().nfc().case_fold();
    assert_eq!(n.compare("", ""), Ordering::Equal);
    assert_eq!(n.compare("", "a"), Ordering::Less);
    assert_eq!(n.compare("a", ""), Ordering::Greater);
}

#[test]
fn test_nfc_idempotent() {
    let n = Normalizer::new().nfc();
    let once = n.normalize("caf\u{00E9}").into_owned();
    let twice = n.normalize(&once);
    assert_eq!(once, twice);
}

#[test]
fn test_case_fold_idempotent() {
    let n = Normalizer::new().case_fold();
    let once = n.normalize("Hello Σ 123").into_owned();
    let twice = n.normalize(&once);
    assert_eq!(once, twice);
}

#[test]
fn test_leading_zeros_with_normalizer() {
    let n = Normalizer::new().nfc().case_sensitive();
    assert_eq!(n.compare("015", "12"), Ordering::Less);
    assert_eq!(n.compare("12", "015"), Ordering::Greater);
    assert_eq!(n.compare("0015", "015"), Ordering::Less);
}

#[test]
fn test_whitespace_with_normalizer() {
    let n = Normalizer::new().nfc().case_sensitive();
    assert_eq!(n.compare("pic4   alpha", "pic 4 else"), Ordering::Less);
    assert_eq!(n.compare("pic 4 else", "pic4  last"), Ordering::Less);
}

#[test]
fn test_long_digit_runs_normalized() {
    let n = Normalizer::new().nfc().case_fold();
    assert_eq!(n.compare("123456789", "123456788"), Ordering::Greater);
    assert_eq!(n.compare("99999", "100000"), Ordering::Less);
}

#[test]
fn test_compare_normalized_mixed() {
    let n = Normalizer::new().nfc().case_fold();
    // Various real-world-like comparisons.
    assert_eq!(n.compare("RFC 2", "rfc 10"), Ordering::Less);
    assert_eq!(n.compare("rfc 10", "RFC 2"), Ordering::Greater);
    assert_eq!(n.compare("Pic 5", "pic 5"), Ordering::Equal);
}

#[test]
fn test_compare_normalized_convenience_function() {
    // Test the convenience function `compare_normalized`
    assert_eq!(compare_normalized("ABC", "abc"), Ordering::Equal);
    assert_eq!(compare_normalized("pic10", "pic2"), Ordering::Greater);
}
