//! Low-level byte and SIMD utilities for natural ordering.
//!
//! This module provides SIMD-accelerated building blocks used by the
//! comparison hot loops: prefix skipping, ASCII detection, digit-run
//! end scanning, word-at-a-time comparison, and basic byte predicates.

mod basic;
mod compare_word_at_a_time;
mod dispatch;
mod is_ascii;
mod skip_equal;
mod skip_while_digit;

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani;

pub use basic::{is_ascii_ws, is_digit, skip_whitespace};
pub use compare_word_at_a_time::compare_word_at_a_time;
pub use is_ascii::simd_is_ascii;
pub use skip_equal::simd_skip_equal;
pub use skip_while_digit::simd_skip_while_digit_both;
