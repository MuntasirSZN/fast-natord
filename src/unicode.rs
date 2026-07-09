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
