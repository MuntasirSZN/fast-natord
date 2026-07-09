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
