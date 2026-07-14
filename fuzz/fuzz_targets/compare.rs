//! Fuzz target for `fast_natord::compare` (case-sensitive natural ordering).
//!
//! Splits the AFL-supplied byte slice at the first null byte (or midpoint
//! if absent) and compares the two halves as UTF-8 strings.  The function
//! is documented as panic-free, so any crash is a real bug.

use fast_natord::compare;

fn fuzz_entry(data: &[u8]) {
    let mid = data.iter().position(|&b| b == 0).unwrap_or(data.len() / 2);
    let (left, right) = data.split_at(mid);
    // Skip the null byte if we split at one.
    let right = if mid < data.len() && data[mid] == 0 {
        &right[1..]
    } else {
        right
    };

    if let (Ok(l), Ok(r)) = (core::str::from_utf8(left), core::str::from_utf8(right)) {
        compare(l, r);
    }
}

fn main() {
    afl::fuzz!(|data: &[u8]| {
        fuzz_entry(data);
    });
}
