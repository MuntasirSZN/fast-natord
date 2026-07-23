//! Valgrind benchmarks for `Normalizer` and `compare_normalized`.
//!
//! cargo bench --bench valgrind_normalizer

#![cfg(not(any(target_os = "windows", target_arch = "wasm32")))]

use fast_natord::*;
use gungraun::{Cachegrind, CachegrindMetrics, Dhat, Massif, Memcheck, OutputFormat, prelude::*};
use std::hint::black_box;

// ── Data ─────────────────────────────────────────────────────────────

const NORM_ASCII_EQUAL: (&str, &str) = ("hello123", "HELLO123");
const NORM_ASCII_NUMERIC: (&str, &str) = ("pic10", "PIC2");
const NORM_ASCII_DIFFERENT: (&str, &str) = ("apple100", "APPLE20");
const NORM_DECOMPOSED_EQUAL: (&str, &str) = ("caf\u{00E9}123", "cafe\u{0301}123");
const NORM_CASE_DECOMPOSED: (&str, &str) = ("Caf\u{00C9}100", "cafe\u{0301}10");
const NORM_ALREADY_NFC: (&str, &str) = ("caf\u{00E9} 10", "caf\u{00E9} 2");
const NORM_MIXED_LONG: (&str, &str) = ("Résumé_2024_Final_v10", "résumé_2024_final_v2");

const ASCII_INPUT: &str = "The quick brown fox jumps over the lazy dog 12345";
const NFC_INPUT: &str = "caf\u{00E9} cr\u{00E8}me br\u{00FB}l\u{00E9}e 12345";
const DECOMPOSED_INPUT: &str = "cafe\u{0301} cre\u{0300}me bru\u{0302}le\u{0301}e 12345";
const UPPER_INPUT: &str = "CAFÉ CRÈME BRÛLÉE 12345";

// ── Setup functions ──────────────────────────────────────────────────

fn setup_nfc_fold<'a>(input: (&'a str, &'a str)) -> (Normalizer, (&'a str, &'a str)) {
    (Normalizer::new().nfc().case_fold(), input)
}

fn setup_nfc<'a>(input: (&'a str, &'a str)) -> (Normalizer, (&'a str, &'a str)) {
    (Normalizer::new().nfc(), input)
}

fn setup_fold<'a>(input: (&'a str, &'a str)) -> (Normalizer, (&'a str, &'a str)) {
    (Normalizer::new().case_fold(), input)
}

fn setup_norm_single(input: &str) -> (Normalizer, &str) {
    (Normalizer::new().nfc().case_fold(), input)
}

fn setup_norm_nfc(input: &str) -> (Normalizer, &str) {
    (Normalizer::new().nfc(), input)
}

fn setup_norm_fold(input: &str) -> (Normalizer, &str) {
    (Normalizer::new().case_fold(), input)
}

fn setup_norm_ascii_only(input: &str) -> (Normalizer, &str) {
    (Normalizer::new().case_ascii_only(), input)
}

// ── Normalizer compare ASCII fast path ───────────────────────────────

#[library_benchmark(setup = setup_nfc_fold)]
#[bench::none_ascii_equal(args = (NORM_ASCII_EQUAL,))]
#[bench::nfc_fold_ascii_equal(args = (NORM_ASCII_EQUAL,))]
#[bench::nfc_fold_ascii_numeric(args = (NORM_ASCII_NUMERIC,))]
#[bench::nfc_fold_ascii_different(args = (NORM_ASCII_DIFFERENT,))]
#[bench::ascii_only_ascii_equal(args = (NORM_ASCII_EQUAL,))]
#[bench::fold_ascii_equal(args = (NORM_ASCII_EQUAL,))]
fn bench_normalizer_compare_ascii(input: (Normalizer, (&str, &str))) -> std::cmp::Ordering {
    let (normalizer, (a, b)) = input;
    black_box(normalizer.compare(black_box(a), black_box(b)))
}

// ── Normalizer compare non-ASCII ─────────────────────────────────────

#[library_benchmark]
#[bench::nfc_decomposed_equal(args = (NORM_DECOMPOSED_EQUAL,), setup = setup_nfc)]
#[bench::nfc_fold_decomposed_equal(args = (NORM_DECOMPOSED_EQUAL,), setup = setup_nfc_fold)]
#[bench::nfc_fold_case_decomposed(args = (NORM_CASE_DECOMPOSED,), setup = setup_nfc_fold)]
#[bench::nfc_fold_mixed_long(args = (NORM_MIXED_LONG,), setup = setup_nfc_fold)]
#[bench::fold_case_non_ascii(args = (NORM_CASE_DECOMPOSED,), setup = setup_fold)]
#[bench::already_nfc(args = (NORM_ALREADY_NFC,), setup = setup_nfc_fold)]
fn bench_normalizer_compare_non_ascii(input: (Normalizer, (&str, &str))) -> std::cmp::Ordering {
    let (normalizer, (a, b)) = input;
    black_box(normalizer.compare(black_box(a), black_box(b)))
}

// ── normalize() method ───────────────────────────────────────────────

#[library_benchmark]
#[bench::ascii_nfc_fold(args = (ASCII_INPUT,), setup = setup_norm_single)]
#[bench::nfc_precomposed(args = (NFC_INPUT,), setup = setup_norm_nfc)]
#[bench::nfc_decomposed(args = (DECOMPOSED_INPUT,), setup = setup_norm_nfc)]
#[bench::fold_upper(args = (UPPER_INPUT,), setup = setup_norm_fold)]
#[bench::ascii_only(args = (ASCII_INPUT,), setup = setup_norm_ascii_only)]
fn bench_normalize(input: (Normalizer, &str)) {
    let (normalizer, s) = input;
    black_box(normalizer.normalize(black_box(s)));
}

// ── Cross-function comparison ────────────────────────────────────────

#[library_benchmark]
#[bench::norm_vs_ic_ascii_equal(args = (NORM_ASCII_EQUAL,), setup = setup_nfc_fold)]
#[bench::norm_vs_ic_non_ascii_equal(args = (NORM_DECOMPOSED_EQUAL,), setup = setup_nfc_fold)]
#[bench::norm_vs_ic_non_ascii_numeric(args = (NORM_MIXED_LONG,), setup = setup_nfc_fold)]
fn bench_norm_vs_ic(input: (Normalizer, (&str, &str))) {
    let (normalizer, (a, b)) = input;
    let norm_result = black_box(normalizer.compare(black_box(a), black_box(b)));
    let ic_result = black_box(compare_ignore_case(black_box(a), black_box(b)));
    black_box((norm_result, ic_result));
}

// ── compare_normalized convenience ───────────────────────────────────

#[library_benchmark]
#[bench::ascii(NORM_ASCII_EQUAL)]
#[bench::non_ascii(NORM_DECOMPOSED_EQUAL)]
fn bench_compare_normalized(input: (&str, &str)) -> std::cmp::Ordering {
    black_box(compare_normalized(black_box(input.0), black_box(input.1)))
}

// ── Groups + main! ──────────────────────────────────────────────────

library_benchmark_group!(
    name = normalizer_compare_ascii_group,
    benchmarks = [bench_normalizer_compare_ascii]
);

library_benchmark_group!(
    name = normalizer_compare_non_ascii_group,
    benchmarks = [bench_normalizer_compare_non_ascii]
);

library_benchmark_group!(name = normalize_group, benchmarks = [bench_normalize]);

library_benchmark_group!(
    name = cross_comparison_group,
    benchmarks = [bench_norm_vs_ic]
);

library_benchmark_group!(
    name = convenience_group,
    benchmarks = [bench_compare_normalized]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Cachegrind::default().format([CachegrindMetrics::All]))
        .tool(Dhat::default())
        .tool(Massif::default())
        .tool(Memcheck::default())
        .output_format(OutputFormat::default().tolerance(0.9)),
    library_benchmark_groups = [
        normalizer_compare_ascii_group,
        normalizer_compare_non_ascii_group,
        normalize_group,
        cross_comparison_group,
        convenience_group,
    ]
);
