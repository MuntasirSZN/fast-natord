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

/// Skip ASCII whitespace on both sides. Marked cold to keep the hot
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
#[cfg(all(target_arch = "x86_64", not(kani)))]
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

/// AVX-512BW — 64-byte ZMM chunks, VPMOVB2M for MSB extraction.
/// Falls back through AVX2 32-byte → SSE2 16-byte → scalar for the tail.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx512bw,avx2")]
pub unsafe fn simd_is_ascii_avx512(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        while i + 64 <= s.len() {
            let chunk = _mm512_loadu_si512(s.as_ptr().add(i) as *const __m512i);
            // _mm512_movepi8_mask extracts the MSB of each byte as a 64-bit mask.
            if _mm512_movepi8_mask(chunk) != 0 {
                return false;
            }
            i += 64;
        }
        // AVX2 tail for remaining 32-byte chunk.
        while i + 32 <= s.len() {
            let chunk = _mm256_loadu_si256(s.as_ptr().add(i) as *const __m256i);
            if _mm256_movemask_epi8(chunk) as u32 != 0 {
                return false;
            }
            i += 32;
        }
        // SSE2 tail for remaining 16-byte chunk.
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

/// SSE4.1 — PTEST via [`_mm_test_all_zeros`] to detect MSB bits on
/// port 0 rather than port 5 (PMOVMSKB).  On some µarchs this frees
/// port 5 for other work.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse4.1")]
pub unsafe fn simd_is_ascii_sse41(s: &[u8]) -> bool {
    use core::arch::x86_64::*;
    unsafe {
        let mut i = 0;
        let msb = _mm_set1_epi8(0x80u8 as i8);
        while i + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(i) as *const __m128i);
            // PTEST sets ZF=1 when (chunk & msb) is all-zero → all bytes ASCII.
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

#[cfg(all(target_arch = "x86_64", not(kani)))]
cpufeatures::new!(cpuid_ascii_avx2, "avx2");
#[cfg(all(target_arch = "x86_64", not(kani)))]
cpufeatures::new!(cpuid_ascii_avx512, "avx512f", "avx512bw");

#[cfg(all(target_arch = "x86_64", not(kani)))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe {
        if cpuid_ascii_avx512::get() {
            simd_is_ascii_avx512(s)
        } else if cpuid_ascii_avx2::get() {
            simd_is_ascii_avx2(s)
        } else if cpuid_sse41::get() {
            simd_is_ascii_sse41(s)
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

#[cfg(all(target_arch = "aarch64", not(kani)))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe { simd_is_ascii_neon(s) }
}

// ── WASM SIMD ASCII helpers ─────────────────────────────────────────

/// WASM SIMD (simd128) implementation of [`simd_is_ascii`].
///
/// Processes 16 bytes per iteration using `i8x16` signed comparison:
/// bytes ≥ 128 are negative in signed interpretation, so `i8x16_lt(chunk, 0)`
/// detects non-ASCII bytes.
///
/// # Safety
///
/// `s` must be valid for reads up to `s.len()`.  Prefer calling the safe
/// [`simd_is_ascii`] wrapper.
#[cfg(all(target_feature = "simd128", target_arch = "wasm32"))]
pub unsafe fn simd_is_ascii_wasm32(s: &[u8]) -> bool {
    use core::arch::wasm32::*;
    unsafe {
        let mut i = 0;
        let zero = i8x16_splat(0);
        while i + 16 <= s.len() {
            let chunk = v128_load(s.as_ptr().add(i) as *const v128);
            // Bytes ≥ 128 are negative in signed interpretation → i8x16_lt(chunk, 0) sets bit.
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

#[cfg(all(target_feature = "simd128", target_arch = "wasm32", not(kani)))]
#[inline(always)]
unsafe fn simd_is_ascii_impl(s: &[u8]) -> bool {
    unsafe { simd_is_ascii_wasm32(s) }
}

// ── Non-SIMD fallback ──────────────────────────────────────────────

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
#[cfg(all(target_arch = "x86_64", not(kani)))]
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

/// AVX-512BW — 64-byte ZMM chunks, mask register for direct bit extraction.
/// Falls back through AVX2 32-byte → SSE2 16-byte → scalar for the tail.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "avx512bw,avx2")]
pub unsafe fn skip_avx512(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = i;
        while k + 64 <= common_len {
            let va = _mm512_loadu_si512(a.as_ptr().add(k) as *const __m512i);
            let vb = _mm512_loadu_si512(b.as_ptr().add(k) as *const __m512i);
            // _mm512_cmpeq_epi8_mask returns a __mmask64 where bit i = 1 → bytes equal.
            let mask = _mm512_cmpeq_epi8_mask(va, vb);
            if mask != 0xFFFF_FFFF_FFFF_FFFF {
                return k + (!mask).trailing_zeros() as usize;
            }
            k += 64;
        }
        // AVX2 tail for remaining 32-byte chunk.
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
        // SSE2 tail for remaining 16-byte chunk.
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

// Priority: AVX-512 > AVX2 > SSE4.2 > SSE4.1 > SSE2.
#[cfg(all(target_arch = "x86_64", not(kani)))]
cpufeatures::new!(cpuid_avx512, "avx512f", "avx512bw");
#[cfg(all(target_arch = "x86_64", not(kani)))]
cpufeatures::new!(cpuid_avx2, "avx2");
#[cfg(all(target_arch = "x86_64", not(kani)))]
cpufeatures::new!(cpuid_sse42, "sse4.2");
#[cfg(all(target_arch = "x86_64", not(kani)))]
cpufeatures::new!(cpuid_sse41, "sse4.1");

#[cfg(all(target_arch = "x86_64", not(kani)))]
#[inline(always)]
pub unsafe fn simd_skip_equal(a: &[u8], b: &[u8], i: usize, common_len: usize) -> usize {
    unsafe {
        // Short strings: skip SIMD dispatch overhead.
        if common_len < 16 {
            return finish_scalar(a, b, i, common_len);
        }
        // Priority: AVX-512 > AVX2 > SSE4.2 > SSE4.1 > SSE2.
        if cpuid_avx512::get() {
            skip_avx512(a, b, i, common_len)
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
                // Difference found inside this 16-byte chunk.
                // Bounded to ≤16 iterations, runs at most once.
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

// ── WASM SIMD skip_equal ───────────────────────────────────────────

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

// ── Other architectures (scalar only) ───────────────────────────────

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
    // SAFETY: caller guarantees preconditions for `simd_skip_equal`.
    unsafe { finish_scalar(a, b, i, common_len) }
}

// ── SIMD digit-run end scanning ─────────────────────────────────

/// Scans `s` from `start` looking for the first non-digit byte.
/// Returns the index of the first byte where [`!is_digit(b)`](is_digit).
/// Uses SIMD on x86_64 (SSE2+), AArch64 (NEON), and WASM (simd128).
/// Falls back to scalar on other architectures or for short runs (<16B).
///
/// For short inputs the overhead of the SIMD dispatch exceeds the benefit,
/// so the function falls through to a byte-by-byte scan immediately.
///
/// Guarantees `result >= start` and `result <= s.len()`.
///
/// # Safety
///
/// `start <= s.len()`. Caller must ensure `s` is valid for reads up to
/// `s.len()`.
#[inline]
pub unsafe fn simd_skip_while_digit(s: &[u8], start: usize) -> usize {
    unsafe {
        // Short inputs: byte-by-byte is faster than SIMD dispatch.
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

// ── x86_64 digit-scan backends ──────────────────────────────────

/// SSE2 — 16-byte unsigned max range check + PMOVMSKB.
#[cfg(all(target_arch = "x86_64", not(kani)))]
#[target_feature(enable = "sse2")]
pub unsafe fn skip_while_digit_sse2(s: &[u8], start: usize) -> usize {
    use core::arch::x86_64::*;
    unsafe {
        let mut k = start;
        while k + 16 <= s.len() {
            let chunk = _mm_loadu_si128(s.as_ptr().add(k) as *const __m128i);
            // Unsigned max trick: c >= '0' iff max(c, '0') == c
            let ge_0 = _mm_cmpeq_epi8(_mm_max_epu8(chunk, _mm_set1_epi8(b'0' as i8)), chunk);
            // c <= '9' iff max(c, '9') == '9'
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
/// Falls back through AVX2 32-byte → SSE2 16-byte → scalar for the tail.
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

#[cfg(all(target_arch = "x86_64", not(kani)))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    unsafe {
        // Priority: AVX-512 > AVX2 > SSE2.
        if cpuid_avx512::get() {
            skip_while_digit_avx512(s, start)
        } else if cpuid_avx2::get() {
            skip_while_digit_avx2(s, start)
        } else {
            skip_while_digit_sse2(s, start)
        }
    }
}

// ── AArch64 NEON digit-scan ───────────────────────────────────────

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
            // If any byte is non-digit, fall through to bounded scalar scan.
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

#[cfg(all(target_arch = "aarch64", not(kani)))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    unsafe { skip_while_digit_neon(s, start) }
}

// ── WASM SIMD digit-scan ────────────────────────────────────────────

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

#[cfg(all(target_feature = "simd128", target_arch = "wasm32", not(kani)))]
#[inline(always)]
unsafe fn simd_skip_while_digit_impl(s: &[u8], start: usize) -> usize {
    unsafe { skip_while_digit_wasm32(s, start) }
}

// ── Scalar digit-scan fallback ────────────────────────────────────

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

#[cfg(test)]
mod tests {
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
        // "23456789" as little-endian u64
        assert_eq!(val, 0x3938373635343332);
    }

    #[test]
    fn test_load_u128() {
        let data = [
            0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
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
        // 8 bytes, differ in last byte.
        // 16-byte check skipped, 8-byte check finds diff → chunk start (0)
        let a = b"1234567A";
        let b = b"1234567B";
        unsafe {
            assert_eq!(finish_scalar(a, b, 0, 8), 0);
        }
    }

    #[test]
    fn test_finish_scalar_diff_within_16byte_chunk() {
        // 16 bytes, differ in last byte.
        // 16-byte chunk load detects diff, breaks into 8-byte loop which
        // advances past the first equal 8 bytes before finding the diff.
        let a = b"abcdefghijklmnoP";
        let b = b"abcdefghijklmnoQ";
        unsafe {
            // u128 at 0: differ → break (k=0)
            // u64 at 0: "abcdefgh" equal → k=8
            // u64 at 8: "ijklmnoP" vs "ijklmnoQ" differ → break, return 8
            assert_eq!(finish_scalar(a, b, 0, 16), 8);
        }
    }

    #[test]
    fn test_finish_scalar_diff_after_16_in_8byte_chunk() {
        // First 16 equal (one 16-byte chunk), then diff within 8-byte chunk.
        let a = b"abcdefghijklmnop1234567A";
        let b = b"abcdefghijklmnop1234567B";
        unsafe {
            // 16-byte check at 0: passes, k=16.
            // 8-byte check at 16: finds diff, breaks, returns 16.
            let k = finish_scalar(a, b, 0, 24);
            assert_eq!(k, 16);
        }
    }

    #[test]
    fn test_finish_scalar_short_below_8() {
        // < 8 bytes: no 8-byte or 16-byte chunks possible, returns start offset.
        let a = b"ab";
        let b = b"ab";
        unsafe {
            assert_eq!(finish_scalar(a, b, 0, 2), 0);
        }

        let a = b"a";
        let b = b"b";
        unsafe {
            assert_eq!(finish_scalar(a, b, 0, 1), 0);
        }
    }

    #[test]
    fn test_simd_skip_equal_identical_exact_stride() {
        // Multiple of 16: SIMD covers completely.
        let a = b"abcdefghijklmnop"; // 16 bytes
        unsafe {
            assert_eq!(simd_skip_equal(a, a, 0, 16), 16);
        }
        let a = b"abcdefghijklmnop1234"; // 20 bytes, tail <8 not advanced by finish_scalar
        unsafe {
            // SIMD covers 0..16, finish_scalar(16,20): 16+8=24>20 → returns 16
            assert_eq!(simd_skip_equal(a, a, 0, 20), 16);
        }
    }

    /// Verify that `simd_skip_equal` returns a position ≤ the first diff.
    ///
    /// SIMD backends pinpoint the exact byte; the scalar fallback returns
    /// the chunk (16/8 byte) boundary before the diff.  Both are correct,
    /// so we only assert the upper bound plus the prefix-equal invariant.
    unsafe fn check_skip_upper_bound(a: &[u8], b: &[u8], common_len: usize, max_expected: usize) {
        unsafe {
            let k = simd_skip_equal(a, b, 0, common_len);
            assert!(
                k <= max_expected,
                "skip returned {k} but first diff ≤ {max_expected}"
            );
            // All bytes before k must be equal.
            for i in 0..k {
                assert_eq!(a[i], b[i], "byte {i} differs but skip returned {k}");
            }
        }
    }

    #[test]
    fn test_simd_skip_equal_diff_first_chunk() {
        let a = b"abcdeFghijklmnop";
        let b = b"abcdeGghijklmnop";
        // First differing byte is at index 5.
        unsafe { check_skip_upper_bound(a, b, 16, 5) }
    }

    #[test]
    fn test_simd_skip_equal_diff_at_16() {
        let a = b"abcdefghijklmnoPX";
        let b = b"abcdefghijklmnoQY";
        // First differing byte is at index 15 (within 16-byte arg).
        unsafe { check_skip_upper_bound(a, b, 16, 15) }
    }

    #[test]
    fn test_simd_skip_equal_diff_in_tail() {
        // Diff within <8-byte tail: finish_scalar can't advance, returns chunk start.
        let a = b"abcdefghijklmnop12345";
        let b = b"abcdefghijklmnop123XY";
        unsafe {
            // SIMD covers 0..16 (all equal), then finish_scalar(16,20):
            // 16+8=24>20 → returns 16.
            let k = simd_skip_equal(a, b, 0, 20);
            assert_eq!(k, 16);
        }
    }

    #[test]
    fn test_simd_skip_equal_short() {
        // < 8 bytes: no SIMD stride, finish_scalar can't skip → returns 0.
        let a = b"short";
        let b = b"shXrt";
        unsafe {
            assert_eq!(simd_skip_equal(a, b, 0, 5), 0);
        }

        let a = b"ab";
        unsafe {
            assert_eq!(simd_skip_equal(a, a, 0, 2), 0);
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn is_digit_matches_spec() {
        let c: u8 = kani::any();
        let spec = (b'0'..=b'9').contains(&c);
        assert_eq!(is_digit(c), spec);
    }

    #[kani::proof]
    fn is_ascii_ws_matches_spec() {
        let c: u8 = kani::any();
        let spec = c == b' ' || c == b'\t' || c == b'\n' || c == b'\x0C' || c == b'\r';
        assert_eq!(is_ascii_ws(c), spec);
    }

    #[kani::proof]
    fn load_u64_in_bounds() {
        const N: usize = 24;
        let data: [u8; N] = kani::any();
        let i: usize = kani::any();
        kani::assume(i <= N - 8);
        let _ = unsafe { load_u64(&data, i) };
    }

    #[kani::proof]
    fn load_u128_in_bounds() {
        const N: usize = 40;
        let data: [u8; N] = kani::any();
        let i: usize = kani::any();
        kani::assume(i <= N - 16);
        let _ = unsafe { load_u128(&data, i) };
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

        let r = unsafe { finish_scalar(&a, &b, k, common_len) };

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

        let r = unsafe { simd_skip_equal(&a, &b, 0, common_len) };

        assert!(r <= common_len);
        let mut idx = 0;
        while idx < r {
            assert_eq!(a[idx], b[idx]);
            idx += 1;
        }
        // Maximality: if the skip stopped short of common_len, there must
        // be a differing byte somewhere at or after r.  The scalar fallback
        // (used by Kani) returns a *chunk* boundary (8/16 bytes), not the
        // exact differing byte, so we cannot assert a[r] != b[r] here.
    }

    #[cfg(target_arch = "x86_64")]
    #[kani::proof]
    #[kani::unwind(28)]
    fn skip_sse2_contract() {
        let a: [u8; SK_LEN] = kani::any();
        let b: [u8; SK_LEN] = kani::any();
        let common_len: usize = kani::any();
        kani::assume(common_len <= SK_LEN);

        let r = unsafe { skip_sse2(&a, &b, 0, common_len) };

        assert!(r <= common_len);
        let mut idx = 0;
        while idx < r {
            assert_eq!(a[idx], b[idx]);
            idx += 1;
        }
        // NOTE: Kani models SIMD intrinsics (simd_bitmask_impl)
        // over-approximately, so it cannot prove that the SSE2
        // pinpointed byte r is the exact differing byte.
    }
}
