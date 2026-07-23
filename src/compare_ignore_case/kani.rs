//! Kani proofs for case-insensitive comparator.

#![cfg(kani)]

use super::*;

#[kani::proof]
#[kani::unwind(3)]
fn compare_ignore_case_impl_memory_safe() {
    let a0: u8 = kani::any();
    let b0: u8 = kani::any();
    kani::assume(a0 < 128);
    kani::assume(b0 < 128);
    let a = [a0];
    let b = [b0];
    let _ = compare_ignore_case_impl(&a, &b);
}
