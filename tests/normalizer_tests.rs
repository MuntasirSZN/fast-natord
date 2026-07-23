//! Integration tests for `fast-natord::Normalizer` public API.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

extern crate alloc;

use alloc::borrow::Cow;
use core::cmp::Ordering;
use fast_natord::{Normalizer, compare_normalized};

#[test]
fn test_noop_normalizer() {
    let norm = Normalizer::new();
    assert_eq!(norm.compare("hello", "hello"), Ordering::Equal);
    assert_eq!(norm.compare("abc", "abd"), Ordering::Less);
}

#[test]
fn test_nfc_ascii_borrowed() {
    let norm = Normalizer::new().nfc();
    let n = norm.normalize("hello");
    assert!(matches!(n, Cow::Borrowed(_)));
}

#[test]
fn test_ascii_only_borrowed_when_no_fold_needed() {
    let norm = Normalizer::new().case_ascii_only();
    let n = norm.normalize("hello");
    assert!(matches!(n, Cow::Borrowed(_)));
}

#[test]
fn test_ascii_case_fold_ascii_only() {
    let norm = Normalizer::new().case_ascii_only();
    let n = norm.normalize("HELLO");
    assert!(matches!(n, Cow::Owned(_)));
    assert_eq!(n.as_ref(), "hello");
}

#[test]
fn test_ascii_case_fold() {
    let norm = Normalizer::new().case_fold();
    let n = norm.normalize("HELLO");
    assert!(matches!(n, Cow::Owned(_)));
    assert_eq!(n.as_ref(), "hello");
}

#[test]
#[cfg(feature = "normalize")]
fn test_nfc_decomposed() {
    let norm = Normalizer::new().nfc();
    let n = norm.normalize("e\u{0301}");
    assert_eq!(n.as_ref(), "\u{00E9}");
}

#[test]
#[cfg(feature = "normalize")]
fn test_compare_normalized_nfc() {
    let norm = Normalizer::new().nfc();
    assert_eq!(norm.compare("\u{00E9}", "e\u{0301}"), Ordering::Equal);
}

#[test]
#[cfg(feature = "normalize")]
fn test_compare_normalized_nfc_case_fold() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("\u{00C9}", "e\u{0301}"), Ordering::Equal);
}

#[test]
fn test_case_fold_equivalent() {
    let norm = Normalizer::new().case_fold();
    assert_eq!(norm.compare("ABC", "abc"), Ordering::Equal);
    assert_eq!(norm.compare("ABC", "abd"), Ordering::Less);
}

#[test]
fn test_compare_normalized_numeric() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("pic10", "pic2"), Ordering::Greater);
    assert_eq!(norm.compare("pic2", "pic10"), Ordering::Less);
}

#[test]
fn test_ascii_only_keeps_non_ascii() {
    let norm = Normalizer::new().case_ascii_only();
    let n = norm.normalize("Café");
    // non-ASCII 'é' is left unchanged
    assert_eq!(n.as_ref(), "café");
}

#[test]
fn test_compare_empty_strings() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("", ""), Ordering::Equal);
    assert_eq!(norm.compare("", "a"), Ordering::Less);
    assert_eq!(norm.compare("a", ""), Ordering::Greater);
}

#[test]
fn test_nfc_idempotent() {
    let norm = Normalizer::new().nfc();
    let once = norm.normalize("hello");
    let twice = norm.normalize(once.as_ref());
    assert_eq!(once.as_ref(), twice.as_ref());
}

#[test]
fn test_case_fold_idempotent() {
    let norm = Normalizer::new().case_fold();
    let once = norm.normalize("HelloWorld");
    let twice = norm.normalize(once.as_ref());
    assert_eq!(once.as_ref(), twice.as_ref());
}

#[test]
fn test_leading_zeros_with_normalizer() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("015", "12"), Ordering::Less);
    assert_eq!(norm.compare("12", "015"), Ordering::Greater);
}

#[test]
fn test_whitespace_with_normalizer() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("pic4   alpha", "pic 4 else"), Ordering::Less);
}

#[test]
fn test_long_digit_runs_normalized() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("123456789", "123456788"), Ordering::Greater);
}

#[test]
fn test_compare_normalized_mixed() {
    let norm = Normalizer::new().nfc().case_fold();
    assert_eq!(norm.compare("Café10", "café2"), Ordering::Greater);
    assert_eq!(norm.compare("café2", "Café10"), Ordering::Less);
}

#[test]
fn test_compare_normalized_convenience_function() {
    assert_eq!(compare_normalized("ABC", "abc"), Ordering::Equal);
    assert_eq!(compare_normalized("pic10", "pic2"), Ordering::Greater);
}

// ── Additional integration tests from integration_tests.rs ───────────

#[test]
#[cfg(feature = "normalize")]
fn test_normalizer_nfd() {
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
