//! Fuzz target for `fast_natord::Normalizer` with every supported configuration.
//!
//! Exercises all four normalisation forms (None, Nfc, Nfkc, Nfd, Nfkd)
//! combined with all three case modes (Sensitive, AsciiOnly, Fold) against
//! arbitrary byte-slice pairs.

use fast_natord::{CaseMode, Normalization, Normalizer};

fn fuzz_entry(data: &[u8]) {
    // Need at least 1 byte to select configuration.
    if data.len() < 2 {
        return;
    }

    // Use first byte to pick Normalization, second byte to pick CaseMode.
    let norm = match data[0] % 5 {
        0 => Normalization::None,
        1 => Normalization::Nfc,
        2 => Normalization::Nfkd,
        3 => Normalization::Nfkc,
        _ => Normalization::Nfd,
    };
    let case = match data[1] % 3 {
        0 => CaseMode::Sensitive,
        1 => CaseMode::AsciiOnly,
        _ => CaseMode::Fold,
    };

    let normalizer = Normalizer::new().normalization(norm).case(case);

    // Split remainder at null or midpoint.
    let rest = &data[2..];
    let mid = rest.iter().position(|&b| b == 0).unwrap_or(rest.len() / 2);
    let (left, right) = rest.split_at(mid);
    let right = if mid < rest.len() && rest[mid] == 0 {
        &right[1..]
    } else {
        right
    };

    if let (Ok(l), Ok(r)) = (core::str::from_utf8(left), core::str::from_utf8(right)) {
        // Exercise both comparison paths.
        normalizer.compare(l, r);
        fast_natord::compare_normalized(l, r);
    }
}

fn main() {
    afl::fuzz!(|data: &[u8]| {
        fuzz_entry(data);
    });
}
