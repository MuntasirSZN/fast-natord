//! Kani proofs for UTF-8 utilities.

#![cfg(kani)]

use crate::unicode::utf8_char_len;

#[kani::proof]
fn utf8_char_len_range() {
    let c: u8 = kani::any();
    let n = utf8_char_len(c);
    assert!((1..=4).contains(&n));
}

#[kani::proof]
fn utf8_char_len_spec() {
    let c: u8 = kani::any();
    let n = utf8_char_len(c);
    if c < 128 {
        assert_eq!(n, 1);
    } else if c < 0xE0 {
        assert_eq!(n, 2);
    } else if c < 0xF0 {
        assert_eq!(n, 3);
    } else {
        assert_eq!(n, 4);
    }
}
