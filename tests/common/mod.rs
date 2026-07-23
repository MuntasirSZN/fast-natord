//! Shared helpers for integration tests.
//!
//! This file is placed in a subdirectory so Cargo doesn't compile it
//! as a separate test crate.  Test files include it via `mod common;`.

use core::cmp::Ordering;
use fast_natord::compare;

/// Assert total order: for each pair `(x_i, y_j)`, verify
/// `compare(x_i, y_j)` matches `i.cmp(&j)`.
pub fn check_total_order(strs: &[&str]) {
    fn ordering_to_op(ord: Ordering) -> &'static str {
        match ord {
            Ordering::Greater => ">",
            Ordering::Equal => "=",
            Ordering::Less => "<",
        }
    }

    for (i, &x) in strs.iter().enumerate() {
        for (j, &y) in strs.iter().enumerate() {
            let actual = compare(x, y);
            let expected = i.cmp(&j);
            assert!(
                actual == expected,
                "expected x {} y, got x {} y (x = `{x}`, y = `{y}`)",
                ordering_to_op(expected),
                ordering_to_op(actual),
            );
        }
    }
}
