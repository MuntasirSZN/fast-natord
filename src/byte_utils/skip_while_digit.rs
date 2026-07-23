//! SIMD-accelerated digit-run end scanning.
//!
//! Finds the first non-digit byte in a byte slice, using SIMD on
//! supported architectures.  Also provides a combined two-string
//! scanner and a short-path scalar scanner.

use super::basic::is_digit;

// ═══════════════════════════════════════════════════════════════════
// x86_64 digit-scan backends
// ═══════════════════════════════════════════════════════════════════

/// SSE2 — 16-byte unsigned max range check + PMOVMSKB.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse2")]
pub unsafe fn skip_while_digit_sse2(s: &[u8], start: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = start;
        while k + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(k) as *const __m128i);
            let ge_0 = _mm_cmpeq_epi8(_mm_max_epu8(chunk, _mm_set1_epi8(b'0' as i8)), chunk);
            let le_9 = _mm_cmpeq_epi8(
                _mm_max_epu8(chunk, _mm_set1_epi8(b'9' as i8)),
                _mm_set1_epi8(b'9' as i8),
            );
            let digit = _mm_and_si128(ge_0, le_9);
            let mask = _mm_movemask_epi8(digit) as u16 as u32;
            if mask != 0xFFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 16;
        }
        while k < s.len() && is_digit(s[k]) {
            k += 1;
        }
        k
    }
}

/// AVX2 — 32-byte vectors with SSE2 tail.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx2")]
pub unsafe fn skip_while_digit_avx2(s: &[u8], start: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = start;
        while k + 32 <= s.len() {
            let chunk = _mm256_loadu_si256(s.as_ptr().add(k) as *const __m256i);
            let ge_0 =
                _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, _mm256_set1_epi8(b'0' as i8)), chunk);
            let le_9 = _mm256_cmpeq_epi8(
                _mm256_max_epu8(chunk, _mm256_set1_epi8(b'9' as i8)),
                _mm256_set1_epi8(b'9' as i8),
            );
            let digit = _mm256_and_si256(ge_0, le_9);
            let mask = _mm256_movemask_epi8(digit) as u32;
            if mask != 0xFFFF_FFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 32;
        }
        // SSE2 tail for the remaining 16-byte chunk.
        while k + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(k) as *const __m128i);
            let ge_0 = _mm_cmpeq_epi8(_mm_max_epu8(chunk, _mm_set1_epi8(b'0' as i8)), chunk);
            let le_9 = _mm_cmpeq_epi8(
                _mm_max_epu8(chunk, _mm_set1_epi8(b'9' as i8)),
                _mm_set1_epi8(b'9' as i8),
            );
            let digit = _mm_and_si128(ge_0, le_9);
            let mask = _mm_movemask_epi8(digit) as u16 as u32;
            if mask != 0xFFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 16;
        }
        while k < s.len() && is_digit(s[k]) {
            k += 1;
        }
        k
    }
}

/// AVX-512BW — 64-byte ZMM chunks with mask-register range check.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx512bw,avx2")]
pub unsafe fn skip_while_digit_avx512(s: &[u8], start: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = start;
        while k + 64 <= s.len() {
            let chunk = _mm512_loadu_si512(s.as_ptr().add(k) as *const __m512i);
            let ge_0 =
                _mm512_cmpeq_epi8_mask(_mm512_max_epu8(chunk, _mm512_set1_epi8(b'0' as i8)), chunk);
            let le_9 = _mm512_cmpeq_epi8_mask(
                _mm512_max_epu8(chunk, _mm512_set1_epi8(b'9' as i8)),
                _mm512_set1_epi8(b'9' as i8),
            );
            let digit = ge_0 & le_9;
            if digit != 0xFFFF_FFFF_FFFF_FFFF {
                return k + (!digit).trailing_zeros() as usize;
            }
            k += 64;
        }
        // AVX2 tail for remaining 32-byte chunk.
        while k + 32 <= s.len() {
            let chunk = _mm256_loadu_si256(s.as_ptr().add(k) as *const __m256i);
            let ge_0 =
                _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, _mm256_set1_epi8(b'0' as i8)), chunk);
            let le_9 = _mm256_cmpeq_epi8(
                _mm256_max_epu8(chunk, _mm256_set1_epi8(b'9' as i8)),
                _mm256_set1_epi8(b'9' as i8),
            );
            let digit = _mm256_and_si256(ge_0, le_9);
            let mask = _mm256_movemask_epi8(digit) as u32;
            if mask != 0xFFFF_FFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 32;
        }
        // SSE2 tail for remaining 16-byte chunk.
        while k + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(k) as *const __m128i);
            let ge_0 = _mm_cmpeq_epi8(_mm_max_epu8(chunk, _mm_set1_epi8(b'0' as i8)), chunk);
            let le_9 = _mm_cmpeq_epi8(
                _mm_max_epu8(chunk, _mm_set1_epi8(b'9' as i8)),
                _mm_set1_epi8(b'9' as i8),
            );
            let digit = _mm_and_si128(ge_0, le_9);
            let mask = _mm_movemask_epi8(digit) as u16 as u32;
            if mask != 0xFFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 16;
        }
        while k < s.len() && is_digit(s[k]) {
            k += 1;
        }
        k
    }
}

// ═══════════════════════════════════════════════════════════════════
// AArch64 NEON digit-scan
// ═══════════════════════════════════════════════════════════════════

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn skip_while_digit_neon(s: &[u8], start: usize) -> usize {
    use core::arch::aarch64::*;
    unsafe {
        let mut k = start;
        while k + 16 <= s.len() {
            let chunk = vld1q_u8(s.as_ptr().add(k));
            let ge_0 = vceqq_u8(vmaxq_u8(chunk, vdupq_n_u8(b'0')), chunk);
            let le_9 = vceqq_u8(vmaxq_u8(chunk, vdupq_n_u8(b'9')), vdupq_n_u8(b'9'));
            let digit = vandq_u8(ge_0, le_9);
            if vminvq_u8(digit) != 0xFF {
                while k < s.len() && is_digit(s[k]) {
                    k += 1;
                }
                return k;
            }
            k += 16;
        }
        while k < s.len() && is_digit(s[k]) {
            k += 1;
        }
        k
    }
}

// ═══════════════════════════════════════════════════════════════════
// WASM simd128 digit-scan
// ═══════════════════════════════════════════════════════════════════

#[cfg(all(target_feature = "simd128", target_arch = "wasm32", not(kani)))]
pub unsafe fn skip_while_digit_wasm32(s: &[u8], start: usize) -> usize {
    use core::arch::wasm32::*;
    unsafe {
        let mut k = start;
        let zero = u8x16_splat(b'0');
        let nine = u8x16_splat(b'9');
        while k + 16 <= s.len() {
            let chunk = v128_load(s.as_ptr().add(k) as *const v128);
            let ge_0 = u8x16_ge(chunk, zero);
            let le_9 = u8x16_le(chunk, nine);
            let digit = v128_and(ge_0, le_9);
            let mask = i8x16_bitmask(digit) as u16;
            if mask != 0xFFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 16;
        }
        while k < s.len() && is_digit(s[k]) {
            k += 1;
        }
        k
    }
}

// ═══════════════════════════════════════════════════════════════════
// Per-arch dispatch helpers
// ═══════════════════════════════════════════════════════════════════

#[cfg(all(target_arch = "x86_64", not(kani)))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    unsafe { (super::dispatch::get_dispatch().skip_while_digit)(s, start) }
}

#[cfg(all(target_arch = "aarch64", not(kani)))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    unsafe { skip_while_digit_neon(s, start) }
}

#[cfg(all(target_feature = "simd128", target_arch = "wasm32", not(kani)))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    unsafe { skip_while_digit_wasm32(s, start) }
}

#[cfg(any(
    kani,
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "wasm32"
    )),
    all(target_arch = "wasm32", not(target_feature = "simd128")),
))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    let mut k = start;
    while k < s.len() && is_digit(s[k]) {
        k += 1;
    }
    k
}

// ═══════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════

/// Scans `s` from `start` looking for the first non-digit byte.
///
/// Returns the index of the first byte where `!is_digit(b)`.
/// Uses SIMD on supported architectures; falls back to scalar for
/// short runs (<16B).
///
/// Guarantees `result >= start` and `result <= s.len()`.
///
/// # Safety
///
/// `start <= s.len()`.
#[inline(always)]
pub unsafe fn simd_skip_while_digit(s: &[u8], start: usize) -> usize {
    unsafe {
        if s.len() - start < 16 {
            let mut k = start;
            while k < s.len() && is_digit(s[k]) {
                k += 1;
            }
            return k;
        }
        simd_skip_while_digit_impl(s, start)
    }
}

/// Scan two strings from `start` for their digit-run end simultaneously.
///
/// For the common case of both strings having short digit runs (<16 bytes),
/// this uses a single-pass byte-by-byte scan that avoids the overhead of
/// two separate `simd_skip_while_digit` calls.
///
/// Returns `(end_a, end_b)` — the index of the first non-digit byte in
/// each string.
///
/// # Safety
///
/// `start_a` must be ≤ `a.len()`, `start_b` ≤ `b.len()`.
#[inline(always)]
pub unsafe fn digit_run_ends_short(
    a: &[u8],
    b: &[u8],
    start_a: usize,
    start_b: usize,
) -> (usize, usize) {
    unsafe {
        let end_a = a.len();
        let end_b = b.len();
        let mut ea = start_a;
        while ea < end_a && is_digit(*a.get_unchecked(ea)) {
            ea += 1;
        }
        let mut eb = start_b;
        while eb < end_b && is_digit(*b.get_unchecked(eb)) {
            eb += 1;
        }
        (ea, eb)
    }
}

/// Combined two-string digit-run end scanning.
///
/// For long runs (≥16 bytes on either side) dispatches SIMD; for short
/// runs falls through to [`digit_run_ends_short`].
///
/// Returns `(end_a, end_b)`.
///
/// # Safety
///
/// `start_a` must be ≤ `a.len()`, `start_b` ≤ `b.len()`.
#[inline(always)]
pub unsafe fn simd_skip_while_digit_both(
    a: &[u8],
    b: &[u8],
    start_a: usize,
    start_b: usize,
) -> (usize, usize) {
    let rem_a = a.len() - start_a;
    let rem_b = b.len() - start_b;
    if rem_a < 16 && rem_b < 16 {
        unsafe { digit_run_ends_short(a, b, start_a, start_b) }
    } else {
        let end_a = unsafe { simd_skip_while_digit(a, start_a) };
        let end_b = unsafe { simd_skip_while_digit(b, start_b) };
        (end_a, end_b)
    }
}
