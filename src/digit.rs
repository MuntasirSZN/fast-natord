use crate::byte_utils;
use core::cmp::Ordering;
use core::cmp::Ordering::{Greater, Less};

#[inline(always)]
pub fn compare_digit_runs<'a>(a: &'a [u8], b: &'a [u8]) -> (Ordering, &'a [u8], &'a [u8]) {
    let mut na = 0usize;
    while na < a.len() && byte_utils::is_digit(unsafe { *a.get_unchecked(na) }) {
        na += 1;
    }
    let mut nb = 0usize;
    while nb < b.len() && byte_utils::is_digit(unsafe { *b.get_unchecked(nb) }) {
        nb += 1;
    }

    let has_leading = (na > 0 && unsafe { *a.get_unchecked(0) } == b'0')
        || (nb > 0 && unsafe { *b.get_unchecked(0) } == b'0');

    if has_leading {
        let limit = na.min(nb);
        for k in 0..limit {
            let ca = unsafe { *a.get_unchecked(k) };
            let cb = unsafe { *b.get_unchecked(k) };
            if ca != cb {
                return (if ca < cb { Less } else { Greater }, &a[na..], &b[nb..]);
            }
        }
        return (na.cmp(&nb), &a[na..], &b[nb..]);
    }

    let mut za = 0;
    while za < na && unsafe { *a.get_unchecked(za) } == b'0' {
        za += 1;
    }
    let mut zb = 0;
    while zb < nb && unsafe { *b.get_unchecked(zb) } == b'0' {
        zb += 1;
    }

    let sig_a = na - za;
    let sig_b = nb - zb;

    let ord = if sig_a != sig_b {
        sig_a.cmp(&sig_b)
    } else {
        byte_utils::compare_same_len(&a[za..za + sig_a], &b[zb..zb + sig_b])
    };

    (ord, &a[na..], &b[nb..])
}
