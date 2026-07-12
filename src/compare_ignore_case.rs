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
        // Cold path: one or both current bytes are whitespace.
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

            // Equal-length: u64 XOR + trailing_zeros.
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

        // Fast path: identical byte → case-fold is a no-op.
        if ca == cb {
            unsafe {
                pa = pa.add(1);
                pb = pb.add(1);
            }
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

    const SAFE_LEN: usize = 6;

    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_ignore_case_impl_memory_safe() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a <= SAFE_LEN);
        kani::assume(len_b <= SAFE_LEN);
        let a: [u8; SAFE_LEN] = kani::any();
        let b: [u8; SAFE_LEN] = kani::any();
        let _ = compare_ignore_case_impl(&a[..len_a], &b[..len_b]);
    }

    const MAX_LEN: usize = 4;
    const ALPHABET: [u8; 6] = [b'0', b'1', b'9', b'a', b'A', b' '];

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
    fn compare_ignore_case_impl_reflexive() {
        let len: usize = kani::any();
        kani::assume(len <= MAX_LEN);
        let buf = any_string(len);
        let s = &buf[..len];
        assert_eq!(compare_ignore_case_impl(s, s), Equal);
    }

    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_ignore_case_impl_antisymmetric() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a <= MAX_LEN && len_b <= MAX_LEN);
        let ba = any_string(len_a);
        let bb = any_string(len_b);
        let a = &ba[..len_a];
        let b = &bb[..len_b];
        assert_eq!(
            compare_ignore_case_impl(a, b),
            compare_ignore_case_impl(b, a).reverse()
        );
    }

    const TRANS_LEN: usize = 3;
    const TRANS_ALPHABET: [u8; 4] = [b'0', b'1', b'a', b'A'];

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
    fn compare_ignore_case_impl_transitive() {
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

        let ab = compare_ignore_case_impl(a, b);
        let bc_ord = compare_ignore_case_impl(b, c);
        let ac = compare_ignore_case_impl(a, c);

        if ab != Greater && bc_ord != Greater {
            assert_ne!(ac, Greater);
        }
        if ab != Less && bc_ord != Less {
            assert_ne!(ac, Less);
        }
    }

    // ── Cross-check against compare_impl: case-folding an ASCII
    //    string upfront and running the case-*sensitive* comparator
    //    must give the same answer as running the case-*insensitive*
    //    comparator on the originals ──────────────────────────────
    #[kani::proof]
    #[kani::unwind(10)]
    fn compare_ignore_case_matches_compare_on_prefolded_ascii() {
        let len_a: usize = kani::any();
        let len_b: usize = kani::any();
        kani::assume(len_a <= MAX_LEN && len_b <= MAX_LEN);
        let ba = any_string(len_a);
        let bb = any_string(len_b);
        let a = &ba[..len_a];
        let b = &bb[..len_b];

        let mut la = [0u8; MAX_LEN];
        let mut lb = [0u8; MAX_LEN];
        let mut i = 0;
        while i < len_a {
            la[i] = a[i].to_ascii_lowercase();
            i += 1;
        }
        i = 0;
        while i < len_b {
            lb[i] = b[i].to_ascii_lowercase();
            i += 1;
        }

        assert_eq!(
            compare_ignore_case_impl(a, b),
            crate::compare::compare_impl(&la[..len_a], &lb[..len_b])
        );
    }
}
