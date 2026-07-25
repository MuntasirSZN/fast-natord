//! Case-insensitive natural ordering comparison.

use core::cmp::Ordering;
use core::cmp::Ordering::{Equal, Greater, Less};

use crate::byte_utils;
use crate::compare::common::CompareCtx;
use crate::unicode;

#[cfg(kani)]
mod kani;

/// Case-insensitive natural order comparison on byte slices.
///
/// SIMD common-prefix skip (byte-level equality is safe because case
/// folding happens in the per-byte tail), then a pointer-based scalar
/// loop with numeric-run awareness.
///
/// The setup and common loop patterns are factored through
/// [`CompareCtx`] — shared with the case-sensitive backend.
#[inline(always)]
pub fn compare_ignore_case_impl(a: &[u8], b: &[u8]) -> Ordering {
    let mut ctx = match unsafe { CompareCtx::new(a, b) } {
        Ok(ctx) => ctx,
        Err(ord) => return ord,
    };

    loop {
        if let Some(ord) = unsafe { ctx.check_end() } {
            return ord;
        }

        let (ca, cb) = unsafe { ctx.current() };

        if byte_utils::is_digit(ca) && byte_utils::is_digit(cb) {
            if let Some(ord) = unsafe { ctx.handle_both_digits(a, b) } {
                return ord;
            }
            continue;
        }

        // Handle non-digits: check whitespace first, then case-fold.
        if ca == cb {
            if byte_utils::is_ascii_ws(ca) {
                unsafe { ctx.skip_ws() };
                continue;
            }
            unsafe { ctx.advance() };
        } else if byte_utils::is_ascii_ws(ca) || byte_utils::is_ascii_ws(cb) {
            unsafe { ctx.skip_ws() };
            continue;
        } else if let Some(ord) = unsafe { ctx.check_digit_boundary(a, ca, cb) } {
            return ord;
        } else if ca < 128 && cb < 128 {
            let lca = ca.to_ascii_lowercase();
            let lcb = cb.to_ascii_lowercase();
            if lca != lcb {
                return if lca < lcb { Less } else { Greater };
            }
            unsafe { ctx.advance() };
        } else if ca >= 128 && cb >= 128 {
            // Both non-ASCII — decode and case-fold.
            unsafe {
                let rest_a =
                    core::slice::from_raw_parts(ctx.pa, ctx.enda as usize - ctx.pa as usize);
                let rest_b =
                    core::slice::from_raw_parts(ctx.pb, ctx.endb as usize - ctx.pb as usize);
                let (ch_a, adv_a) = unicode::decode_char(rest_a);
                let (ch_b, adv_b) = unicode::decode_char(rest_b);
                if ch_a != ch_b {
                    let cmp = ch_a.to_lowercase().cmp(ch_b.to_lowercase());
                    if cmp != Equal {
                        return cmp;
                    }
                }
                ctx.pa = ctx.pa.add(adv_a);
                ctx.pb = ctx.pb.add(adv_b);
            }
        } else {
            return if ca < cb { Less } else { Greater };
        }
    }
}
