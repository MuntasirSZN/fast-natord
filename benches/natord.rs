//! Benchmarks for `fast-natord` using divan.
//!
//! cargo bench --bench natord

use divan::{AllocProfiler, Bencher};

fn main() {
    divan::main();
}

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

// ── Bench cases ──────────────────────────────────────────────────────

const EQUAL: (&str, &str) = (
    "abcdefghijklmnopqrstuvwxyz00001",
    "abcdefghijklmnopqrstuvwxyz00001",
);

const DIFFERENT_TAIL_DIGIT: (&str, &str) = (
    "abcdefghijklmnopqrstuvwxyz00001",
    "abcdefghijklmnopqrstuvwxyz00002",
);

const DIFFERENT_LEADING_DIGIT: (&str, &str) = ("a10", "a2");

const SHORT_PREFIX: (&str, &str) = ("pic 5 something", "pic 6");

const SHORT_IDENTICAL: (&str, &str) = ("hello", "hello");

const SHORT_DIFFERENT: (&str, &str) = ("fred", "jane");

const WITH_LEADING_ZEROS: (&str, &str) = ("1.001", "1.002");

const LONG_DIGIT_RUNS: (&str, &str) = ("12345678901234", "12345678901235");

const MIXED_SPACES: (&str, &str) = ("pic4   alpha", "pic 4 else");

// ── compare ──────────────────────────────────────────────────────────

#[divan::bench]
fn equal(b: Bencher) {
    let (l, r) = EQUAL;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn different_tail_digit(b: Bencher) {
    let (l, r) = DIFFERENT_TAIL_DIGIT;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn different_leading_digit(b: Bencher) {
    let (l, r) = DIFFERENT_LEADING_DIGIT;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn short_prefix(b: Bencher) {
    let (l, r) = SHORT_PREFIX;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn short_identical(b: Bencher) {
    let (l, r) = SHORT_IDENTICAL;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn short_different(b: Bencher) {
    let (l, r) = SHORT_DIFFERENT;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn with_leading_zeros(b: Bencher) {
    let (l, r) = WITH_LEADING_ZEROS;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn long_digit_runs(b: Bencher) {
    let (l, r) = LONG_DIGIT_RUNS;
    b.bench_local(|| fast_natord::compare(l, r));
}

#[divan::bench]
fn mixed_spaces(b: Bencher) {
    let (l, r) = MIXED_SPACES;
    b.bench_local(|| fast_natord::compare(l, r));
}

// ── Variable-length prefix ───────────────────────────────────────────

#[divan::bench]
fn short_prefix_sized(b: Bencher) {
    let a = "abcdefghijklmnopqrstuvwxyz00001";
    let c = "abcdefghijklmnopqrstuvwxyz00002";
    b.bench_local(|| fast_natord::compare(a, c));
}

#[divan::bench]
fn long_prefix_sized(b: Bencher) {
    let prefix = "x".repeat(256);
    let a = prefix.clone() + "00001";
    let c = prefix + "00002";
    b.bench_local(|| fast_natord::compare(&a, &c));
}

// ── compare_ignore_case ──────────────────────────────────────────────

const IC_EQUAL: (&str, &str) = ("AlphabetSoup0123", "alphabetSoup0123");

const IC_DIFFERENT_END: (&str, &str) = ("AbCdEf123", "aBcDeF456");

#[divan::bench]
fn ic_equal(b: Bencher) {
    let (l, r) = IC_EQUAL;
    b.bench_local(|| fast_natord::compare_ignore_case(l, r));
}

#[divan::bench]
fn ic_different_end(b: Bencher) {
    let (l, r) = IC_DIFFERENT_END;
    b.bench_local(|| fast_natord::compare_ignore_case(l, r));
}

// ── compare_iter ─────────────────────────────────────────────────────

#[divan::bench]
fn iter_compare(b: Bencher) {
    let (l, r) = DIFFERENT_LEADING_DIGIT;
    b.bench_local(|| {
        fast_natord::compare_iter(
            l.chars(),
            r.chars(),
            |c| c.is_whitespace(),
            |a, b| a.cmp(b),
            |c| c.to_digit(10).map(|v| v as isize),
        )
    });
}

// ═══════════════════════════════════════════════════════════════════════
//  Normalizer benchmarks
// ═══════════════════════════════════════════════════════════════════════
//
// All normalizer benchmarks pre-construct the Normalizer statically so
// construction cost is excluded.

use fast_natord::Normalizer;

// ── Builders for each normalizer config ─────────────────────────────
//
// Used once per benchmark function, outside the hot loop, so
// construction cost is excluded from measurement.

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

// ── Data ────────────────────────────────────────────────────────────
//
// ASCII pairs (no normalization needed — tests Normalizer dispatch
// overhead on the fast path).

const NORM_ASCII_EQUAL: (&str, &str) = ("hello123", "HELLO123");
const NORM_ASCII_NUMERIC: (&str, &str) = ("pic10", "PIC2");
const NORM_ASCII_DIFFERENT: (&str, &str) = ("apple100", "APPLE20");

/// Precomposed é (U+00E9) vs decomposed e + combining acute (U+0301)
/// — exercises NFC recomposition.
const NORM_DECOMPOSED_EQUAL: (&str, &str) = ("caf\u{00E9}123", "cafe\u{0301}123");

/// Uppercase É precomposed (U+00C9) vs lowercase é decomposed
/// — exercises both NFC recomposition AND case folding.
const NORM_CASE_DECOMPOSED: (&str, &str) = ("Caf\u{00C9}100", "cafe\u{0301}10");

/// Already NFC, already lowercase — pure comparison, no transformation.
const NORM_ALREADY_NFC: (&str, &str) = ("caf\u{00E9} 10", "caf\u{00E9} 2");

/// Longer non-ASCII with numeric suffix, mixed case — exercises
/// prefix-SIMD skip + normalization + numeric run detection.
const NORM_MIXED_LONG: (&str, &str) = (
    "R\u{00E9}sum\u{00E9} Draft 100",
    "r\u{0065}\u{0301}sum\u{0065}\u{0301} draft 20",
);

/// Strings for the `normalize()` method benchmark.
static ASCII_INPUT: &str = "The quick brown fox jumps over the lazy dog 12345";
static NFC_INPUT: &str = "caf\u{00E9} cr\u{00E8}me br\u{00FB}l\u{00E9}e 12345";
static DECOMPOSED_INPUT: &str = "cafe\u{0301} cre\u{0300}me bru\u{0302}le\u{0301}e 12345";
static UPPER_INPUT: &str = "CAFÉ CRÈME BRÛLÉE 12345";

// ── ASCII fast path — compare ───────────────────────────────────────
//
// All-ASCII inputs should zero-allocate and return borrowed Cows.
// These measure the Normalizer dispatch overhead vs raw compare().

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

// ── Non-ASCII — compare ─────────────────────────────────────────────

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

// ── normalize() method ──────────────────────────────────────────────
//
// Measures pure pre-processing cost independent of comparison.

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

// ── Cross-function comparison ───────────────────────────────────────
//
// Normalizer::compare (pre-fold, then fast compare) vs
// compare_ignore_case (per-char lowercasing in the hot loop).

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

// ── compare_normalized convenience ───────────────────────────────────

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
