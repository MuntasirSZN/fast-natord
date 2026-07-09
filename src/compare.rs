use crate::byte_utils;
use crate::digit;
use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

/// Case-sensitive natural order compare on byte slices.
pub fn compare_impl(a: &[u8], b: &[u8]) -> Ordering {
    if a.len() == b.len() && a.as_ptr() == b.as_ptr() {
        return Equal;
    }

    let len_a = a.len();
    let len_b = b.len();
    let common_len = len_a.min(len_b);
    let mut i = 0usize;
    let mut j = 0usize;

    while i + 8 <= common_len {
        let wa = unsafe { byte_utils::load_u64(a, i) };
        let wb = unsafe { byte_utils::load_u64(b, i) };
        if wa != wb {
            break;
        }
        i += 8;
        j += 8;
    }

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

        if !(da | db) {
            if ca != cb {
                return if ca < cb { Less } else { Greater };
            }
            i += 1;
            j += 1;
            continue;
        }

        if da ^ db {
            if ca != cb {
                return if ca < cb { Less } else { Greater };
            }
            i += 1;
            j += 1;
            continue;
        }

        let (ord, a_rest, b_rest) = digit::compare_digit_runs(&a[i..], &b[j..]);
        if ord != Equal {
            return ord;
        }
        i = len_a - a_rest.len();
        j = len_b - b_rest.len();
    }
}
