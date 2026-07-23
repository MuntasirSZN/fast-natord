//! Kani proofs for case-sensitive comparator.

#![cfg(kani)]

use crate::compare::compare_impl;

#[kani::proof]
#[kani::unwind(4)]
fn compare_impl_memory_safe() {
    let a0: u8 = kani::any();
    let b0: u8 = kani::any();
    let a = [a0];
    let b = [b0];
    let _ = compare_impl(&a, &b);
}

const NUM_LEN: usize = 2;

fn digits_to_value(buf: &[u8]) -> u64 {
    let mut v: u64 = 0;
    if buf.len() > 0 {
        v = (buf[0] - b'0') as u64;
    }
    if buf.len() > 1 {
        v = v * 10 + (buf[1] - b'0') as u64;
    }
    v
}

fn any_digit_run(len: usize, forbid_leading_zero: bool) -> [u8; NUM_LEN] {
    let mut buf = [0u8; NUM_LEN];
    if 0 < len {
        let d: u8 = kani::any();
        kani::assume(d <= 9);
        buf[0] = b'0' + d;
    }
    if 1 < len {
        let d: u8 = kani::any();
        kani::assume(d <= 9);
        buf[1] = b'0' + d;
    }
    if forbid_leading_zero && len > 1 {
        kani::assume(buf[0] != b'0');
    }
    buf
}

#[kani::proof]
#[kani::unwind(3)]
fn compare_impl_right_aligned_matches_integer_value() {
    let len_a: usize = kani::any();
    let len_b: usize = kani::any();
    kani::assume(len_a >= 1 && len_a <= NUM_LEN);
    kani::assume(len_b >= 1 && len_b <= NUM_LEN);

    let ba = any_digit_run(len_a, true);
    let bb = any_digit_run(len_b, true);
    let a = &ba[..len_a];
    let b = &bb[..len_b];

    let expected = digits_to_value(a).cmp(&digits_to_value(b));
    assert_eq!(compare_impl(a, b), expected);
}

#[kani::proof]
#[kani::unwind(3)]
fn compare_impl_leading_zero_matches_lexicographic() {
    let len_a: usize = kani::any();
    let len_b: usize = kani::any();
    kani::assume(len_a >= 1 && len_a <= NUM_LEN);
    kani::assume(len_b >= 1 && len_b <= NUM_LEN);

    let ba = any_digit_run(len_a, false);
    let bb = any_digit_run(len_b, false);
    kani::assume(ba[0] == b'0');
    kani::assume(bb[0] == b'0');
    let a = &ba[..len_a];
    let b = &bb[..len_b];

    assert_eq!(compare_impl(a, b), a.cmp(b));
}
