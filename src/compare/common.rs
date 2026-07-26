//! Shared comparison context for both backends.
//!
//! Factors out the common setup and loop idioms (SIMD prefix skip,
//! digit-run boundary hardening, end-of-buffer check, both-digit
//! handling, whitespace skip, digit-boundary check) so that the
//! [`compare_impl`](super::compare_impl) and
//! [`compare_ignore_case_impl`](super::compare_ignore_case_impl)
//! functions only differ in per-character comparison logic — no
//! code duplication.

use core::cmp::Ordering;
use core::cmp::Ordering::Equal;

use crate::byte_utils;

/// Inline helpers shared by the case-sensitive and case-insensitive
/// comparison hot loops.
///
/// Methods use `#[inline(always)]` so the compiler sees through them
/// as direct pointer arithmetic — zero abstraction overhead.
pub(crate) struct CompareCtx {
    pub pa: *const u8,
    pub pb: *const u8,
    pub enda: *const u8,
    pub endb: *const u8,
}

impl CompareCtx {
    /// Initialise pointers from two byte slices.
    ///
    /// Returns `Err(Ordering)` if a quick decision was made on the
    /// same-pointer optimisation — the caller should return that
    /// ordering immediately.
    #[inline(always)]
    pub unsafe fn new(a: &[u8], b: &[u8]) -> Result<Self, Ordering> {
        if a.len() == b.len() && a.as_ptr() == b.as_ptr() {
            return Err(Equal);
        }

        let len_a = a.len();
        let len_b = b.len();
        let common_len = len_a.min(len_b);
        let adv = unsafe { byte_utils::simd_skip_equal(a, b, 0, common_len) };

        let mut pa: *const u8 = unsafe { a.as_ptr().add(adv) };
        let mut pb: *const u8 = unsafe { b.as_ptr().add(adv) };
        let enda: *const u8 = unsafe { a.as_ptr().add(len_a) };
        let endb: *const u8 = unsafe { b.as_ptr().add(len_b) };

        // Harden digit-run boundary: if `simd_skip_equal` landed in the
        // middle of a digit run (both current and previous bytes are digits
        // on both sides), rewind so the digit-aware loop gets full context
        // for leading-zero and right-aligned handling.
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

        Ok(Self { pa, pb, enda, endb })
    }

    /// Check if either pointer has reached its string end.
    ///
    /// Returns `Some(Ordering)` when one string is exhausted — ordering is
    /// determined by remaining length.
    /// Combined bounds-check and load.
    ///
    /// Returns `Ok((ca, cb))` if both pointers are in-bounds, or
    /// `Err(Ordering)` derived from remaining length when one is
    /// exhausted.  Replaces the previous `check_end` + `current` pair
    /// for fewer total instructions in the hot loop.
    #[inline(always)]
    pub unsafe fn try_current(&self) -> Result<(u8, u8), Ordering> {
        if self.pa >= self.enda || self.pb >= self.endb {
            let rem_a = (self.enda as usize).wrapping_sub(self.pa as usize);
            let rem_b = (self.endb as usize).wrapping_sub(self.pb as usize);
            return Err(rem_a.cmp(&rem_b));
        }
        unsafe { Ok((*self.pa, *self.pb)) }
    }

    /// Handle the case where both current bytes are digits.
    ///
    /// Computes the leading-zero predicate, delegates to
    /// [`byte_utils::handle_digit_case`], and updates internal pointers.
    /// Returns `Some(Ordering)` if the comparison is settled, or `None`
    /// if the caller should continue the outer loop.
    #[inline(always)]
    pub unsafe fn handle_both_digits(&mut self, a: &[u8], b: &[u8]) -> Option<Ordering> {
        // Leading-zero check uses the current bytes before advancing.
        let ca = unsafe { *self.pa };
        let cb = unsafe { *self.pb };
        let la0 = ca == b'0'
            && (self.pa == a.as_ptr() || unsafe { !byte_utils::is_digit(*self.pa.sub(1)) });
        let lb0 = cb == b'0'
            && (self.pb == b.as_ptr() || unsafe { !byte_utils::is_digit(*self.pb.sub(1)) });

        let (result, new_pa, new_pb) = unsafe {
            byte_utils::handle_digit_case(a, b, self.pa, self.pb, self.enda, self.endb, la0 | lb0)
        };
        self.pa = new_pa;
        self.pb = new_pb;
        result
    }

    /// Check for digit-vs-non-digit boundary when the preceding byte is a digit.
    ///
    /// In this situation the side that is still a digit wins (longer run).
    /// Returns `Some(Ordering)` if the rule applies, or `None` to fall through.
    #[inline(always)]
    pub unsafe fn check_digit_boundary(&self, a: &[u8], ca: u8, cb: u8) -> Option<Ordering> {
        if byte_utils::is_digit(ca) != byte_utils::is_digit(cb)
            && self.pa > a.as_ptr()
            && unsafe { byte_utils::is_digit(*self.pa.sub(1)) }
        {
            return Some(if byte_utils::is_digit(ca) {
                Ordering::Greater
            } else {
                Ordering::Less
            });
        }
        None
    }

    /// Skip ASCII whitespace on both sides.
    #[inline(always)]
    pub unsafe fn skip_ws(&mut self) {
        unsafe { byte_utils::skip_whitespace(&mut self.pa, &mut self.pb, self.enda, self.endb) };
    }

    /// Advance both pointers by one byte.
    #[inline(always)]
    pub unsafe fn advance(&mut self) {
        self.pa = unsafe { self.pa.add(1) };
        self.pb = unsafe { self.pb.add(1) };
    }
}
