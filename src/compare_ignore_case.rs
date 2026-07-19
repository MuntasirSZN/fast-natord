use crate::byte_utils;
use crate::unicode;
use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

/// Case-insensitive natural order comparison on byte slices.
///
/// SIMD common-prefix skip (byte-level equality is safe because case
/// folding happens in the per-byte tail), then a pointer-based scalar
/// loop with numeric-run awareness.
#[inline(always)]
pub fn compare_ignore_case_impl(a: &[u8], b: &[u8]) -> Ordering {
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

        // Both sides are digits: compare the two runs.
        if byte_utils::is_digit(ca) && byte_utils::is_digit(cb) {
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
            continue;
        }

        // Handle non-digits: check whitespace first, then case-fold.
        if ca == cb {
            // Matching bytes — check whitespace (cold path).
            if byte_utils::is_ascii_ws(ca) {
                unsafe { byte_utils::skip_whitespace(&mut pa, &mut pb, enda, endb) };
                continue;
            }
            unsafe {
                pa = pa.add(1);
                pb = pb.add(1);
            }
        } else if byte_utils::is_ascii_ws(ca) || byte_utils::is_ascii_ws(cb) {
            // Differing due to whitespace on at least one side.
            unsafe { byte_utils::skip_whitespace(&mut pa, &mut pb, enda, endb) };
            continue;
        } else if byte_utils::is_digit(ca) != byte_utils::is_digit(cb)
            && pa > a.as_ptr()
            && unsafe { byte_utils::is_digit(*pa.sub(1)) }
        {
            // Digit-run continuation: the longer run wins.
            return if byte_utils::is_digit(ca) {
                Greater
            } else {
                Less
            };
        } else if ca < 128 && cb < 128 {
            // Both ASCII — lowercasing is cheap.
            let lca = ca.to_ascii_lowercase();
            let lcb = cb.to_ascii_lowercase();
            if lca != lcb {
                return if lca < lcb { Less } else { Greater };
            }
            unsafe {
                pa = pa.add(1);
                pb = pb.add(1);
            }
        } else if ca >= 128 && cb >= 128 {
            // Both non-ASCII — decode code points and case-fold.
            unsafe {
                let rest_a = core::slice::from_raw_parts(pa, enda as usize - pa as usize);
                let rest_b = core::slice::from_raw_parts(pb, endb as usize - pb as usize);
                let (ch_a, adv_a) = unicode::decode_char(rest_a);
                let (ch_b, adv_b) = unicode::decode_char(rest_b);
                if ch_a != ch_b {
                    let cmp = ch_a.to_lowercase().cmp(ch_b.to_lowercase());
                    if cmp != Equal {
                        return cmp;
                    }
                }
                pa = pa.add(adv_a);
                pb = pb.add(adv_b);
            }
        } else {
            // One ASCII, one non-ASCII — byte order decides.
            return if ca < cb { Less } else { Greater };
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(3)]
    fn compare_ignore_case_impl_memory_safe() {
        let a0: u8 = kani::any();
        let b0: u8 = kani::any();
        kani::assume(a0 < 128);
        kani::assume(b0 < 128);
        let a = [a0];
        let b = [b0];
        let _ = compare_ignore_case_impl(&a, &b);
    }
}
