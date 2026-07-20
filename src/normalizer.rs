//! # Normalizer
//!
//! Pre-processes strings before comparison using configurable Unicode
//! normalization and optional case folding, keeping the hot loop free
//! of per-character normalization overhead.
//!
//! ## Design
//!
//! Normalization and case folding happen **once** per string (not inside
//! the comparison hot loop).  After pre-processing, the optimised
//! case-sensitive comparator is used — this preserves the sub-nanosecond
//! per-comparison throughput for the common case while avoiding repeated
//! per-character `to_lowercase()` calls.
//!
//! ## SIMD acceleration
//!
//! Internally, the normalizer delegates to the
//! [`simd-normalizer`](https://crates.io/crates/simd-normalizer) crate
//! (available behind the `normalize` feature), which uses a single-pass
//! SIMD-guided architecture scanning 64-byte chunks at a time.
//! Additionally, our own [`simd_is_ascii`](crate::byte_utils::simd_is_ascii)
//! check short-circuits the normalizer entirely for all-ASCII inputs.
//!
//! ## Feature flag
//!
//! The `normalize` feature (off by default) enables NFC, NFD, NFKC,
//! NKFD normalization and SIMD-accelerated case folding via
//! `simd-normalizer`.  Without it:
//!
//! * [`Normalization::Nfc`] / [`Nfd`](Normalization::Nfd) / [`Nfkc`](Normalization::Nfkc) / [`Nfkd`](Normalization::Nfkd) silently behave as
//!   [`None`](Normalization::None).
//! * [`CaseMode::Fold`] falls back to [`char::to_lowercase()`] (no SIMD
//!   acceleration).
//! * [`CaseMode::AsciiOnly`] and [`CaseMode::Sensitive`] are unaffected.

use alloc::borrow::Cow;
use alloc::string::String;
use core::cmp::Ordering;

use crate::byte_utils;

// ── Enums ────────────────────────────────────────────────────────────

/// Unicode normalization form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Normalization {
    /// No normalization.
    #[default]
    None,
    /// NFC — canonical decomposition followed by canonical composition.
    ///
    /// The most broadly useful normalization: makes canonically
    /// equivalent strings (e.g., `é` U+00E9 vs `e\u{301}`) compare
    /// as equal.  Requires the `normalize` feature; without it this
    /// variant is a silent no-op.
    Nfc,
    /// NFD — canonical decomposition.
    Nfd,
    /// NFKC — compatibility decomposition + canonical composition.
    Nfkc,
    /// NFKD — compatibility decomposition.
    Nfkd,
}

/// Case handling mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaseMode {
    /// Case-sensitive (default).
    #[default]
    Sensitive,
    /// Full Unicode case folding.
    ///
    /// When the `normalize` feature is enabled this delegates to
    /// `simd-normalizer`'s SIMD-accelerated simple case folding
    /// (CaseFolding.txt, status C+S).  Without the feature it falls
    /// back to [`char::to_lowercase()`].
    Fold,
    /// ASCII-only case folding.
    ///
    /// Non-ASCII characters are left unchanged.  Much faster than
    /// [`Fold`](CaseMode::Fold) for predominantly ASCII data.
    AsciiOnly,
}

// ── Normalizer ───────────────────────────────────────────────────────

/// A configured normalizer that pre-processes strings before comparison.
///
/// Normalization and case folding happen **once** per string, not inside
/// the comparison hot loop.  After pre-processing, the optimised
/// case-sensitive comparator is used.
///
/// # Examples
///
/// ```rust
/// use fast_natord::{Normalizer, Normalization, CaseMode};
///
/// // Case-insensitive natural sort with NFC normalisation.
/// let norm = Normalizer::new()
///     .normalization(Normalization::Nfc)
///     .case(CaseMode::Fold);
/// assert_eq!(norm.compare("ABC", "abc"), core::cmp::Ordering::Equal);
/// assert_eq!(norm.compare("pic10", "pic2"), core::cmp::Ordering::Greater);
///
/// // When the `normalize` feature is enabled, canonically equivalent
/// // strings like `é` (U+00E9) and `e\u{301}` compare as equal under NFC.
/// ```
#[derive(Clone, Debug)]
pub struct Normalizer {
    normalization: Normalization,
    case_mode: CaseMode,
}

impl Normalizer {
    /// Create a new `Normalizer` with default settings (no normalization,
    /// case-sensitive).
    #[inline]
    pub fn new() -> Self {
        Normalizer {
            normalization: Normalization::None,
            case_mode: CaseMode::Sensitive,
        }
    }

    /// Set the Unicode normalization form.
    #[inline]
    pub fn normalization(mut self, mode: Normalization) -> Self {
        self.normalization = mode;
        self
    }

    /// Convenience shorthand for [`Normalizer::normalization`]`(Normalization::Nfc)`.
    #[inline]
    pub fn nfc(self) -> Self {
        self.normalization(Normalization::Nfc)
    }

    /// Convenience shorthand for [`Normalizer::normalization`]`(Normalization::Nfd)`.
    #[inline]
    pub fn nfd(self) -> Self {
        self.normalization(Normalization::Nfd)
    }

    /// Convenience shorthand for [`Normalizer::normalization`]`(Normalization::Nfkc)`.
    #[inline]
    pub fn nfkc(self) -> Self {
        self.normalization(Normalization::Nfkc)
    }

    /// Convenience shorthand for [`Normalizer::normalization`]`(Normalization::Nfkd)`.
    #[inline]
    pub fn nfkd(self) -> Self {
        self.normalization(Normalization::Nfkd)
    }

    /// Set the case handling mode.
    #[inline]
    pub fn case(mut self, mode: CaseMode) -> Self {
        self.case_mode = mode;
        self
    }

    /// Convenience shorthand for [`Normalizer::case`]`(CaseMode::Sensitive)`.
    #[inline]
    pub fn case_sensitive(self) -> Self {
        self.case(CaseMode::Sensitive)
    }

    /// Convenience shorthand for [`Normalizer::case`]`(CaseMode::Fold)`.
    #[inline]
    pub fn case_fold(self) -> Self {
        self.case(CaseMode::Fold)
    }

    /// Convenience shorthand for [`Normalizer::case`]`(CaseMode::AsciiOnly)`.
    #[inline]
    pub fn case_ascii_only(self) -> Self {
        self.case(CaseMode::AsciiOnly)
    }

    // ── Core methods ────────────────────────────────────────────────

    /// Normalize `s` according to this normalizer's configuration.
    ///
    /// Returns `Cow::Borrowed` when no transformation is needed (e.g.,
    /// all-ASCII input with NFC normalization, or `Normalization::None`
    /// with `CaseMode::Sensitive`).
    pub fn normalize<'a>(&self, s: &'a str) -> Cow<'a, str> {
        // Fastest path: nothing to do.
        if self.normalization == Normalization::None && self.case_mode == CaseMode::Sensitive {
            return Cow::Borrowed(s);
        }

        // Step 1 — Unicode normalization (SIMD-accelerated via
        // simd-normalizer when the feature is enabled).
        let s = self.apply_normalization(s);

        // Step 2 — Case folding.
        self.apply_case(s)
    }

    /// Compare two strings after normalizing both.
    ///
    /// Applies the configured normalization and/or case folding to both
    /// strings upfront, then delegates to the SIMD-accelerated
    /// case-sensitive natural order comparator.
    ///
    /// When both inputs are all-ASCII, no allocation or transformation
    /// occurs for NFC normalization or ASCII-only case folding.
    pub fn compare(&self, a: &str, b: &str) -> Ordering {
        let na = self.normalize(a);
        let nb = self.normalize(b);
        crate::compare::compare_impl(na.as_bytes(), nb.as_bytes())
    }

    // ── Internal helpers ────────────────────────────────────────────

    /// Apply Unicode normalization (if configured).
    fn apply_normalization<'a>(&self, s: &'a str) -> Cow<'a, str> {
        match self.normalization {
            Normalization::None => Cow::Borrowed(s),
            _ => {
                // SIMD fast path: all-ASCII strings are in every normal
                // form, so we skip the normalizer entirely.
                if byte_utils::simd_is_ascii(s.as_bytes()) {
                    return Cow::Borrowed(s);
                }

                #[cfg(feature = "normalize")]
                {
                    use simd_normalizer::UnicodeNormalization;
                    match self.normalization {
                        Normalization::Nfc => s.nfc(),
                        Normalization::Nfd => s.nfd(),
                        Normalization::Nfkc => s.nfkc(),
                        Normalization::Nfkd => s.nfkd(),
                        Normalization::None => unreachable!(),
                    }
                }
                #[cfg(not(feature = "normalize"))]
                {
                    let _ = s;
                    Cow::Borrowed(s)
                }
            }
        }
    }

    /// Apply case folding (if configured).
    fn apply_case<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        match self.case_mode {
            CaseMode::Sensitive => s,
            CaseMode::Fold => self.fold_unicode(s),
            CaseMode::AsciiOnly => self.fold_ascii(s),
        }
    }

    /// ASCII-only case folding: lowercase [A-Z] → [a-z], keep other
    /// bytes unchanged.
    fn fold_ascii<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        // Fast path: if the string is all-ASCII we can do a bulk
        // lowercasing with no per-byte branching beyond the check.
        if byte_utils::simd_is_ascii(s.as_bytes()) {
            // Already-lowercase ASCII: nothing to fold, keep borrowed.
            if !s.as_bytes().iter().any(|b| b.is_ascii_uppercase()) {
                return s;
            }
            let mut bytes = s.as_bytes().to_vec();
            bytes.make_ascii_lowercase();
            // SAFETY: ASCII bytes are always valid UTF-8.
            return Cow::Owned(unsafe { String::from_utf8_unchecked(bytes) });
        }

        // Mixed ASCII/non-ASCII: fold only the ASCII bytes.
        let mut result = String::with_capacity(s.len());
        let mut changed = false;
        for c in s.chars() {
            let folded = c.to_ascii_lowercase();
            if folded != c {
                changed = true;
            }
            result.push(folded);
        }
        if changed { Cow::Owned(result) } else { s }
    }

    /// Full Unicode case folding.
    ///
    /// Delegates to `simd-normalizer`'s SIMD-accelerated case folding
    /// when the `normalize` feature is enabled; falls back to
    /// [`char::to_lowercase()`] otherwise.
    fn fold_unicode<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        #[cfg(feature = "normalize")]
        {
            match s {
                Cow::Borrowed(borrowed) => {
                    simd_normalizer::casefold(borrowed, simd_normalizer::CaseFoldMode::Standard)
                }
                Cow::Owned(owned) => {
                    match simd_normalizer::casefold(&owned, simd_normalizer::CaseFoldMode::Standard)
                    {
                        Cow::Owned(folded) => Cow::Owned(folded),
                        Cow::Borrowed(_) => Cow::Owned(owned),
                    }
                }
            }
        }
        #[cfg(not(feature = "normalize"))]
        {
            self.fold_unicode_fallback(s)
        }
    }

    /// Fallback Unicode case folding using [`char::to_lowercase()`].
    ///
    /// Used when the `normalize` feature is not enabled.
    #[cfg(not(feature = "normalize"))]
    fn fold_unicode_fallback<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        // Fast path: all-ASCII strings need only ASCII case folding.
        // Avoids char-by-char Unicode to_lowercase() overhead.
        if byte_utils::simd_is_ascii(s.as_bytes()) {
            // Already-lowercase ASCII: nothing to fold, keep borrowed.
            if !s.as_bytes().iter().any(|b| b.is_ascii_uppercase()) {
                return s;
            }
            let mut bytes = s.as_bytes().to_vec();
            bytes.make_ascii_lowercase();
            // SAFETY: ASCII bytes are always valid UTF-8.
            return Cow::Owned(unsafe { String::from_utf8_unchecked(bytes) });
        }

        // Full Unicode case folding for non-ASCII input.
        // A single-pass approach with `str::chars().position()` + slice
        // would confuse char indices with byte indices on multi-byte
        // strings and panic.
        let mut result = String::with_capacity(s.len());
        let mut changed = false;
        for c in s.chars() {
            let lc: char = c.to_lowercase().next().unwrap_or(c);
            if lc != c {
                changed = true;
            }
            result.push(lc);
        }
        if changed { Cow::Owned(result) } else { s }
    }
}

impl Default for Normalizer {
    #[inline]
    fn default() -> Self {
        Normalizer::new()
    }
}

// ── Convenience functions ────────────────────────────────────────────

/// Compare two strings using NFC normalization and Unicode case folding.
///
/// Shorthand for a [`Normalizer`] configured with
/// [`Normalization::Nfc`] + [`CaseMode::Fold`].
///
/// ```rust
/// # use fast_natord::compare_normalized;
/// assert_eq!(compare_normalized("ABC", "abc"), core::cmp::Ordering::Equal);
/// ```
#[inline]
pub fn compare_normalized(a: &str, b: &str) -> Ordering {
    NORMALIZER_NFC_FOLD.compare(a, b)
}

static NORMALIZER_NFC_FOLD: Normalizer = Normalizer {
    normalization: Normalization::Nfc,
    case_mode: CaseMode::Fold,
};

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    // On wasm32 `#[test]` delegates to wasm_bindgen_test.
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    #[test]
    fn test_noop_normalizer() {
        let n = Normalizer::new();
        assert_eq!(n.normalize("hello"), Cow::Borrowed("hello"));
        assert_eq!(n.compare("a", "b"), core::cmp::Ordering::Less);
        assert_eq!(n.compare("b", "a"), core::cmp::Ordering::Greater);
        assert_eq!(n.compare("a", "a"), core::cmp::Ordering::Equal);
    }

    #[test]
    fn test_nfc_ascii_borrowed() {
        // All-ASCII → NFC is a no-op → borrowed.
        let n = Normalizer::new().nfc();
        assert_eq!(n.normalize("hello world"), Cow::Borrowed("hello world"));
    }

    #[test]
    fn test_ascii_only_borrowed_when_no_fold_needed() {
        // Mixed ASCII/non-ASCII where all ASCII is already lowercase:
        // fold_ascii should return Borrowed (no allocation).
        let n = Normalizer::new().case_ascii_only();
        let result = n.normalize("hello\u{00E9}");
        match result {
            Cow::Borrowed(s) => assert_eq!(s, "hello\u{00E9}"),
            _ => panic!("expected Borrowed but got Owned"),
        }
    }

    #[test]
    fn test_ascii_case_fold_ascii_only() {
        let n = Normalizer::new().case_ascii_only();
        let norm = n.normalize("HelloWorld");
        assert_eq!(norm, "helloworld");
    }

    #[test]
    fn test_ascii_case_fold() {
        let n = Normalizer::new().case_fold();
        let norm = n.normalize("Hello World");
        assert_eq!(norm, "hello world");
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_nfc_decomposed() {
        // É (U+00C9) decomposed = E (U+0045) + combining acute (U+0301).
        // NFC should recompose to U+00C9.
        let n = Normalizer::new().nfc();
        let norm = n.normalize("E\u{301}");
        assert_eq!(norm, "\u{00C9}");
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_compare_normalized_nfc() {
        // NFC makes é == e + combining accent.
        let n = Normalizer::new().nfc();
        assert_eq!(n.compare("\u{00E9}", "e\u{301}"), Ordering::Equal);
    }

    #[test]
    #[cfg(feature = "normalize")]
    fn test_compare_normalized_nfc_case_fold() {
        let n = Normalizer::new().nfc().case_fold();
        // Case folding + NFC: uppercase with composition vs lowercase decomposed.
        assert_eq!(
            n.compare("Caf\u{00C9}", "caf\u{0065}\u{0301}"),
            Ordering::Equal
        );
    }

    #[test]
    fn test_case_fold_equivalent() {
        let n = Normalizer::new().case_fold();
        assert_eq!(n.compare("ABC", "abc"), Ordering::Equal);
        assert_eq!(n.compare("ABC", "abd"), Ordering::Less);
    }

    #[test]
    fn test_compare_normalized_numeric() {
        let n = Normalizer::new().nfc().case_fold();
        assert_eq!(n.compare("pic10", "pic2"), Ordering::Greater);
        assert_eq!(n.compare("pic2", "pic10"), Ordering::Less);
    }

    #[test]
    fn test_ascii_only_keeps_non_ascii() {
        let n = Normalizer::new().case_ascii_only();
        let result = n.normalize("ABCé");
        // Uppercase ASCII → lowercased; non-ASCII → untouched (é stays é).
        assert_eq!(result, "abcé");
    }

    #[test]
    fn test_compare_empty_strings() {
        let n = Normalizer::new().nfc().case_fold();
        assert_eq!(n.compare("", ""), Ordering::Equal);
        assert_eq!(n.compare("", "a"), Ordering::Less);
        assert_eq!(n.compare("a", ""), Ordering::Greater);
    }

    #[test]
    fn test_nfc_idempotent() {
        let n = Normalizer::new().nfc();
        let once = n.normalize("caf\u{00E9}").into_owned();
        let twice = n.normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn test_case_fold_idempotent() {
        let n = Normalizer::new().case_fold();
        let once = n.normalize("Hello Σ 123").into_owned();
        let twice = n.normalize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn test_leading_zeros_with_normalizer() {
        let n = Normalizer::new().nfc().case_sensitive();
        assert_eq!(n.compare("015", "12"), Ordering::Less);
        assert_eq!(n.compare("12", "015"), Ordering::Greater);
        assert_eq!(n.compare("0015", "015"), Ordering::Less);
    }

    #[test]
    fn test_whitespace_with_normalizer() {
        let n = Normalizer::new().nfc().case_sensitive();
        assert_eq!(n.compare("pic4   alpha", "pic 4 else"), Ordering::Less);
        assert_eq!(n.compare("pic 4 else", "pic4  last"), Ordering::Less);
    }

    #[test]
    fn test_long_digit_runs_normalized() {
        let n = Normalizer::new().nfc().case_fold();
        assert_eq!(n.compare("123456789", "123456788"), Ordering::Greater);
        assert_eq!(n.compare("99999", "100000"), Ordering::Less);
    }

    #[test]
    fn test_compare_normalized_mixed() {
        let n = Normalizer::new().nfc().case_fold();
        // Various real-world-like comparisons.
        assert_eq!(n.compare("RFC 2", "rfc 10"), Ordering::Less);
        assert_eq!(n.compare("rfc 10", "RFC 2"), Ordering::Greater);
        assert_eq!(n.compare("Pic 5", "pic 5"), Ordering::Equal);
    }
}
