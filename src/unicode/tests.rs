//! Tests for UTF-8 decoding utilities.

use super::*;

// On wasm32 `#[test]` delegates to wasm_bindgen_test.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn test_utf8_char_len_ascii() {
    assert_eq!(utf8_char_len(0b0000_0000), 1);
    assert_eq!(utf8_char_len(0b0111_1111), 1);
}

#[test]
fn test_utf8_char_len_2byte() {
    assert_eq!(utf8_char_len(0b1100_0000), 2);
    assert_eq!(utf8_char_len(0b1101_1111), 2);
}

#[test]
fn test_utf8_char_len_3byte() {
    assert_eq!(utf8_char_len(0b1110_0000), 3);
    assert_eq!(utf8_char_len(0b1110_1111), 3);
}

#[test]
fn test_utf8_char_len_4byte() {
    assert_eq!(utf8_char_len(0b1111_0000), 4);
    assert_eq!(utf8_char_len(0b1111_0111), 4);
}

#[test]
fn test_decode_char_ascii() {
    assert_eq!(decode_char(b"a"), ('a', 1));
    assert_eq!(decode_char(b"Z"), ('Z', 1));
}

#[test]
fn test_decode_char_2byte() {
    assert_eq!(decode_char(b"\xC3\xA9"), ('é', 2));
}

#[test]
fn test_decode_char_3byte() {
    assert_eq!(decode_char(b"\xE2\x82\xAC"), ('€', 3));
}

#[test]
fn test_utf8_char_len_continuation_byte() {
    // Continuation bytes are not valid leading bytes, but the function
    // still returns a reasonable length (it would be treated as 2-byte).
    assert_eq!(utf8_char_len(0b1000_0000), 2);
    assert_eq!(utf8_char_len(0b1011_1111), 2);
}

#[test]
fn test_decode_char_ascii_boundary() {
    assert_eq!(decode_char(b"\x7F"), ('\x7F', 1));
    assert_eq!(decode_char(b"\x00"), ('\0', 1));
}

#[test]
fn test_decode_char_4byte() {
    assert_eq!(decode_char(b"\xF0\x9F\x8D\x8E"), ('🍎', 4));
}
