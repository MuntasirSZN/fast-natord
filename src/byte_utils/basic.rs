//! Basic byte predicates and scalar helpers.
//!
//! Provides fast inlineable helpers for digit/whitespace detection,
//! unaligned memory loads, and scalar byte-level comparison (used as
//! fallback when SIMD is unavailable or for short inputs).

/// True when `c` is an ASCII decimal digit (`0`–`9`).
#[inline(always)]
pub fn is_digit(c: u8) -> bool {
    c.wrapping_sub(b'0') <= 9
}

/// True when `c` is an ASCII whitespace byte: tab, newline, form feed,
/// carriage return, or space.
///
/// Note: vertical tab (`0x0B`) is NOT included.
#[inline(always)]
pub fn is_ascii_ws(c: u8) -> bool {
    c == b' ' || c.wrapping_sub(b'\t') <= 1 || c.wrapping_sub(b'\x0C') <= 1
}

/// Load a `u64` from `s[i..i+8]` with unaligned read.
///
/// # Safety
///
/// `i + 8 <= s.len()`.
#[inline(always)]
pub unsafe fn load_u64(s: &[u8], i: usize) -> u64 {
    unsafe { (s.as_ptr().add(i) as *const u64).read_unaligned() }
}

/// Load a `u128` from `s[i..i+16]` with unaligned read.
///
/// # Safety
///
/// `i + 16 <= s.len()`.
#[inline(always)]
pub unsafe fn load_u128(s: &[u8], i: usize) -> u128 {
    unsafe { (s.as_ptr().add(i) as *const u128).read_unaligned() }
}

/// Skip ASCII whitespace on both sides.  Marked cold to keep the hot
/// comparison loop free of whitespace-handling code.
#[cold]
pub unsafe fn skip_whitespace(
    pa: &mut *const u8,
    pb: &mut *const u8,
    enda: *const u8,
    endb: *const u8,
) {
    unsafe {
        while *pa < enda && is_ascii_ws(**pa) {
            *pa = pa.add(1);
        }
        while *pb < endb && is_ascii_ws(**pb) {
            *pb = pb.add(1);
        }
    }
}

/// Scalar tail processing shared by all SIMD `skip_equal` backends.
///
/// Processes remaining bytes (after a SIMD chunk) using 16-byte `u128`
/// compares, then 8-byte `u64` compares, then byte-by-byte.
///
/// Returns the first index where `a` and `b` differ, or `common_len` if
/// all `common_len` bytes are equal.
///
/// # Safety
///
/// `k <= common_len <= min(a.len(), b.len())`.
#[inline(always)]
pub unsafe fn finish_scalar(a: &[u8], b: &[u8], mut k: usize, common_len: usize) -> usize {
    unsafe {
        while k + 16 <= common_len {
            if load_u128(a, k) != load_u128(b, k) {
                break;
            }
            k += 16;
        }
        while k + 8 <= common_len {
            if load_u64(a, k) != load_u64(b, k) {
                break;
            }
            k += 8;
        }
        // Trailing <8 bytes: byte-by-byte to find exact diff position.
        while k < common_len && *a.get_unchecked(k) == *b.get_unchecked(k) {
            k += 1;
        }
        k
    }
}
