//! Unit tests for byte_utils sub-modules.

use super::basic::{finish_scalar, load_u64, load_u128};
use super::*;

// On wasm32 `#[test]` delegates to wasm_bindgen_test.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn test_is_digit() {
    for b in b'0'..=b'9' {
        assert!(is_digit(b), "byte {:?} should be a digit", b);
    }
    assert!(!is_digit(b' '));
    assert!(!is_digit(b'a'));
    assert!(!is_digit(b'Z'));
    assert!(!is_digit(b'/'));
    assert!(!is_digit(b':'));
    assert!(!is_digit(b'\0'));
}

#[test]
fn test_is_ascii_ws() {
    assert!(is_ascii_ws(b' '));
    assert!(is_ascii_ws(b'\t'));
    assert!(is_ascii_ws(b'\n'));
    assert!(is_ascii_ws(b'\x0C'));
    assert!(is_ascii_ws(b'\r'));
    assert!(!is_ascii_ws(b'a'));
    assert!(!is_ascii_ws(b'0'));
    assert!(!is_ascii_ws(b'\x0B'));
    assert!(!is_ascii_ws(b'\0'));
}

#[test]
fn test_load_u64() {
    let data = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let val = unsafe { load_u64(&data, 0) };
    assert_eq!(val, 0x0807060504030201);
}

#[test]
fn test_load_u64_offset() {
    let data: &[u8] = b"0123456789";
    let val = unsafe { load_u64(data, 2) };
    assert_eq!(val, 0x3938373635343332);
}

#[test]
fn test_load_u128() {
    let data = [
        0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10,
    ];
    let val = unsafe { load_u128(&data, 0) };
    assert_eq!(val, 0x100F0E0D0C0B0A090807060504030201);
}

#[test]
fn test_simd_is_ascii_empty() {
    assert!(simd_is_ascii(b""));
}

#[test]
fn test_simd_is_ascii_short() {
    assert!(simd_is_ascii(b"abc"));
    assert!(!simd_is_ascii(b"\xFF"));
    assert!(!simd_is_ascii(b"a\x80b"));
    assert!(!simd_is_ascii(b"\x80"));
}

#[test]
fn test_simd_is_ascii_exact_16() {
    let s = b"ABCDEFGHIJKLMNOP";
    assert!(simd_is_ascii(s));
    let mut ns = *s;
    ns[15] = 0x80;
    assert!(!simd_is_ascii(&ns));
}

#[test]
fn test_simd_is_ascii_long_ascii() {
    let long = b"Hello, World! This is a test string longer than 16 bytes to trigger SIMD.";
    assert!(simd_is_ascii(long));
}

#[test]
fn test_simd_is_ascii_long_non_ascii() {
    let long = b"Hello, World! This has \x80 non-ascii byte!";
    assert!(!simd_is_ascii(long));
}

#[test]
fn test_simd_is_ascii_non_ascii_early() {
    let mut buf = [b'A'; 16];
    buf[5] = 0xC0;
    assert!(!simd_is_ascii(&buf));
}

#[test]
fn test_simd_is_ascii_non_ascii_tail() {
    let mut buf = [b'A'; 33];
    buf[32] = 0x80;
    assert!(!simd_is_ascii(&buf));

    let mut buf2 = [b'A'; 32];
    buf2[31] = 0x80;
    assert!(!simd_is_ascii(&buf2));
}

#[test]
fn test_finish_scalar_identical() {
    let a = b"abcdefghijklmnop";
    let b = b"abcdefghijklmnop";
    unsafe {
        assert_eq!(finish_scalar(a, b, 0, 16), 16);
    }
}

#[test]
fn test_finish_scalar_diff_within_8byte_chunk() {
    let a = b"1234567A";
    let b = b"1234567B";
    unsafe {
        assert_eq!(finish_scalar(a, b, 0, 8), 7);
    }
}

#[test]
fn test_finish_scalar_diff_within_16byte_chunk() {
    let a = b"abcdefghijklmnoP";
    let b = b"abcdefghijklmnoQ";
    unsafe {
        assert_eq!(finish_scalar(a, b, 0, 16), 15);
    }
}

#[test]
fn test_finish_scalar_diff_after_16_in_8byte_chunk() {
    let a = b"abcdefghijklmnop1234567A";
    let b = b"abcdefghijklmnop1234567B";
    unsafe {
        let k = finish_scalar(a, b, 0, 24);
        assert_eq!(k, 23);
    }
}

#[test]
fn test_finish_scalar_short_below_8() {
    let a = b"ab";
    let b = b"ab";
    unsafe {
        assert_eq!(finish_scalar(a, b, 0, 2), 2);
    }

    let a = b"a";
    let b = b"b";
    unsafe {
        assert_eq!(finish_scalar(a, b, 0, 1), 0);
    }
}

#[test]
fn test_simd_skip_equal_identical_exact_stride() {
    let a = b"abcdefghijklmnop"; // 16 bytes
    unsafe {
        assert_eq!(simd_skip_equal(a, a, 0, 16), 16);
    }
    let a = b"abcdefghijklmnop1234"; // 20 bytes
    unsafe {
        assert_eq!(simd_skip_equal(a, a, 0, 20), 20);
    }
}

/// Verify that `simd_skip_equal` returns a position ≤ the first diff.
unsafe fn check_skip_upper_bound(a: &[u8], b: &[u8], common_len: usize, max_expected: usize) {
    unsafe {
        let k = simd_skip_equal(a, b, 0, common_len);
        assert!(
            k <= max_expected,
            "skip returned {k} but first diff ≤ {max_expected}"
        );
        for i in 0..k {
            assert_eq!(a[i], b[i], "byte {i} differs but skip returned {k}");
        }
    }
}

#[test]
fn test_simd_skip_equal_diff_first_chunk() {
    let a = b"abcdeFghijklmnop";
    let b = b"abcdeGghijklmnop";
    unsafe { check_skip_upper_bound(a, b, 16, 5) }
}

#[test]
fn test_simd_skip_equal_diff_at_16() {
    let a = b"abcdefghijklmnoPX";
    let b = b"abcdefghijklmnoQY";
    unsafe { check_skip_upper_bound(a, b, 16, 15) }
}

#[test]
fn test_simd_skip_equal_diff_in_tail() {
    let a = b"abcdefghijklmnop12345";
    let b = b"abcdefghijklmnop123XY";
    unsafe {
        let k = simd_skip_equal(a, b, 0, 20);
        assert_eq!(k, 19);
    }
}

#[test]
fn test_simd_skip_equal_short() {
    let a = b"short";
    let b = b"shXrt";
    unsafe {
        assert_eq!(simd_skip_equal(a, b, 0, 5), 2);
    }
}

#[test]
fn test_simd_skip_equal_short_identical() {
    let a = b"ab";
    unsafe {
        assert_eq!(simd_skip_equal(a, a, 0, 2), 2);
    }
}
