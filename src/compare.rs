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

    // SAFETY: adv ≤ common_len ≤ both lengths.
    let mut pa = unsafe { a.as_ptr().add(adv) };
    let mut pb = unsafe { b.as_ptr().add(adv) };
    let enda = unsafe { a.as_ptr().add(len_a) };
    let endb = unsafe { b.as_ptr().add(len_b) };

    // Harden digit-run boundary: if `simd_skip_equal` landed in the
    // middle of a digit run (both previous bytes are digits AND both
    // next bytes are digits, meaning the run continues past the boundary),
    // rewind so the digit-aware loop gets full context for leading-zero and
    // right-aligned handling.
    if adv > 0 && adv < common_len {
        unsafe {
            let ca_after = *a.as_ptr().add(adv);
            let cb_after = *b.as_ptr().add(adv);
            if byte_utils::is_digit(ca_after)
                && byte_utils::is_digit(cb_after)
                && byte_utils::is_digit(*a.as_ptr().add(adv - 1))
                && byte_utils::is_digit(*b.as_ptr().add(adv - 1))
            {
                pa = a.as_ptr().add(adv - 1);
                pb = b.as_ptr().add(adv - 1);
            }
        }
    }

    loop {
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
            // True leading-zero: current byte is '0' AND it's the first
            // byte of the digit run (not a middle digit after rewind).
            let la0 =
                ca == b'0' && (pa == a.as_ptr() || unsafe { !byte_utils::is_digit(*pa.sub(1)) });
            let lb0 =
                cb == b'0' && (pb == b.as_ptr() || unsafe { !byte_utils::is_digit(*pb.sub(1)) });

            if la0 || lb0 {
                // Left-aligned (leading-zero): shorter run can win.
                unsafe {
                    // Short remaining strings: single-pass byte-by-byte
                    // avoids the overhead of two simd_skip_while_digit calls
                    // plus a second pass for word-at-a-time comparison.
                    if (enda as usize).wrapping_sub(pa as usize) < 16
                        && (endb as usize).wrapping_sub(pb as usize) < 16
                    {
                        let mut pa_run = pa;
                        let mut pb_run = pb;
                        loop {
                            let da = pa_run < enda && byte_utils::is_digit(*pa_run);
                            let db = pb_run < endb && byte_utils::is_digit(*pb_run);
                            if da && db {
                                let va = *pa_run;
                                let vb = *pb_run;
                                if va != vb {
                                    return if va < vb { Less } else { Greater };
                                }
                                pa_run = pa_run.add(1);
                                pb_run = pb_run.add(1);
                            } else if da {
                                return Greater;
                            } else if db {
                                return Less;
                            } else {
                                break;
                            }
                        }
                        pa = pa_run;
                        pb = pb_run;
                        continue;
                    }

                    // Long runs: combined two-string digit scan.
                    let start_a = (pa as usize) - (a.as_ptr() as usize);
                    let start_b = (pb as usize) - (b.as_ptr() as usize);
                    let (end_a, end_b) =
                        byte_utils::simd_skip_while_digit_both(a, b, start_a, start_b);
                    let ka = end_a - start_a;
                    let kb = end_b - start_b;
                    let min_len = if ka < kb { ka } else { kb };

                    if let Some(ord) = byte_utils::compare_word_at_a_time(pa, pb, min_len) {
                        return ord;
                    }

                    if ka != kb {
                        return ka.cmp(&kb);
                    }
                    pa = a.as_ptr().add(end_a);
                    pb = b.as_ptr().add(end_b);
                }
                continue;
            }

            // Right-aligned: longer significant run wins; equal length →
            // word-at-a-time compare.
            let ka;
            let kb;
            let pa_after;
            let pb_after;
            unsafe {
                if (enda as usize).wrapping_sub(pa as usize) < 16
                    && (endb as usize).wrapping_sub(pb as usize) < 16
                {
                    // Short-path: byte-by-byte with early-return when
                    // one side has a longer digit run (most common case).
                    let mut pa_run = pa;
                    let mut pb_run = pb;
                    loop {
                        let da = pa_run < enda && byte_utils::is_digit(*pa_run);
                        let db = pb_run < endb && byte_utils::is_digit(*pb_run);
                        if da && db {
                            pa_run = pa_run.add(1);
                            pb_run = pb_run.add(1);
                        } else if da {
                            return Greater;
                        } else if db {
                            return Less;
                        } else {
                            break;
                        }
                    }
                    ka = pa_run as usize - pa as usize;
                    kb = pb_run as usize - pb as usize;
                    pa_after = pa_run;
                    pb_after = pb_run;
                } else {
                    // Long runs: combined SIMD digit scan.
                    let start_a = (pa as usize) - (a.as_ptr() as usize);
                    let start_b = (pb as usize) - (b.as_ptr() as usize);
                    let (end_a, end_b) =
                        byte_utils::simd_skip_while_digit_both(a, b, start_a, start_b);
                    ka = end_a - start_a;
                    kb = end_b - start_b;
                    pa_after = a.as_ptr().add(end_a);
                    pb_after = b.as_ptr().add(end_b);
                }
            }

            if ka != kb {
                return ka.cmp(&kb);
            }

            // Equal-length: word-at-a-time compare.
            unsafe {
                if let Some(ord) = byte_utils::compare_word_at_a_time(pa, pb, ka) {
                    return ord;
                }
            }

            pa = pa_after;
            pb = pb_after;
            continue;
        }

        // At most one side is a digit (or neither).  Digit bytes are always
        // below non-digit bytes, so a plain byte compare preserves natural
        // order (numbers before text) — except when the last aligned
        // matching byte was a digit: the digit run may continue on one side
        // but not the other, so the longer run wins.
        if ca != cb {
            // Whitespace takes precedence — skip it before comparing.
            if byte_utils::is_ascii_ws(ca) || byte_utils::is_ascii_ws(cb) {
                unsafe { byte_utils::skip_whitespace(&mut pa, &mut pb, enda, endb) };
                continue;
            }
            // When one side is a digit and the other isn't, and the byte
            // immediately before this position was also a digit, the digit
            // run continues on one side — the longer run wins.
            if byte_utils::is_digit(ca) != byte_utils::is_digit(cb)
                && pa > a.as_ptr()
                && unsafe { byte_utils::is_digit(*pa.sub(1)) }
            {
                return if byte_utils::is_digit(ca) {
                    Greater
                } else {
                    Less
                };
            }
            return if ca < cb { Less } else { Greater };
        }

        // Cold: whitespace skip (rare in filenames).
        if unsafe { byte_utils::is_ascii_ws(*pa) } {
            unsafe { byte_utils::skip_whitespace(&mut pa, &mut pb, enda, endb) };
            continue;
        }

        unsafe {
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
