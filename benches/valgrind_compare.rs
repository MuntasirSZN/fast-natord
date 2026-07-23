//! Valgrind benchmarks for `compare`, `compare_ignore_case`, `compare_iter`.
//!
//! cargo bench --bench valgrind_compare

#![cfg(not(any(target_os = "windows", target_arch = "wasm32")))]

use fast_natord::*;
use gungraun::{Cachegrind, CachegrindMetrics, Dhat, Massif, Memcheck, OutputFormat, prelude::*};
use std::hint::black_box;

// ── Data ─────────────────────────────────────────────────────────────

const EQUAL: (&str, &str) = ("hello", "hello");
const DIFFERENT_TAIL_DIGIT: (&str, &str) = ("file10", "file11");
const DIFFERENT_LEADING_DIGIT: (&str, &str) = ("a10", "a2");
const SHORT_PREFIX: (&str, &str) = ("pic 5 something", "pic 6");
const SHORT_DIFFERENT: (&str, &str) = ("fred", "jane");
const WITH_LEADING_ZEROS: (&str, &str) = ("1.001", "1.002");
const LONG_DIGIT_RUNS: (&str, &str) = ("12345678901234", "12345678901235");
const MIXED_SPACES: (&str, &str) = ("pic4   alpha", "pic 4 else");

const IC_EQUAL: (&str, &str) = ("AlphabetSoup0123", "alphabetSoup0123");
const IC_DIFFERENT_END: (&str, &str) = ("AbCdEf123", "aBcDeF456");

// Default functions for compare_iter
fn skip_none(_: &u8) -> bool {
    false
}
fn cmp_u8(a: &u8, b: &u8) -> std::cmp::Ordering {
    a.cmp(b)
}
fn to_digit_u8(c: &u8) -> Option<isize> {
    (c.is_ascii_digit()).then(|| (*c - b'0') as isize)
}

// ── compare ──────────────────────────────────────────────────────────

#[library_benchmark]
#[bench::short(EQUAL)]
#[bench::long(DIFFERENT_TAIL_DIGIT)]
#[bench::different_leading_digit(DIFFERENT_LEADING_DIGIT)]
#[bench::short_prefix(SHORT_PREFIX)]
#[bench::short_different(SHORT_DIFFERENT)]
#[bench::leading_zeros(WITH_LEADING_ZEROS)]
#[bench::long_digits(LONG_DIGIT_RUNS)]
#[bench::mixed_spaces(MIXED_SPACES)]
fn bench_compare(input: (&str, &str)) -> std::cmp::Ordering {
    black_box(compare(black_box(input.0), black_box(input.1)))
}

// ── compare_ignore_case ──────────────────────────────────────────────

#[library_benchmark]
#[bench::short(IC_EQUAL)]
#[bench::long(IC_DIFFERENT_END)]
fn bench_compare_ignore_case(input: (&str, &str)) -> std::cmp::Ordering {
    black_box(compare_ignore_case(black_box(input.0), black_box(input.1)))
}

// ── compare_iter ─────────────────────────────────────────────────────

#[library_benchmark]
#[bench::short(EQUAL)]
#[bench::long(DIFFERENT_TAIL_DIGIT)]
#[bench::different_leading_digit(DIFFERENT_LEADING_DIGIT)]
#[bench::short_prefix(SHORT_PREFIX)]
fn bench_compare_iter(input: (&str, &str)) -> std::cmp::Ordering {
    black_box(compare_iter(
        black_box(input.0.bytes()),
        black_box(input.1.bytes()),
        skip_none,
        cmp_u8,
        to_digit_u8,
    ))
}

// ── Groups + main! ──────────────────────────────────────────────────

library_benchmark_group!(
    name = compare_group,
    benchmarks = [bench_compare, bench_compare_ignore_case, bench_compare_iter,]
);

main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Cachegrind::default().format([CachegrindMetrics::All]))
        .tool(Dhat::default())
        .tool(Massif::default())
        .tool(Memcheck::default())
        .output_format(OutputFormat::default().tolerance(0.9)),
    library_benchmark_groups = [compare_group,]
);
