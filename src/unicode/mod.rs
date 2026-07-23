//! UTF-8 character length detection and decoding utilities.

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani;

/// Returns the byte length of a UTF-8 character given its leading byte.
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

/// Decode a UTF-8 character from the start of a byte slice.
///
/// Returns the decoded `char` and its byte length (1–4).
#[inline(always)]
pub fn decode_char(s: &[u8]) -> (char, usize) {
    let first = s[0];
    if first < 128 {
        return (first as char, 1);
    }
    let len = utf8_char_len(first);
    let ch = unsafe {
        let mut buf = [0u8; 4];
        buf[..len].copy_from_slice(&s[..len]);
        core::str::from_utf8_unchecked(&buf[..len])
            .chars()
            .next()
            .unwrap_or(char::REPLACEMENT_CHARACTER)
    };
    (ch, len)
}
