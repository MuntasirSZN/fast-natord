//! Unit tests for byte_utils sub-modules.

use super::basic::{finish_scalar, load_u64, load_u128};
use super::skip_while_digit::{digit_run_ends_short, simd_skip_while_digit};
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

// ── digit_run_ends_short ──────────────────────────────────────────────

#[test]
fn test_digit_run_ends_short_both_run() {
    unsafe {
        let (ea, eb) = digit_run_ends_short(b"ab12cde", b"xy123z", 2, 2);
        assert_eq!(ea, 4, "a run should end at index 4 (the 'c')");
        assert_eq!(eb, 5, "b run should end at index 5 (the 'z')");
    }
}

#[test]
fn test_digit_run_ends_short_a_longer() {
    unsafe {
        let (ea, eb) = digit_run_ends_short(b"ab12345x", b"xy12z", 2, 2);
        assert_eq!(ea, 7, "a run has 5 digits");
        assert_eq!(eb, 4, "b run has 2 digits");
    }
}

#[test]
fn test_digit_run_ends_short_a_shorter() {
    unsafe {
        let (ea, eb) = digit_run_ends_short(b"ab12x", b"xy123z", 2, 2);
        assert_eq!(ea, 4, "a run ends at 4");
        assert_eq!(eb, 5, "b run ends at 5");
    }
}

#[test]
fn test_digit_run_ends_short_all_digits() {
    unsafe {
        // Entire string is digits
        let (ea, eb) = digit_run_ends_short(b"123456", b"789", 0, 0);
        assert_eq!(ea, 6);
        assert_eq!(eb, 3);
    }
}

#[test]
fn test_digit_run_ends_short_no_digits() {
    unsafe {
        let (ea, eb) = digit_run_ends_short(b"abc", b"xyz", 0, 0);
        assert_eq!(ea, 0);
        assert_eq!(eb, 0);
    }
}

#[test]
fn test_digit_run_ends_short_immediate_non_digit() {
    unsafe {
        let (ea, eb) = digit_run_ends_short(b"abx", b"xya", 2, 2);
        assert_eq!(ea, 2, "no digit run in a");
        assert_eq!(eb, 2, "no digit run in b");
    }
}

// ── simd_skip_while_digit_both short-path ─────────────────────────────

#[test]
fn test_skip_while_digit_both_short_runs() {
    // Both remaining < 16 → triggers digit_run_ends_short inside the function
    unsafe {
        let (ea, eb) = simd_skip_while_digit_both(
            b"aa123bc", b"bb45xyz", 2, // start_a after "aa"
            2, // start_b after "bb"
        );
        assert_eq!(ea, 5, "a: 3 digits '123', ends at index 5");
        assert_eq!(eb, 4, "b: 2 digits '45', ends at index 4");
    }
}

#[test]
fn test_skip_while_digit_both_a_longer() {
    unsafe {
        let (ea, eb) = simd_skip_while_digit_both(b"aa12345xx", b"bb12yyy", 2, 2);
        assert_eq!(ea, 7, "a: 5 digits ends at 7");
        assert_eq!(eb, 4, "b: 2 digits ends at 4");
    }
}

#[test]
fn test_skip_while_digit_both_both_no_digits() {
    unsafe {
        let (ea, eb) = simd_skip_while_digit_both(b"abcdef", b"uvwxyz", 0, 0);
        assert_eq!(ea, 0);
        assert_eq!(eb, 0);
    }
}

// ── compare_word_at_a_time equal u128 chunks ──────────────────────────

#[test]
fn test_word_at_a_time_u128_equal() {
    // 16 equal bytes → hits the equal u128 chunk path (diff == 0)
    unsafe {
        let a = b"0123456789abcdef";
        let b = b"0123456789abcdef";
        assert_eq!(
            compare_word_at_a_time(a.as_ptr(), b.as_ptr(), 16),
            None,
            "all equal should return None"
        );
    }
}

#[test]
fn test_word_at_a_time_u128_equal_with_tail() {
    // 20 bytes: first u128 equal, then u64 finds diff at byte 16
    unsafe {
        let a = b"0123456789abcdefGX";
        let b = b"0123456789abcdefHY";
        let res = compare_word_at_a_time(a.as_ptr(), b.as_ptr(), 18);
        assert!(res.is_some());
        assert_eq!(res, Some(core::cmp::Ordering::Less));
    }
}

#[test]
fn test_word_at_a_time_u64_equal_then_byte_diff() {
    // 12 bytes: u128 skips (12 < 16), u64 block of 8 equal bytes, then byte diff
    unsafe {
        let a = b"01234567XY";
        let b = b"01234567XZ";
        let res = compare_word_at_a_time(a.as_ptr(), b.as_ptr(), 10);
        assert!(res.is_some());
        assert_eq!(res, Some(core::cmp::Ordering::Less));
    }
}

// ── simd_skip_while_digit short <16 bytes ─────────────────────────────

#[test]
fn test_simd_skip_while_digit_scalar_short() {
    // remaining < 16 → scalar fallback
    unsafe {
        let s = b"ab123c";
        assert_eq!(simd_skip_while_digit(s, 2), 5);
    }
}

#[test]
fn test_simd_skip_while_digit_scalar_short_no_digits() {
    unsafe {
        let s = b"abcde";
        assert_eq!(simd_skip_while_digit(s, 0), 0);
    }
}

#[test]
fn test_simd_skip_while_digit_scalar_short_all_digits() {
    unsafe {
        let s = b"12345";
        assert_eq!(simd_skip_while_digit(s, 0), 5);
    }
}

// ── simd_skip_equal short <32 bytes scalar fallback ───────────────────

#[test]
fn test_simd_skip_equal_under_32() {
    let a = b"abcdefghij123456";
    let b = b"abcdefghijX23456";
    unsafe {
        // common_len = 16 < 32 → uses finish_scalar directly, no SIMD dispatch
        let k = simd_skip_equal(a, b, 0, 16);
        assert_eq!(k, 10, "diff at byte 10 ('1' vs 'X')");
    }
}

#[test]
fn test_simd_skip_equal_at_32_boundary() {
    let a = b"abcdefghijklmnopqrstuvwxyzABCD"; // 30 bytes
    let b = b"abcdefghijklmnopqrstuvwxyzabcD"; // diff at byte 26
    unsafe {
        let k = simd_skip_equal(a, b, 0, 30);
        assert_eq!(k, 26);
    }
}

// ── finish_scalar edge: u64 all-equal then u128───

#[test]
fn test_finish_scalar_u128_all_equal_then_diff() {
    // 24 bytes: first 16 (u128) equal, then 8 (u64) equal, then byte diff
    let a = b"abcdefghijklmnop1234567A";
    let b = b"abcdefghijklmnop1234567B";
    unsafe {
        assert_eq!(finish_scalar(a, b, 0, 24), 23);
    }
}

#[test]
fn test_finish_scalar_u128_diff_in_first_chunk() {
    let a = b"abcdefghijklmnoPxxxxxx";
    let b = b"abcdefghijklmnoQxxxxxx";
    unsafe {
        // diff at byte 15, common_len=22
        assert_eq!(finish_scalar(a, b, 0, 22), 15);
    }
}
