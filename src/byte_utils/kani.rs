//! Kani proofs for byte_utils.

#![cfg(kani)]

use crate::byte_utils;
use crate::byte_utils::basic;
use crate::byte_utils::skip_equal;

#[kani::proof]
fn is_digit_matches_spec() {
    let c: u8 = kani::any();
    let spec = (b'0'..=b'9').contains(&c);
    assert_eq!(byte_utils::is_digit(c), spec);
}

#[kani::proof]
fn is_ascii_ws_matches_spec() {
    let c: u8 = kani::any();
    let spec = c == b' ' || c == b'\t' || c == b'\n' || c == b'\x0C' || c == b'\r';
    assert_eq!(byte_utils::is_ascii_ws(c), spec);
}

#[kani::proof]
fn load_u64_in_bounds() {
    const N: usize = 24;
    let data: [u8; N] = kani::any();
    let i: usize = kani::any();
    kani::assume(i <= N - 8);
    let _ = unsafe { basic::load_u64(&data, i) };
}

#[kani::proof]
fn load_u128_in_bounds() {
    const N: usize = 40;
    let data: [u8; N] = kani::any();
    let i: usize = kani::any();
    kani::assume(i <= N - 16);
    let _ = unsafe { basic::load_u128(&data, i) };
}

const FS_LEN: usize = 24;

#[kani::proof]
#[kani::unwind(28)]
fn finish_scalar_contract() {
    let a: [u8; FS_LEN] = kani::any();
    let b: [u8; FS_LEN] = kani::any();
    let k: usize = kani::any();
    let common_len: usize = kani::any();
    kani::assume(k <= common_len);
    kani::assume(common_len <= FS_LEN);

    let r = unsafe { basic::finish_scalar(&a, &b, k, common_len) };

    assert!(r >= k);
    assert!(r <= common_len);

    let mut idx = k;
    while idx < r {
        assert_eq!(a[idx], b[idx]);
        idx += 1;
    }
}

const SK_LEN: usize = 24;

#[kani::proof]
#[kani::unwind(28)]
fn simd_skip_equal_contract() {
    let a: [u8; SK_LEN] = kani::any();
    let b: [u8; SK_LEN] = kani::any();
    let common_len: usize = kani::any();
    kani::assume(common_len <= SK_LEN);

    let r = unsafe { byte_utils::simd_skip_equal(&a, &b, 0, common_len) };

    assert!(r <= common_len);
    let mut idx = 0;
    while idx < r {
        assert_eq!(a[idx], b[idx]);
        idx += 1;
    }
}

#[cfg(target_arch = "x86_64")]
#[kani::proof]
#[kani::unwind(28)]
fn skip_sse2_contract() {
    let a: [u8; SK_LEN] = kani::any();
    let b: [u8; SK_LEN] = kani::any();
    let common_len: usize = kani::any();
    kani::assume(common_len <= SK_LEN);

    let r = unsafe { skip_equal::skip_sse2(&a, &b, 0, common_len) };

    assert!(r <= common_len);
    let mut idx = 0;
    while idx < r {
        assert_eq!(a[idx], b[idx]);
        idx += 1;
    }
}
