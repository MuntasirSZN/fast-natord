//! Digit-run handling extracted from the hot comparison loop.
//!
//! Extracting this ~70-line block into a single shared function
//! eliminates duplicate compilation in both backends.Inline
//! hints are omitted — LLVM decides based on LTO context.

use core::cmp::Ordering::{self, Greater, Less};

use super::basic::is_digit;
use super::compare_word_at_a_time::compare_word_at_a_time;
use super::skip_while_digit::simd_skip_while_digit_both;

/// Handle the case where both current bytes are digits.
///
/// The caller must supply `has_leading_zero` — computed as:
/// ```ignore
/// (ca == b'0' && (pa == a.as_ptr() || !is_digit(*pa.sub(1))))
/// || (cb == b'0' && (pb == b.as_ptr() || !is_digit(*pb.sub(1))))
/// ```
///
/// Returns `(Some(Ordering), _, _)` if the comparison is settled, or
/// `(None, pa_after, pb_after)` with updated pointers past the digit
/// run when the outer loop should continue.
///
/// # Safety
///
/// `pa`, `pb` must point into `a`, `b`; `enda`, `endb` must be the
/// end pointers.
pub(crate) unsafe fn handle_digit_case(
    a: &[u8],
    b: &[u8],
    pa: *const u8,
    pb: *const u8,
    enda: *const u8,
    endb: *const u8,
    has_leading_zero: bool,
) -> (Option<Ordering>, *const u8, *const u8) {
    unsafe {
        if has_leading_zero {
            // Left-aligned: shorter run can win via per-digit compare.
            handle_left_aligned(a, b, pa, pb, enda, endb)
        } else {
            // Right-aligned: longer significant run wins.
            handle_right_aligned(a, b, pa, pb, enda, endb)
        }
    }
}

unsafe fn handle_left_aligned(
    a: &[u8],
    b: &[u8],
    pa: *const u8,
    pb: *const u8,
    enda: *const u8,
    endb: *const u8,
) -> (Option<Ordering>, *const u8, *const u8) {
    let rem_a = (enda as usize).wrapping_sub(pa as usize);
    let rem_b = (endb as usize).wrapping_sub(pb as usize);
    if rem_a < 16 && rem_b < 16 {
        // Short-run: byte-by-byte with value compare.
        let mut pa_run = pa;
        let mut pb_run = pb;
        loop {
            let da = pa_run < enda && unsafe { is_digit(*pa_run) };
            let db = pb_run < endb && unsafe { is_digit(*pb_run) };
            if da && db {
                let va = unsafe { *pa_run };
                let vb = unsafe { *pb_run };
                if va != vb {
                    return (Some(if va < vb { Less } else { Greater }), pa_run, pb_run);
                }
                pa_run = unsafe { pa_run.add(1) };
                pb_run = unsafe { pb_run.add(1) };
            } else if da {
                return (Some(Greater), pa_run, pb_run);
            } else if db {
                return (Some(Less), pa_run, pb_run);
            } else {
                break;
            }
        }
        return (None, pa_run, pb_run);
    }

    // Long-run: SIMD scan then compare on the shorter run.
    let start_a = (pa as usize) - (a.as_ptr() as usize);
    let start_b = (pb as usize) - (b.as_ptr() as usize);
    let (end_a, end_b) = unsafe { simd_skip_while_digit_both(a, b, start_a, start_b) };
    let ka = end_a - start_a;
    let kb = end_b - start_b;
    let min_len = if ka < kb { ka } else { kb };

    if let Some(ord) = unsafe { compare_word_at_a_time(pa, pb, min_len) } {
        return (Some(ord), pa, pb);
    }

    if ka != kb {
        return (Some(ka.cmp(&kb)), pa, pb);
    }
    (None, unsafe { a.as_ptr().add(end_a) }, unsafe {
        b.as_ptr().add(end_b)
    })
}

fn handle_right_aligned(
    a: &[u8],
    b: &[u8],
    pa: *const u8,
    pb: *const u8,
    enda: *const u8,
    endb: *const u8,
) -> (Option<Ordering>, *const u8, *const u8) {
    let rem_a = (enda as usize).wrapping_sub(pa as usize);
    let rem_b = (endb as usize).wrapping_sub(pb as usize);
    if rem_a < 16 && rem_b < 16 {
        // Short-path: count digits, return on length mismatch.
        let mut pa_run = pa;
        let mut pb_run = pb;
        loop {
            let da = pa_run < enda && is_digit(unsafe { *pa_run });
            let db = pb_run < endb && is_digit(unsafe { *pb_run });
            if da && db {
                pa_run = unsafe { pa_run.add(1) };
                pb_run = unsafe { pb_run.add(1) };
            } else if da {
                return (Some(Greater), pa_run, pb_run);
            } else if db {
                return (Some(Less), pa_run, pb_run);
            } else {
                break;
            }
        }
        let ka = pa_run as usize - pa as usize;
        let kb = pb_run as usize - pb as usize;
        if ka != kb {
            return (Some(ka.cmp(&kb)), pa_run, pb_run);
        }
        // Equal length: word-at-a-time compare on the original positions.
        if let Some(ord) = unsafe { compare_word_at_a_time(pa, pb, ka) } {
            return (Some(ord), pa_run, pb_run);
        }
        return (None, pa_run, pb_run);
    }

    // Long-run: SIMD scan.
    let start_a = (pa as usize) - (a.as_ptr() as usize);
    let start_b = (pb as usize) - (b.as_ptr() as usize);
    let (end_a, end_b) = unsafe { simd_skip_while_digit_both(a, b, start_a, start_b) };
    let ka = end_a - start_a;
    let kb = end_b - start_b;
    let pa_after = unsafe { a.as_ptr().add(end_a) };
    let pb_after = unsafe { b.as_ptr().add(end_b) };

    if ka != kb {
        return (Some(ka.cmp(&kb)), pa_after, pb_after);
    }

    // Equal length: word-at-a-time.
    if let Some(ord) = unsafe { compare_word_at_a_time(pa, pb, ka) } {
        return (Some(ord), pa_after, pb_after);
    }

    (None, pa_after, pb_after)
}
