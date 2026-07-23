//! Word-at-a-time byte comparison.
//!
//! Compares two byte sequences of equal length using XOR + trailing_zeros,
//! processing 16 bytes (`u128`), then 8 bytes (`u64`), then byte-by-byte.

/// Compare two byte sequences of equal length using word-at-a-time XOR.
///
/// Processes 16 bytes (u128), then 8 bytes (u64), then byte-by-byte.
/// Returns `Some(Ordering)` at the first differing byte, or `None`
/// if all `len` bytes are equal.
///
/// # Safety
///
/// `pa` and `pb` must point to valid memory for at least `len` bytes.
/// `len == 0` returns `None` immediately.
#[inline(always)]
pub unsafe fn compare_word_at_a_time(
    pa: *const u8,
    pb: *const u8,
    len: usize,
) -> Option<core::cmp::Ordering> {
    unsafe {
        let end = pa.add(len);
        let mut pa_eq = pa;
        let mut pb_eq = pb;

        while (pa_eq as usize) + 16 <= (end as usize) {
            let wa = (pa_eq as *const u128).read_unaligned();
            let wb = (pb_eq as *const u128).read_unaligned();
            let diff = wa ^ wb;
            if diff != 0 {
                let byte_off = (diff.trailing_zeros() / 8) as usize;
                let ca_eq = *pa_eq.add(byte_off);
                let cb_eq = *pb_eq.add(byte_off);
                return Some(if ca_eq < cb_eq {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Greater
                });
            }
            pa_eq = pa_eq.add(16);
            pb_eq = pb_eq.add(16);
        }
        while (pa_eq as usize) + 8 <= (end as usize) {
            let wa = (pa_eq as *const u64).read_unaligned();
            let wb = (pb_eq as *const u64).read_unaligned();
            let diff = wa ^ wb;
            if diff != 0 {
                let byte_off = (diff.trailing_zeros() / 8) as usize;
                let ca_eq = *pa_eq.add(byte_off);
                let cb_eq = *pb_eq.add(byte_off);
                return Some(if ca_eq < cb_eq {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Greater
                });
            }
            pa_eq = pa_eq.add(8);
            pb_eq = pb_eq.add(8);
        }
        while pa_eq < end {
            let ca_eq = *pa_eq;
            let cb_eq = *pb_eq;
            if ca_eq != cb_eq {
                return Some(if ca_eq < cb_eq {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Greater
                });
            }
            pa_eq = pa_eq.add(1);
            pb_eq = pb_eq.add(1);
        }
        None
    }
}
