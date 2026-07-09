use crate::byte_utils;
use crate::digit;
use crate::unicode;
use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

/// Case-insensitive natural order comparison on byte slices.
pub fn compare_ignore_case_impl(a: &[u8], b: &[u8]) -> Ordering {
    if a.len() == b.len() && a.as_ptr() == b.as_ptr() {
        return Equal;
    }

    let len_a = a.len();
    let len_b = b.len();
    let mut i = 0usize;
    let mut j = 0usize;

    loop {
        while i < len_a && byte_utils::is_ascii_ws(unsafe { *a.get_unchecked(i) }) {
            i += 1;
        }
        while j < len_b && byte_utils::is_ascii_ws(unsafe { *b.get_unchecked(j) }) {
            j += 1;
        }

        let rem_a = len_a - i;
        let rem_b = len_b - j;

        if rem_a == 0 || rem_b == 0 {
            return rem_a.cmp(&rem_b);
        }

        let ca = unsafe { *a.get_unchecked(i) };
        let cb = unsafe { *b.get_unchecked(j) };

        let da = byte_utils::is_digit(ca);
        let db = byte_utils::is_digit(cb);

        if da && db {
            let (ord, a_rest, b_rest) = digit::compare_digit_runs(&a[i..], &b[j..]);
            if ord != Equal {
                return ord;
            }
            i = len_a - a_rest.len();
            j = len_b - b_rest.len();
            continue;
        }

        if ca < 128 && cb < 128 {
            let lca = ca.to_ascii_lowercase();
            let lcb = cb.to_ascii_lowercase();
            if lca != lcb {
                return if lca < lcb { Less } else { Greater };
            }
            i += 1;
            j += 1;
        } else if ca >= 128 && cb >= 128 {
            let (ch_a, adv_a) = unicode::decode_char(&a[i..]);
            let (ch_b, adv_b) = unicode::decode_char(&b[j..]);
            let cmp = ch_a.to_lowercase().cmp(ch_b.to_lowercase());
            if cmp != Equal {
                return cmp;
            }
            i += adv_a;
            j += adv_b;
        } else {
            return if ca < cb { Less } else { Greater };
        }
    }
}
