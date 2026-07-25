//! Case-insensitive natural ordering comparison.

use crate::byte_utils;
use crate::unicode;
use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

#[cfg(kani)]
mod kani;

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

    // Harden digit-run boundary.
    if adv > 0 && adv < common_len {
        unsafe {
            if byte_utils::is_digit(*a.as_ptr().add(adv))
                & byte_utils::is_digit(*b.as_ptr().add(adv))
                & byte_utils::is_digit(*a.as_ptr().add(adv - 1))
                & byte_utils::is_digit(*b.as_ptr().add(adv - 1))
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
            let la0 =
                ca == b'0' && (pa == a.as_ptr() || unsafe { !byte_utils::is_digit(*pa.sub(1)) });
            let lb0 =
                cb == b'0' && (pb == b.as_ptr() || unsafe { !byte_utils::is_digit(*pb.sub(1)) });
            let (result, new_pa, new_pb) =
                unsafe { byte_utils::handle_digit_case(a, b, pa, pb, enda, endb, la0 | lb0) };
            pa = new_pa;
            pb = new_pb;
            if let Some(ord) = result {
                return ord;
            }
            continue;
        }

        // Handle non-digits: check whitespace first, then case-fold.
        if ca == cb {
            if byte_utils::is_ascii_ws(ca) {
                unsafe { byte_utils::skip_whitespace(&mut pa, &mut pb, enda, endb) };
                continue;
            }
            unsafe {
                pa = pa.add(1);
                pb = pb.add(1);
            }
        } else if byte_utils::is_ascii_ws(ca) || byte_utils::is_ascii_ws(cb) {
            unsafe { byte_utils::skip_whitespace(&mut pa, &mut pb, enda, endb) };
            continue;
        } else if byte_utils::is_digit(ca) != byte_utils::is_digit(cb)
            && pa > a.as_ptr()
            && unsafe { byte_utils::is_digit(*pa.sub(1)) }
        {
            return if byte_utils::is_digit(ca) {
                Greater
            } else {
                Less
            };
        } else if ca < 128 && cb < 128 {
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
            // Both non-ASCII — decode and case-fold.
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
            return if ca < cb { Less } else { Greater };
        }
    }
}
