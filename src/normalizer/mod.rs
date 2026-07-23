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

pub mod enums;

use alloc::borrow::Cow;
use alloc::string::String;
use core::cmp::Ordering;

#[cfg(feature = "normalize")]
use simd_normalizer::casefold;

use crate::byte_utils;
pub use enums::{CaseMode, Normalization};

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
/// let norm = Normalizer::new()
///     .normalization(Normalization::Nfc)
///     .case(CaseMode::Fold);
/// assert_eq!(norm.compare("ABC", "abc"), core::cmp::Ordering::Equal);
/// assert_eq!(norm.compare("pic10", "pic2"), core::cmp::Ordering::Greater);
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
    /// Returns `Cow::Borrowed` when no transformation is needed.
    pub fn normalize<'a>(&self, s: &'a str) -> Cow<'a, str> {
        if self.normalization == Normalization::None && self.case_mode == CaseMode::Sensitive {
            return Cow::Borrowed(s);
        }

        // Step 1 — Unicode normalization.
        let s = self.apply_normalization(s);

        // Step 2 — Case folding.
        self.apply_case(s)
    }

    /// Compare two strings using this normalizer's configuration.
    ///
    /// Equivalent to normalizing both then using the optimised comparator.
    pub fn compare(&self, a: &str, b: &str) -> Ordering {
        let na = self.normalize(a);
        let nb = self.normalize(b);
        crate::compare(na.as_ref(), nb.as_ref())
    }

    // ── Internal helpers ────────────────────────────────────────────

    fn apply_normalization<'a>(&self, s: &'a str) -> Cow<'a, str> {
        match self.normalization {
            Normalization::None => Cow::Borrowed(s),
            _ => {
                // SIMD fast path: all-ASCII is already normalized.
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
                    // Without the `normalize` feature, all normalization
                    // forms silently behave as `None`.
                    Cow::Borrowed(s)
                }
            }
        }
    }

    fn apply_case<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        match self.case_mode {
            CaseMode::Sensitive => s,
            CaseMode::AsciiOnly => self.fold_ascii(s),
            CaseMode::Fold => self.fold_full(s),
        }
    }

    /// ASCII-only case folding: lowercase [A-Z] → [a-z].
    fn fold_ascii<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        if byte_utils::simd_is_ascii(s.as_bytes()) {
            if !s.as_bytes().iter().any(|b| b.is_ascii_uppercase()) {
                return s;
            }
            let mut bytes = s.as_bytes().to_vec();
            bytes.make_ascii_lowercase();
            return Cow::Owned(unsafe { String::from_utf8_unchecked(bytes) });
        }

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
    fn fold_full<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        if byte_utils::simd_is_ascii(s.as_bytes()) {
            return self.fold_ascii(s);
        }

        #[cfg(feature = "normalize")]
        {
            use simd_normalizer::CaseFoldMode;
            let folded = casefold(s.as_ref(), CaseFoldMode::Standard);
            match folded {
                Cow::Owned(owned) => Cow::Owned(owned),
                Cow::Borrowed(_) => s,
            }
        }

        #[cfg(not(feature = "normalize"))]
        {
            let mut result = String::with_capacity(s.len());
            let mut changed = false;
            for c in s.chars() {
                let folded: String = c.to_lowercase().collect();
                if folded.len() != 1 || folded.chars().next().unwrap() != c {
                    changed = true;
                }
                result.push_str(&folded);
            }
            if changed { Cow::Owned(result) } else { s }
        }
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
