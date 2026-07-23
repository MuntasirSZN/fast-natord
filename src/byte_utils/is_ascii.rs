//! SIMD-accelerated ASCII detection.
//!
//! Provides [`simd_is_ascii`] with optimised backends for x86_64 (SSE2+,
//! via dispatch table), AArch64 (NEON), WASM (simd128), and a portable
//! scalar fallback.

// ── SSE2 (x86_64 baseline) ─────────────────────────────────────────

/// SSE2 — 16-byte PCMPEQB + PMOVMSKB.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse2")]
pub unsafe fn simd_is_ascii_sse2(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        while i + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(i) as *const __m128i);
            if _mm_movemask_epi8(chunk) != 0 {
                return false;
            }
            i += 16;
        }
        while i < s.len() {
            if s[i] >= 128 {
                return false;
            }
            i += 1;
        }
        true
    }
}

// ── SSE4.1 (PTEST) ────────────────────────────────────────────────

/// SSE4.1 — PTEST via `_mm_test_all_zeros` on port 0 rather than port 5.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse4.1")]
pub unsafe fn simd_is_ascii_sse41(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        let msb = _mm_set1_epi8(0x80u8 as i8);
        while i + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(i) as *const __m128i);
            if _mm_test_all_zeros(chunk, msb) == 0 {
                return false;
            }
            i += 16;
        }
        while i < s.len() {
            if s[i] >= 128 {
                return false;
            }
            i += 1;
        }
        true
    }
}

// ── AVX2 (32-byte) ────────────────────────────────────────────────

/// AVX2 — 32-byte YMM chunks, SSE2 tail.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx2")]
pub unsafe fn simd_is_ascii_avx2(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        while i + 32 <= s.len() {
            let chunk = _mm256_loadu_si256(s.as_ptr().add(i) as *const __m256i);
            if _mm256_movemask_epi8(chunk) as u32 != 0 {
                return false;
            }
            i += 32;
        }
        while i + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(i) as *const __m128i);
            if _mm_movemask_epi8(chunk) != 0 {
                return false;
            }
            i += 16;
        }
        while i < s.len() {
            if s[i] >= 128 {
                return false;
            }
            i += 1;
        }
        true
    }
}

// ── AVX-512BW (64-byte) ───────────────────────────────────────────

/// AVX-512BW — 64-byte ZMM chunks, VPMOVB2M, AVX2/SSE2 tail.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx512bw,avx2")]
pub unsafe fn simd_is_ascii_avx512(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        while i + 64 <= s.len() {
            let chunk = _mm512_loadu_si512(s.as_ptr().add(i) as *const __m512i);
            if _mm512_movepi8_mask(chunk) != 0 {
                return false;
            }
            i += 64;
        }
        while i + 32 <= s.len() {
            let chunk = _mm256_loadu_si256(s.as_ptr().add(i) as *const __m256i);
            if _mm256_movemask_epi8(chunk) as u32 != 0 {
                return false;
            }
            i += 32;
        }
        while i + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(i) as *const __m128i);
            if _mm_movemask_epi8(chunk) != 0 {
                return false;
            }
            i += 16;
        }
        while i < s.len() {
            if s[i] >= 128 {
                return false;
            }
            i += 1;
        }
        true
    }
}

// ── AArch64 NEON ───────────────────────────────────────────────────

/// NEON — VMAXV for vector-wide max byte value.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn simd_is_ascii_neon(s: &[u8]) -> bool {
    use core::arch::aarch64::*;
    unsafe {
        let mut i = 0;
        while i + 16 <= s.len() {
            let chunk = vld1q_u8(s.as_ptr().add(i));
            if vmaxvq_u8(chunk) >= 128 {
                return false;
            }
            i += 16;
        }
        while i < s.len() {
            if s[i] >= 128 {
                return false;
            }
            i += 1;
        }
        true
    }
}

// ── WASM simd128 ───────────────────────────────────────────────────

/// simd128 — i8x16 signed comparison against zero.
#[cfg(all(target_feature = "simd128", target_arch = "wasm32"))]
pub unsafe fn simd_is_ascii_wasm32(s: &[u8]) -> bool {
    use core::arch::wasm32::*;
    unsafe {
        let mut i = 0;
        let zero = i8x16_splat(0);
        while i + 16 <= s.len() {
            let chunk = v128_load(s.as_ptr().add(i) as *const v128);
            if i8x16_bitmask(i8x16_lt(chunk, zero)) != 0 {
                return false;
            }
            i += 16;
        }
        while i < s.len() {
            if s[i] >= 128 {
                return false;
            }
            i += 1;
        }
        true
    }
}

// ── Per-arch dispatch helpers ──────────────────────────────────────

#[cfg(all(target_arch = "x86_64", not(kani)))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe { (super::dispatch::get_dispatch().is_ascii)(s) }
}

#[cfg(all(target_arch = "aarch64", not(kani)))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe { simd_is_ascii_neon(s) }
}

#[cfg(all(target_feature = "simd128", target_arch = "wasm32", not(kani)))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe { simd_is_ascii_wasm32(s) }
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
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    !s.iter().any(|&b| b >= 128)
}

// ── Public API ─────────────────────────────────────────────────────

/// Returns `true` if every byte in `s` has bit 7 clear.
///
/// Uses SIMD on x86_64 (SSE2+) and AArch64 (NEON).  Falls back to scalar
/// on other architectures.
///
/// This is the fast path used by the [`Normalizer`](crate::Normalizer) to
/// short-circuit normalisation for all-ASCII strings.
#[inline]
pub fn simd_is_ascii(s: &[u8]) -> bool {
    if s.len() < 16 {
        return !s.iter().any(|&b| b >= 128);
    }
    unsafe { simd_is_ascii_impl(s) }
}
