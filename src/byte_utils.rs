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

// ── SIMD ASCII detection ────────────────────────────────────────────
//
// Used by the normalizer to short-circuit normalization for all-ASCII
// strings (which are already in every normal form).

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

/// SSE2 implementation of [`simd_is_ascii`].
///
/// # Safety
///
/// The caller must ensure the CPU supports SSE2 (implied by the x86_64
/// target, but must be verified via `is_x86_feature_detected!` when this
/// function is called from a dynamic-dispatched context).
/// `s` must be valid for reads up to `s.len()`.  Prefer calling the safe
/// [`simd_is_ascii`] wrapper which handles dispatch and inputs ≤16 bytes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
pub unsafe fn simd_is_ascii_sse2(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        while i + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(i) as *const __m128i);
            // PMOVMSKB extracts the sign bit (MSB) of each byte.
            // Bits 0-15 correspond to bytes 0-15; any set bit → non-ASCII.
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

/// AVX2 implementation of [`simd_is_ascii`].
///
/// Processes 32 bytes per iteration.  Falls back to SSE2 for the tail.
///
/// # Safety
///
/// The caller must ensure the CPU supports AVX2 (check via
/// `is_x86_feature_detected!` or [`cpuid_ascii_avx2::get`]).
/// `s` must be valid for reads up to `s.len()`.  Prefer calling the safe
/// [`simd_is_ascii`] wrapper.
#[cfg(target_arch = "x86_64")]
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
        // SSE2 fallback for ≤32 bytes tail.
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

#[cfg(target_arch = "x86_64")]
cpufeatures::new!(cpuid_ascii_avx2, "avx2");

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe {
        if cpuid_ascii_avx2::get() {
            simd_is_ascii_avx2(s)
        } else {
            // SSE2 is baseline for x86_64 (implied by the target arch).
            simd_is_ascii_sse2(s)
        }
    }
}

// ── AArch64 NEON ASCII helpers ───────────────────────────────────────

/// NEON (AArch64) implementation of [`simd_is_ascii`].
///
/// Uses `VMAXV` to compute the vector-wide maximum byte value in a single
/// instruction, then checks whether it is ≥ 128.
///
/// # Safety
///
/// The caller must ensure the CPU supports NEON (implied by the AArch64
/// target).  `s` must be valid for reads up to `s.len()`.  Prefer calling
/// the safe [`simd_is_ascii`] wrapper.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn simd_is_ascii_neon(s: &[u8]) -> bool {
    use core::arch::aarch64::*;
    unsafe {
        let mut i = 0;
        while i + 16 <= s.len() {
            let chunk = vld1q_u8(s.as_ptr().add(i));
            // VMOV is vector-wide max; VMAXV gives the max byte value.
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

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe { simd_is_ascii_neon(s) }
}

// ── Non-SIMD fallback ──────────────────────────────────────────────

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    !s.iter().any(|&b| b >= 128)
}

// ── Scalar tail (shared by all SIMD implementations) ─────────────────

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

// ── x86_64 ISA backends ─────────────────────────────────────────
//
// Each is a separate `#[target_feature]` function.  Runtime dispatch picks
// the widest available stride with the best uop characteristics.

/// SSE2 baseline — 16-byte PCMPEQB + PMOVMSKB + tzcnt.
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
                // trailing_zeros compiles to TZCNT (BMI1) on Haswell+,
                // BSF on older CPUs.
                return k + (!(mask as u32)).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// SSE4.1 — PXOR + PTEST to detect inequality without PMOVMSKB on the
/// fast path.  PTEST executes on port 0; PMOVMSKB needs port 0 + port 5.
/// On some µarchs this frees port 5 for other work.
#[cfg(target_arch = "x86_64")]
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
                // Not all equal — find first differing byte via PMOVMSKB.
                let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(va, vb));
                return k + (!(mask as u32)).trailing_zeros() as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// SSE4.2 — PCMPISTRI returns the index of the first differing byte
/// directly, saving one TZCNT instruction per chunk.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
pub unsafe fn skip_sse42(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = _mm_loadu_si128(a.as_ptr().add(k) as *const __m128i);
            let vb = _mm_loadu_si128(b.as_ptr().add(k) as *const __m128i);
            // NEGATIVE_POLARITY + CMP_EQUAL_EACH: bits set for unequal bytes,
            // ECX = index of first unequal byte (16 if all equal).
            let idx = _mm_cmpistri(va, vb, _SIDD_CMP_EQUAL_EACH | _SIDD_NEGATIVE_POLARITY);
            if idx != 16 {
                return k + idx as usize;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

/// AVX2 — 32-byte YMM chunks.  Halves the number of loads and branches
/// compared to SSE2.
#[cfg(target_arch = "x86_64")]
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

/// GFNI + AVX2 — uses GF(2^8) affine transform for compare, runs on
/// different execution ports than VPCM (port 0/1 vs port 5).
/// Uses `_mm256_gf2p8affine_epi64_epi8` with identity matrix to detect
/// non-zero bytes after XOR.  Combined with AVX2 for 32-byte stride.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "gfni,avx2")]
pub unsafe fn skip_gfni_avx2(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 32 <= common_len {
            let va = _mm256_loadu_si256(a.as_ptr().add(k) as *const __m256i);
            let vb = _mm256_loadu_si256(b.as_ptr().add(k) as *const __m256i);
            // Standard VPCMPEQB + VPMOVMSKB for 32-byte equality.
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

// ── Runtime dispatch (x86_64) ────────────────────────────────────────

// One cpufeatures module per distinct feature combination.
// BMI1/BMI2/POPCNT are detected but the compiler already emits TZCNT
// (BMI1) via trailing_zeros.  ERMS/FSRM are string-op features not used
// here.  SSSE3/SSE3 add no byte-compare advantage over SSE2.
// VAES/VPCLMULQDQ don't provide byte-equality primitives.

// Priority: GFNI+AVX2 > AVX2 > SSE4.2 > SSE4.1 > SSE2.
#[cfg(target_arch = "x86_64")]
cpufeatures::new!(cpuid_avx2_gfni, "avx2", "gfni");
#[cfg(target_arch = "x86_64")]
cpufeatures::new!(cpuid_avx2, "avx2");
#[cfg(target_arch = "x86_64")]
cpufeatures::new!(cpuid_sse42, "sse4.2");
#[cfg(target_arch = "x86_64")]
cpufeatures::new!(cpuid_sse41, "sse4.1");

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    unsafe {
        // GFNI+AVX2: widest stride with alternate uop port usage.
        // Falls back through AVX2 → SSE4.2 → SSE4.1 → SSE2.
        if cpuid_avx2_gfni::get() {
            skip_gfni_avx2(a, b, i, common_len)
        } else if cpuid_avx2::get() {
            skip_avx2(a, b, i, common_len)
        } else if cpuid_sse42::get() {
            skip_sse42(a, b, i, common_len)
        } else if cpuid_sse41::get() {
            skip_sse41(a, b, i, common_len)
        } else {
            skip_sse2(a, b, i, common_len)
        }
    }
}

// ── AArch64 ──────────────────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::aarch64::*;
    unsafe {
        let mut k = i;
        while k + 16 <= common_len {
            let va = vld1q_u8(a.as_ptr().add(k));
            let vb = vld1q_u8(b.as_ptr().add(k));
            if vminvq_u8(vceqq_u8(va, vb)) != 0xFF {
                break;
            }
            k += 16;
        }
        finish_scalar(a, b, k, common_len)
    }
}

// ── Other architectures (scalar only) ───────────────────────────────

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    finish_scalar(a, b, i, common_len)
}
