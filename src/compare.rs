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
                    let start_a = (pa_run as usize) - (a.as_ptr() as usize);
                    let start_b = (pb_run as usize) - (b.as_ptr() as usize);
                    pa_run = a
                        .as_ptr()
                        .add(byte_utils::simd_skip_while_digit(a, start_a));
                    pb_run = b
                        .as_ptr()
                        .add(byte_utils::simd_skip_while_digit(b, start_b));
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
                let start_a = (pa as usize) - (a.as_ptr() as usize);
                let start_b = (pb as usize) - (b.as_ptr() as usize);
                let end_a = byte_utils::simd_skip_while_digit(a, start_a);
                let end_b = byte_utils::simd_skip_while_digit(b, start_b);
                ka = end_a - start_a;
                kb = end_b - start_b;
                pa_run = a.as_ptr().add(end_a);
                pb_run = b.as_ptr().add(end_b);
            }

            if ka != kb {
                return ka.cmp(&kb);
            }

            // Equal-length: u128 then u64 XOR + trailing_zeros.
            let end_run = pa_run;
            let mut pa_eq = pa;
            let mut pb_eq = pb;

            unsafe {
                while (pa_eq as usize) + 16 <= (end_run as usize) {
                    let wa = (pa_eq as *const u128).read_unaligned();
                    let wb = (pb_eq as *const u128).read_unaligned();
                    let diff = wa ^ wb;
                    if diff != 0 {
                        let byte_off = (diff.trailing_zeros() / 8) as usize;
                        let ca_eq = *pa_eq.add(byte_off);
                        let cb_eq = *pb_eq.add(byte_off);
                        return if ca_eq < cb_eq { Less } else { Greater };
                    }
                    pa_eq = pa_eq.add(16);
                    pb_eq = pb_eq.add(16);
                }
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
                        return if ca_eq < cb_eq { Less } else { Greater };
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

    #[kani::proof]
    #[kani::unwind(4)]
    fn compare_impl_memory_safe() {
        let a0: u8 = kani::any();
        let b0: u8 = kani::any();
        let a = [a0];
        let b = [b0];
        // Exercise pointer arithmetic and bounds checks for safety.
        let _ = compare_impl(&a, &b);
    }

    const NUM_LEN: usize = 2;

    fn digits_to_value(buf: &[u8]) -> u64 {
        let mut v: u64 = 0;
        if buf.len() > 0 {
            v = (buf[0] - b'0') as u64;
        }
        if buf.len() > 1 {
            v = v * 10 + (buf[1] - b'0') as u64;
        }
        v
    }

    fn any_digit_run(len: usize, forbid_leading_zero: bool) -> [u8; NUM_LEN] {
        let mut buf = [0u8; NUM_LEN];
        if 0 < len {
            let d: u8 = kani::any();
            kani::assume(d <= 9);
            buf[0] = b'0' + d;
        }
        if 1 < len {
            let d: u8 = kani::any();
            kani::assume(d <= 9);
            buf[1] = b'0' + d;
        }
        if forbid_leading_zero && len > 1 {
            kani::assume(buf[0] != b'0');
        }
        buf
    }

    #[kani::proof]
    #[kani::unwind(3)]
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
    #[kani::unwind(3)]
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
