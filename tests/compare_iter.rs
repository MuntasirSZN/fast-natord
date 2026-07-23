//! Integration tests for `fast-natord::compare_iter`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use core::cmp::Ordering;
use fast_natord::compare_iter;

#[test]
fn test_compare_iter_basic() {
    let result = compare_iter(
        "a10".chars(),
        "a2".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Greater);

    let result = compare_iter(
        "a2".chars(),
        "a10".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);

    let result = compare_iter(
        "abc".chars(),
        "abc".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Equal);
}

#[test]
fn test_compare_iter_left_aligned_zeros() {
    let result = compare_iter(
        "015".chars(),
        "12".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}

#[test]
fn test_compare_iter_right_aligned_accum() {
    let result = compare_iter(
        "1243".chars(),
        "1234".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Greater);
}

#[test]
fn test_compare_iter_right_aligned_tie() {
    let result = compare_iter(
        "1234".chars(),
        "1234".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Equal);
}

#[test]
fn test_compare_iter_left_longer() {
    let result = compare_iter(
        "abc".chars(),
        "ab".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Greater);

    let result = compare_iter(
        "ab".chars(),
        "abc".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}

#[test]
fn test_compare_iter_empty() {
    let result = compare_iter(
        "".chars(),
        "".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Equal);

    let result = compare_iter(
        "".chars(),
        "a".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}

#[test]
fn test_compare_iter_skip_whitespace() {
    let result = compare_iter(
        "a b".chars(),
        "ab".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Equal);
}

#[test]
fn test_compare_iter_zero_length_digits() {
    let result = compare_iter(
        "a0b".chars(),
        "a00b".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}

#[test]
fn test_compare_iter_right_aligned_longer_run_wins() {
    let result = compare_iter(
        "a123".chars(),
        "a12".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Greater);

    let result = compare_iter(
        "a12".chars(),
        "a123".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}

#[test]
fn test_compare_iter_left_aligned_zero_vs_zeros() {
    let result = compare_iter(
        "0".chars(),
        "00".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}

#[test]
fn test_compare_iter_skip_at_start() {
    let result = compare_iter(
        " a".chars(),
        "a".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Equal);

    let result = compare_iter(
        "  ".chars(),
        "".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Equal);
}

#[test]
fn test_compare_iter_right_aligned_then_non_digit() {
    let result = compare_iter(
        "a123b".chars(),
        "a123c".chars(),
        |c| c.is_whitespace(),
        |a, b| a.cmp(b),
        |c| c.to_digit(10).map(|v| v as isize),
    );
    assert_eq!(result, Ordering::Less);
}
