//! Case-sensitive natural ordering comparison.

pub(crate) mod common;

use core::cmp::Ordering;
use core::cmp::Ordering::{Greater, Less};

use crate::byte_utils;
use crate::compare::common::CompareCtx;

#[cfg(kani)]
mod kani;

/// Case-sensitive natural order compare on byte slices.
///
/// Uses SIMD to skip common prefix, then a pointer-based scalar
/// loop with numeric-run awareness.  Equal-length digit runs use
/// word-at-a-time comparison (XOR + trailing_zeros).
///
/// The setup and common loop patterns are factored through
/// [`CompareCtx`] — shared with the case-insensitive backend.
#[inline(always)]
pub fn compare_impl(a: &[u8], b: &[u8]) -> Ordering {
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

        // At most one side is a digit (or neither).
        if ca != cb {
            if byte_utils::is_ascii_ws(ca) || byte_utils::is_ascii_ws(cb) {
                unsafe { ctx.skip_ws() };
                continue;
            }
            if let Some(ord) = unsafe { ctx.check_digit_boundary(a, ca, cb) } {
                return ord;
            }
            return if ca < cb { Less } else { Greater };
        }

        if byte_utils::is_ascii_ws(ca) {
            unsafe { ctx.skip_ws() };
            continue;
        }

        unsafe { ctx.advance() };
    }
}
