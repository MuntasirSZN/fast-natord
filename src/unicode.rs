#[inline(always)]
pub fn utf8_char_len(first: u8) -> usize {
    if first < 128 {
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

#[inline(always)]
pub fn decode_char(s: &[u8]) -> (char, usize) {
    let first = s[0];
    if first < 128 {
        return (first as char, 1);
    }
    let len = utf8_char_len(first);
    let ch = unsafe {
        let slice = core::str::from_utf8_unchecked(&s[..len]);
        slice.chars().next().unwrap_unchecked()
    };
    (ch, len)
}

#[cfg(test)]
mod tests {
    use super::*;
    // On wasm32 `#[test]` delegates to wasm_bindgen_test.
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn test_utf8_char_len_ascii() {
        assert_eq!(utf8_char_len(0x00), 1);
        assert_eq!(utf8_char_len(0x7F), 1);
        assert_eq!(utf8_char_len(b'a'), 1);
    }

    #[test]
    fn test_utf8_char_len_2byte() {
        assert_eq!(utf8_char_len(0xC0), 2);
        assert_eq!(utf8_char_len(0xDF), 2);
        assert_eq!(utf8_char_len(0xC3), 2);
    }

    #[test]
    fn test_utf8_char_len_3byte() {
        assert_eq!(utf8_char_len(0xE0), 3);
        assert_eq!(utf8_char_len(0xEF), 3);
        assert_eq!(utf8_char_len(0xE4), 3);
    }

    #[test]
    fn test_utf8_char_len_4byte() {
        assert_eq!(utf8_char_len(0xF0), 4);
        assert_eq!(utf8_char_len(0xFF), 4);
    }

    #[test]
    fn test_decode_char_ascii() {
        assert_eq!(decode_char(b"abc"), ('a', 1));
        assert_eq!(decode_char(b"123"), ('1', 1));
        assert_eq!(decode_char(b" "), (' ', 1));
    }

    #[test]
    fn test_decode_char_2byte() {
        // U+00E9 = é = 0xC3 0xA9
        assert_eq!(decode_char(b"\xC3\xA9"), ('\u{00E9}', 2));
        assert_eq!(decode_char(b"\xC3\xA9!"), ('\u{00E9}', 2));
    }

    #[test]
    fn test_decode_char_3byte() {
        // U+4E00 = 一 = 0xE4 0xB8 0x80
        assert_eq!(decode_char(b"\xE4\xB8\x80"), ('\u{4E00}', 3));
    }

    #[test]
    fn test_utf8_char_len_continuation_byte() {
        // 0x80-0xBF are continuation bytes, should be treated as 2-byte leads
        assert_eq!(utf8_char_len(0x80), 2);
        assert_eq!(utf8_char_len(0xBF), 2);
    }

    #[test]
    fn test_decode_char_ascii_boundary() {
        // 0x7F = last 1-byte (ASCII) value
        assert_eq!(decode_char(b"\x7F"), ('\x7F', 1));
        // 0xC2 0x80 = U+0080, first 2-byte UTF-8 sequence
        assert_eq!(decode_char(b"\xC2\x80"), ('\u{80}', 2));
    }

    #[test]
    fn test_decode_char_4byte() {
        // U+1F600 = 😀 = 0xF0 0x9F 0x98 0x80
        assert_eq!(decode_char(b"\xF0\x9F\x98\x80"), ('\u{1F600}', 4));
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn utf8_char_len_matches_char_len_utf8() {
        let c: char = kani::any();
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        assert_eq!(utf8_char_len(s.as_bytes()[0]), c.len_utf8());
    }

    #[kani::proof]
    fn decode_char_roundtrip_exact() {
        let c: char = kani::any();
        let mut buf = [0u8; 4];
        let len = c.encode_utf8(&mut buf).len();
        let (decoded, adv) = decode_char(&buf[..len]);
        assert_eq!(decoded, c);
        assert_eq!(adv, len);
    }

    const TRAIL: usize = 4;

    #[kani::proof]
    #[kani::unwind(6)]
    fn decode_char_roundtrip_with_trailing_garbage() {
        let c: char = kani::any();
        let mut buf = [0u8; 4 + TRAIL];
        let clen = c.encode_utf8(&mut buf).len();

        let trailing: [u8; TRAIL] = kani::any();
        let mut i = 0;
        while i < TRAIL {
            buf[clen + i] = trailing[i];
            i += 1;
        }

        let extra: usize = kani::any();
        kani::assume(extra <= TRAIL);

        let (decoded, adv) = decode_char(&buf[..clen + extra]);
        assert_eq!(decoded, c);
        assert_eq!(adv, clen);
    }
}
