#[inline(always)]
pub unsafe fn load_u64(s: &[u8], i: usize) -> u64 {
    unsafe { (s.as_ptr().add(i) as *const u64).read_unaligned() }
}

#[inline(always)]
pub unsafe fn load_u128(s: &[u8], i: usize) -> u128 {
    unsafe { (s.as_ptr().add(i) as *const u128).read_unaligned() }
}

#[inline(always)]
pub fn is_digit(c: u8) -> bool {
    c.wrapping_sub(b'0') <= 9
}

#[inline(always)]
pub fn is_ascii_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\x0C' | b'\r')
}

/// Finish the common-prefix skip with scalar 16/8-byte chunks for any tail
/// that SIMD did not cover (or when no SIMD backend is available).
#[inline(always)]
unsafe fn finish_scalar(a: &[u8], b: &[u8], mut k: usize, common_len: usize) -> usize {
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
        k
    }
}

/// Skip equal bytes from `i` up to `common_len` using the widest SIMD path
/// the target supports, then a scalar tail. Pure byte equality — semantically
/// identical to a `u128`/`u64` prefix scan, so it is safe for both case-sensitive
/// and case-insensitive comparison (case folding happens only in the per-byte
/// tail). Returns the advanced index (equal to `j` in the callers).
///
/// Backends (baseline ISA features, called once per comparison):
/// * `x86_64`: 16-byte SSE2 chunks (always available on x86_64).
/// * `aarch64`: 16-byte NEON chunks (always available on AArch64).
/// * any other target: scalar-only tail.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    unsafe {
        use core::arch::x86_64::*;
        let mut k = i;
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(va, vb));
            // mask == 0xFFFF → all 16 bytes equal.
            // First zero bit = first differing byte.  Use tzcnt to find
            // its position and advance k past it, skipping the scalar rescan.
            if mask != 0xFFFF {
                let diff_bit = (!(mask as u32)).trailing_zeros() as usize;
                return k + diff_bit;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    unsafe {
        use core::arch::aarch64::*;
        let mut k = i;
        while k + 16 <= common_len {
            let va = vld1q_u8(a.as_ptr().add(k));
            let vb = vld1q_u8(b.as_ptr().add(k));
            // All 16 bytes equal <=> min over the equality mask is 0xFF.
            if vminvq_u8(vceqq_u8(va, vb)) != 0xFF {
                break;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    finish_scalar(a, b, i, common_len)
}