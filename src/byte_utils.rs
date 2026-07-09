use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

#[inline(always)]
pub unsafe fn load_u64(s: &[u8], i: usize) -> u64 {
    unsafe { (s.as_ptr().add(i) as *const u64).read_unaligned() }
}

#[inline(always)]
pub fn is_digit(c: u8) -> bool {
    c.wrapping_sub(b'0') <= 9
}

#[inline(always)]
pub fn is_ascii_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\x0C' | b'\r')
}

#[inline(always)]
pub fn compare_same_len(a: &[u8], b: &[u8]) -> Ordering {
    debug_assert!(a.len() == b.len());
    let len = a.len();
    let mut i = 0;

    while i + 8 <= len {
        let wa = unsafe { load_u64(a, i) };
        let wb = unsafe { load_u64(b, i) };
        if wa != wb {
            break;
        }
        i += 8;
    }

    while i < len {
        let ca = unsafe { *a.get_unchecked(i) };
        let cb = unsafe { *b.get_unchecked(i) };
        if ca != cb {
            return if ca < cb { Less } else { Greater };
        }
        i += 1;
    }

    Equal
}
