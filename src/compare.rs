use crate::byte_utils;
use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

/// Case-sensitive natural order compare on byte slices.
///
/// Uses SIMD to skip common prefix, then a pointer-based scalar
/// loop with numeric-run awareness.  Equal-length digit runs use
/// word-at-a-time comparison (XOR + trailing_zeros).
#[inline(always)]
pub fn compare_impl(a: &[u8], b: &[u8]) -> Ordering {
    if a.len() == b.len() && a.as_ptr() == b.as_ptr() {
        return Equal;
    }

    let len_a = a.len();
    let len_b = b.len();
    let common_len = len_a.min(len_b);
    let adv = unsafe { byte_utils::simd_skip_equal(a, b, 0, common_len) };

    // Tracks whether the last consumed aligned-and-equal byte was a digit.
    // When the first differing bytes have mixed digit/non-digit types this
    // tells us the digit run continues on one side but ended on the other.
    // Initialised from the SIMD-skipped prefix and updated as the loop
    // processes matching bytes.
    let mut last_eq_digit = adv > 0 && byte_utils::is_digit(a[adv - 1]);

    // SAFETY: adv ≤ common_len ≤ both lengths.
    let mut pa = unsafe { a.as_ptr().add(adv) };
    let mut pb = unsafe { b.as_ptr().add(adv) };
    let enda = unsafe { a.as_ptr().add(len_a) };
    let endb = unsafe { b.as_ptr().add(len_b) };

    loop {
        // Cold path: one or both current bytes are whitespace.
        // Hottest case (no whitespace in typical filenames) falls through.
        unsafe {
            if (pa < enda && byte_utils::is_ascii_ws(*pa))
                || (pb < endb && byte_utils::is_ascii_ws(*pb))
            {
                while pa < enda && byte_utils::is_ascii_ws(*pa) {
                    pa = pa.add(1);
                }
                while pb < endb && byte_utils::is_ascii_ws(*pb) {
                    pb = pb.add(1);
                }
            }
        }

        if pa >= enda || pb >= endb {
            let rem_a = (enda as usize).wrapping_sub(pa as usize);
            let rem_b = (endb as usize).wrapping_sub(pb as usize);
            return rem_a.cmp(&rem_b);
        }

        let ca;
        let cb;
        unsafe {
            ca = *pa;
            cb = *pb;
        }

        if byte_utils::is_digit(ca) && byte_utils::is_digit(cb) {
            // Both sides are digits.
            let la0 = ca == b'0';
            let lb0 = cb == b'0';

            if la0 || lb0 {
                // Left-aligned: compare char-by-char up to the shorter run,
                // then the shorter run wins.
                let mut pa_run = pa;
                let mut pb_run = pb;
                unsafe {
                    loop {
                        let da2 = pa_run < enda && byte_utils::is_digit(*pa_run);
                        let db2 = pb_run < endb && byte_utils::is_digit(*pb_run);
                        if da2 && db2 {
                            let va = *pa_run;
                            let vb = *pb_run;
                            if va != vb {
                                return if va < vb { Less } else { Greater };
                            }
                            pa_run = pa_run.add(1);
                            pb_run = pb_run.add(1);
                        } else if da2 {
                            return Greater;
                        } else if db2 {
                            return Less;
                        } else {
                            break;
                        }
                    }
                    // Scan remaining digits on each side.
                    while pa_run < enda && byte_utils::is_digit(*pa_run) {
                        pa_run = pa_run.add(1);
                    }
                    while pb_run < endb && byte_utils::is_digit(*pb_run) {
                        pb_run = pb_run.add(1);
                    }
                }
                let ka = pa_run as usize - pa as usize;
                let kb = pb_run as usize - pb as usize;
                if ka != kb {
                    return ka.cmp(&kb);
                }
                // Equal-length left-aligned runs that matched: continue
                // the main loop to compare post-run characters.
                pa = pa_run;
                pb = pb_run;
                last_eq_digit = true;
                continue;
            }

            // Right-aligned: longer significant run wins; equal length →
            // word-at-a-time compare.
            let ka;
            let kb;
            let pa_run;
            let pb_run;
            unsafe {
                let mut par = pa;
                while par < enda && byte_utils::is_digit(*par) {
                    par = par.add(1);
                }
                let mut pbr = pb;
                while pbr < endb && byte_utils::is_digit(*pbr) {
                    pbr = pbr.add(1);
                }
                ka = par as usize - pa as usize;
                kb = pbr as usize - pb as usize;
                pa_run = par;
                pb_run = pbr;
            }

            if ka != kb {
                return ka.cmp(&kb);
            }

            // Equal-length: u64 XOR + trailing_zeros (like optimized memcmp).
            let end_run = pa_run;
            let mut pa_eq = pa;
            let mut pb_eq = pb;

            unsafe {
                while (pa_eq as usize) + 8 <= (end_run as usize) {
                    let wa = (pa_eq as *const u64).read_unaligned();
                    let wb = (pb_eq as *const u64).read_unaligned();
                    let diff = wa ^ wb;
                    if diff != 0 {
                        let byte_off = (diff.trailing_zeros() / 8) as usize;
                        let ca_eq = *pa_eq.add(byte_off);
                        let cb_eq = *pb_eq.add(byte_off);
                        return if ca_eq < cb_eq { Less } else { Greater };
                    }
                    pa_eq = pa_eq.add(8);
                    pb_eq = pb_eq.add(8);
                }
                // Tail bytes (0–7).
                while pa_eq < end_run {
                    let ca_eq = *pa_eq;
                    let cb_eq = *pb_eq;
                    if ca_eq != cb_eq {
                        return if ca_eq == /* ~ changed by cargo-mutants ~ */ cb_eq { Less } else { Greater };
                    }
                    pa_eq = pa_eq.add(1);
                    pb_eq = pb_eq.add(1);
                }
            }

            pa = pa_run;
            pb = pb_run;
            // The equal-length digit runs were digits — remember for later
            // mixed-type checks.
            last_eq_digit = true;
            continue;
        }

        // At most one side is a digit (or neither).  Digit bytes are always
        // below non-digit/non-ws bytes, so a plain byte compare preserves
        // natural order (numbers before text) — except when the last
        // aligned matching byte was a digit: the digit run may continue on
        // one side but not the other, so the longer run wins.
        if ca != cb {
            if last_eq_digit && byte_utils::is_digit(ca) != byte_utils::is_digit(cb) {
                // Last aligned bytes were a matching digit run; one side's
                // digit run continues while the other's has ended → longer
                // run wins.
                return if byte_utils::is_digit(ca) {
                    Greater
                } else {
                    Less
                };
            }
            return if ca < cb { Less } else { Greater };
        }
        unsafe {
            // Matching non-digit bytes: clear the digit-run flag.
            last_eq_digit = false;
            pa = pa.add(1);
            pb = pb.add(1);
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    const SAFE_LEN: usize = 6;

    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_impl_memory_safe() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a <= SAFE_LEN);
        kani::assume(len_b <= SAFE_LEN);
        let a: [u8; SAFE_LEN] = kani::any();
        let b: [u8; SAFE_LEN] = kani::any();
        let _ = compare_impl(&a[..len_a], &b[..len_b]);
    }

    const MAX_LEN: usize = 4;
    const ALPHABET: [u8; 5] = [b'0', b'1', b'9', b'a', b' '];

    fn any_string(len: usize) -> [u8; MAX_LEN] {
        let mut buf = [0u8; MAX_LEN];
        let mut i = 0;
        while i < MAX_LEN {
            if i < len {
                let idx: usize = kani::any();
                kani::assume(idx < ALPHABET.len());
                buf[i] = ALPHABET[idx];
            }
            i += 1;
        }
        buf
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_impl_reflexive() {
        let len: usize = kani::any();
        kani::assume(len <= MAX_LEN);
        let buf = any_string(len);
        let s = &buf[..len];
        assert_eq!(compare_impl(s, s), Equal);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_impl_antisymmetric() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a <= MAX_LEN && len_b <= MAX_LEN);
        let ba = any_string(len_a);
        let bb = any_string(len_b);
        let a = &ba[..len_a];
        let b = &bb[..len_b];
        assert_eq!(compare_impl(a, b), compare_impl(b, a).reverse());
    }

    const TRANS_LEN: usize = 3;
    const TRANS_ALPHABET: [u8; 3] = [b'0', b'1', b'a'];

    fn any_trans_string(len: usize) -> [u8; TRANS_LEN] {
        let mut buf = [0u8; TRANS_LEN];
        let mut i = 0;
        while i < TRANS_LEN {
            if i < len {
                let idx: usize = kani::any();
                kani::assume(idx < TRANS_ALPHABET.len());
                buf[i] = TRANS_ALPHABET[idx];
            }
            i += 1;
        }
        buf
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_impl_transitive() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        let len_c: usize = kani::any();
        kani::assume(len_a <= TRANS_LEN && len_b <= TRANS_LEN && len_c <= TRANS_LEN);
        let ba = any_trans_string(len_a);
        let bb = any_trans_string(len_b);
        let bc = any_trans_string(len_c);
        let a = &ba[..len_a];
        let b = &bb[..len_b];
        let c = &bc[..len_c];

        let ab = compare_impl(a, b);
        let bc_ord = compare_impl(b, c);
        let ac = compare_impl(a, c);

        // a <= b and b <= c implies a <= c (and strictly, if either
        // input relation is strict).
        if ab != Greater && bc_ord != Greater {
            assert_ne!(ac, Greater);
        }
        if ab != Less && bc_ord != Less {
            assert_ne!(ac, Less);
        }
    }

    const NUM_LEN: usize = 4;

    fn digits_to_value(buf: &[u8]) -> u64 {
        let mut v: u64 = 0;
        let mut i = 0;
        while i < buf.len() {
            v = v * 10 + (buf[i] - b'0') as u64;
            i += 1;
        }
        v
    }

    fn any_digit_run(len: usize, forbid_leading_zero: bool) -> [u8; NUM_LEN] {
        let mut buf = [0u8; NUM_LEN];
        let mut i = 0;
        while i < NUM_LEN {
            if i < len {
                let d: u8 = kani::any();
                kani::assume(d <= 9);
                buf[i] = b'0' + d;
            }
            i += 1;
        }
        if forbid_leading_zero && len > 1 {
            kani::assume(buf[0] != b'0');
        }
        buf
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn compare_impl_right_aligned_matches_integer_value() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a >= 1 && len_a <= NUM_LEN);
        kani::assume(len_b >= 1 && len_b <= NUM_LEN);

        let ba = any_digit_run(len_a, true);
        let bb = any_digit_run(len_b, true);
        let a = &ba[..len_a];
        let b = &bb[..len_b];

        let expected = digits_to_value(a).cmp(&digits_to_value(b));
        assert_eq!(compare_impl(a, b), expected);
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn compare_impl_leading_zero_matches_lexicographic() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a >= 1 && len_a <= NUM_LEN);
        kani::assume(len_b >= 1 && len_b <= NUM_LEN);

        let ba = any_digit_run(len_a, false);
        let bb = any_digit_run(len_b, false);
        kani::assume(ba[0] == b'0');
        kani::assume(bb[0] == b'0');
        let a = &ba[..len_a];
        let b = &bb[..len_b];

        assert_eq!(compare_impl(a, b), a.cmp(b));
    }
}
