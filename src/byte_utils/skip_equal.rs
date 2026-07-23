//! SIMD-accelerated common-prefix skipping.
//!
//! Provides [`simd_skip_equal`] which finds the first differing byte
//! between two byte slices, using SIMD on supported architectures.
//!
//! Arch-specific backends:
//! - x86_64: SSE2, SSE4.1, SSE4.2, AVX2, AVX-512BW (via dispatch table)
//! - AArch64: NEON
//! - WASM: simd128
//! - Other: scalar fallback

use super::basic::finish_scalar;
// ── Scalar tail (shared by all SIMD backends) ───────────────────────

// `finish_scalar` is re-exported from `basic` for use by sibling modules.

// ═══════════════════════════════════════════════════════════════════
// x86_64 ISA backends
// ═══════════════════════════════════════════════════════════════════

/// SSE2 — 16-byte PCMPEQB + PMOVMSKB + tzcnt.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
pub unsafe fn skip_sse2(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(va, vb));
            if mask != 0xFFFF {
                return k + (!(mask as u32)).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// SSE4.1 — PXOR + PTEST to detect inequality without PMOVMSKB on the
/// fast path.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse4.1")]
pub unsafe fn skip_sse41(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            let neq = _mm_xor_si128(va, vb);
            if _mm_test_all_zeros(neq, neq) == 0 {
                let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(va, vb));
                return k + (!(mask as u32)).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// SSE4.2 — PCMPISTRI returns diff index directly, saving TZCNT.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse4.2")]
pub unsafe fn skip_sse42(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            let idx = _mm_cmpistri(va, vb, _SIDD_CMP_EQUAL_EACH | _SIDD_NEGATIVE_POLARITY);
            if idx != 16 {
                return k + idx as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// AVX2 — 32-byte YMM chunks.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx2")]
pub unsafe fn skip_avx2(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 32 <= common_len {
            let va = _mm256_loadu_si256(a.as_ptr().add(k) as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr().add(k) as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(va, vb);
            let mask = _mm256_movemask_epi8(cmp) as u32;
            if mask != 0xFFFF_FFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 32;
        }
        // SSE2 fallback for remaining 16-byte data.
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(va, vb));
            if mask != 0xFFFF {
                return k + (!(mask as u32)).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// AVX-512BW — 64-byte ZMM chunks, mask register, AVX2/SSE2 tail.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx512bw,avx2")]
pub unsafe fn skip_avx512(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 64 <= common_len {
            let va = _mm512_loadu_si512(a.as_ptr().add(k) as *const __m512i);
            let vb = _mm512_loadu_si512(b.as_ptr().add(k) as *const __m512i);
            let mask = _mm512_cmpeq_epi8_mask(va, vb);
            if mask != 0xFFFF_FFFF_FFFF_FFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 64;
        }
        while k + 32 <= common_len {
            let va = _mm256_loadu_si256(a.as_ptr().add(k) as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr().add(k) as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(va, vb);
            let mask = _mm256_movemask_epi8(cmp) as u32;
            if mask != 0xFFFF_FFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 32;
        }
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(va, vb));
            if mask != 0xFFFF {
                return k + (!(mask as u32)).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

// ═══════════════════════════════════════════════════════════════════
// x86_64 — dispatch through unified table
// ═══════════════════════════════════════════════════════════════════

#[cfg(all(target_arch = "x86_64", not(kani)))]
#[inline(always)]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    unsafe {
        // Short strings: skip SIMD function call overhead.
        if common_len < 32 {
            return finish_scalar(a, b, i, common_len);
        }
        (super::dispatch::get_dispatch().skip_equal)(a, b, i, common_len)
    }
}

// ═══════════════════════════════════════════════════════════════════
// AArch64 NEON
// ═══════════════════════════════════════════════════════════════════

#[cfg(all(target_arch = "aarch64", not(kani)))]
#[target_feature(enable = "neon")]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::aarch64::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = vld1q_u8(a.as_ptr().add(k));
            let vb = vld1q_u8(b.as_ptr().add(k));
            if vminvq_u8(vceqq_u8(va, vb)) != 0xFF {
                while a[k] == b[k] {
                    k += 1;
                }
                return k;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

// ═══════════════════════════════════════════════════════════════════
// WASM simd128
// ═══════════════════════════════════════════════════════════════════

#[cfg(all(target_feature = "simd128", target_arch = "wasm32", not(kani)))]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::wasm32::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = v128_load(a.as_ptr().add(k) as *const v128);
            let vb = v128_load(b.as_ptr().add(k) as *const v128);
            let eq = u8x16_eq(va, vb);
            let mask = i8x16_bitmask(eq) as u16;
            if mask != 0xFFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Scalar fallback (kani, non-SIMD architectures)
// ═══════════════════════════════════════════════════════════════════

#[cfg(any(
    kani,
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "wasm32"
    )),
    all(target_arch = "wasm32", not(target_feature = "simd128")),
))]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    unsafe { finish_scalar(a, b, i, common_len) }
}
