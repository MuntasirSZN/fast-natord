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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_simd_skip_equal_diff_first_chunk() {
        let a = b"abcdeFghijklmnop";
        let b = b"abcdeGghijklmnop";
        unsafe {
            assert_eq!(simd_skip_equal(a, b, 0, 16), 5);
        }
    }

    #[test]
    fn test_simd_skip_equal_diff_at_16() {
        let a = b"abcdefghijklmnoPX";
        let b = b"abcdefghijklmnoQY";
        unsafe {
            let k = simd_skip_equal(a, b, 0, 16);
            assert_eq!(k, 15);
        }
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
