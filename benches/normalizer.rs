//! Benchmarks for `Normalizer` and `compare_normalized`.
//!
//! cargo bench --bench normalizer

use divan::{AllocProfiler, Bencher};

fn main() {
    divan::main();
}

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

use fast_natord::Normalizer;

// ── Data ─────────────────────────────────────────────────────────────

const NORM_ASCII_EQUAL: (&str, &str) = ("hello123", "HELLO123");
const NORM_ASCII_NUMERIC: (&str, &str) = ("pic10", "PIC2");
const NORM_ASCII_DIFFERENT: (&str, &str) = ("apple100", "APPLE20");

/// Precomposed é (U+00E9) vs decomposed e + combining acute (U+0301).
const NORM_DECOMPOSED_EQUAL: (&str, &str) = ("caf\u{00E9}123", "cafe\u{0301}123");

/// Uppercase É precomposed (U+00C9) vs lowercase é decomposed.
const NORM_CASE_DECOMPOSED: (&str, &str) = ("Caf\u{00C9}100", "cafe\u{0301}10");

/// Already NFC, already lowercase.
const NORM_ALREADY_NFC: (&str, &str) = ("caf\u{00E9} 10", "caf\u{00E9} 2");

/// Longer non-ASCII with numeric suffix, mixed case.
const NORM_MIXED_LONG: (&str, &str) = (
    "R\u{00E9}sum\u{00E9} Draft 100",
    "r\u{0065}\u{0301}sum\u{0065}\u{0301} draft 20",
);

/// Strings for the `normalize()` method benchmark.
static ASCII_INPUT: &str = "The quick brown fox jumps over the lazy dog 12345";
static NFC_INPUT: &str = "caf\u{00E9} cr\u{00E8}me br\u{00FB}l\u{00E9}e 12345";
static DECOMPOSED_INPUT: &str = "cafe\u{0301} cre\u{0300}me bru\u{0302}le\u{0301}e 12345";
static UPPER_INPUT: &str = "CAFÉ CRÈME BRÛLÉE 12345";

const IC_EQUAL: (&str, &str) = ("AlphabetSoup0123", "alphabetSoup0123");

// ── Builders ─────────────────────────────────────────────────────────

fn norm_none() -> Normalizer {
    Normalizer::new()
}

fn norm_nfc_fold() -> Normalizer {
    Normalizer::new().nfc().case_fold()
}

fn norm_nfc() -> Normalizer {
    Normalizer::new().nfc()
}

fn norm_fold() -> Normalizer {
    Normalizer::new().case_fold()
}

fn norm_ascii_only() -> Normalizer {
    Normalizer::new().case_ascii_only()
}

// ═══════════════════════════════════════════════════════════════════════
//  ASCII fast path — compare
// ═══════════════════════════════════════════════════════════════════════

#[divan::bench]
fn norm_none_ascii_equal(b: Bencher) {
    let norm = norm_none();
    let (l, r) = NORM_ASCII_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_nfc_fold_ascii_equal(b: Bencher) {
    let norm = norm_nfc_fold();
    let (l, r) = NORM_ASCII_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_nfc_fold_ascii_numeric(b: Bencher) {
    let norm = norm_nfc_fold();
    let (l, r) = NORM_ASCII_NUMERIC;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_nfc_fold_ascii_different(b: Bencher) {
    let norm = norm_nfc_fold();
    let (l, r) = NORM_ASCII_DIFFERENT;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_ascii_only_ascii_equal(b: Bencher) {
    let norm = norm_ascii_only();
    let (l, r) = NORM_ASCII_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_fold_ascii_equal(b: Bencher) {
    let norm = norm_fold();
    let (l, r) = NORM_ASCII_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

// ═══════════════════════════════════════════════════════════════════════
//  Non-ASCII — compare
// ═══════════════════════════════════════════════════════════════════════

#[divan::bench]
fn norm_nfc_decomposed_equal(b: Bencher) {
    let norm = norm_nfc();
    let (l, r) = NORM_DECOMPOSED_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_nfc_fold_decomposed_equal(b: Bencher) {
    let norm = norm_nfc_fold();
    let (l, r) = NORM_DECOMPOSED_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_nfc_fold_case_decomposed(b: Bencher) {
    let norm = norm_nfc_fold();
    let (l, r) = NORM_CASE_DECOMPOSED;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_nfc_fold_mixed_long(b: Bencher) {
    let norm = norm_nfc_fold();
    let (l, r) = NORM_MIXED_LONG;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_fold_case_non_ascii(b: Bencher) {
    let norm = norm_fold();
    b.bench_local(|| norm.compare("\u{00C9} 100", "\u{00E9} 10"));
}

#[divan::bench]
fn norm_already_nfc_compare(b: Bencher) {
    let norm = norm_nfc();
    let (l, r) = NORM_ALREADY_NFC;
    b.bench_local(|| norm.compare(l, r));
}

// ═══════════════════════════════════════════════════════════════════════
//  normalize() method
// ═══════════════════════════════════════════════════════════════════════

#[divan::bench]
fn normalize_ascii_nfc_fold(b: Bencher) {
    let norm = norm_nfc_fold();
    b.bench_local(|| norm.normalize(ASCII_INPUT));
}

#[divan::bench]
fn normalize_nfc_precomposed(b: Bencher) {
    let norm = norm_nfc();
    b.bench_local(|| norm.normalize(NFC_INPUT));
}

#[divan::bench]
fn normalize_nfc_decomposed(b: Bencher) {
    let norm = norm_nfc();
    b.bench_local(|| norm.normalize(DECOMPOSED_INPUT));
}

#[divan::bench]
fn normalize_fold_upper(b: Bencher) {
    let norm = norm_fold();
    b.bench_local(|| norm.normalize(UPPER_INPUT));
}

#[divan::bench]
fn normalize_ascii_only(b: Bencher) {
    let norm = norm_ascii_only();
    b.bench_local(|| norm.normalize(UPPER_INPUT));
}

// ═══════════════════════════════════════════════════════════════════════
//  Cross-function comparison — norm (case_fold) vs compare_ignore_case
// ═══════════════════════════════════════════════════════════════════════

#[divan::bench]
fn norm_vs_ic_ascii_equal(b: Bencher) {
    let norm = norm_fold();
    let (l, r) = IC_EQUAL;
    b.bench_local(|| norm.compare(l, r));
}

#[divan::bench]
fn norm_vs_ic_non_ascii_equal(b: Bencher) {
    let norm = norm_fold();
    b.bench_local(|| norm.compare("R\u{00E9}sum\u{00E9} 100", "r\u{00E9}sum\u{00E9} 100"));
}

#[divan::bench]
fn norm_vs_ic_non_ascii_numeric(b: Bencher) {
    let norm = norm_fold();
    b.bench_local(|| norm.compare("R\u{00E9}sum\u{00E9} 100", "r\u{00E9}sum\u{00E9} 20"));
}

// ═══════════════════════════════════════════════════════════════════════
//  compare_normalized convenience
// ═══════════════════════════════════════════════════════════════════════

#[divan::bench]
fn compare_normalized_ascii(b: Bencher) {
    let (l, r) = NORM_ASCII_EQUAL;
    b.bench_local(|| fast_natord::compare_normalized(l, r));
}

#[divan::bench]
fn compare_normalized_non_ascii(b: Bencher) {
    b.bench_local(|| {
        fast_natord::compare_normalized(
            "R\u{00E9}sum\u{00E9} 100",
            "r\u{0065}\u{0301}sum\u{0065}\u{0301} 20",
        )
    });
}
