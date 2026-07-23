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

#[cfg(feature = "normalize")]
use simd_normalizer::casefold;

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

    /// Compare two strings using this normalizer's configuration.
    ///
    /// Equivalent to `self.normalize(a).as_ref().cmp(self.normalize(b).as_ref())`
    /// but uses the optimized case-sensitive comparator after normalization.
    pub fn compare(&self, a: &str, b: &str) -> Ordering {
        let na = self.normalize(a);
        let nb = self.normalize(b);
        crate::compare(na.as_ref(), nb.as_ref())
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
                    // Use simd-normalizer's UnicodeNormalization trait
                    use simd_normalizer::UnicodeNormalization;
                    let norm = match self.normalization {
                        Normalization::Nfc => s.nfc(),
                        Normalization::Nfd => s.nfd(),
                        Normalization::Nfkc => s.nfkc(),
                        Normalization::Nfkd => s.nfkd(),
                        Normalization::None => unreachable!(),
                    };
                    // simd_normalizer returns Cow<'_, str>
                    norm
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

    /// Apply case folding (if configured).
    fn apply_case<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        match self.case_mode {
            CaseMode::Sensitive => s,
            CaseMode::AsciiOnly => self.fold_ascii(s),
            CaseMode::Fold => self.fold_full(s),
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
    fn fold_full<'a>(&self, s: Cow<'a, str>) -> Cow<'a, str> {
        // Fast path: if the string is all-ASCII, use ASCII fast path
        // even for full case folding (to_ascii_lowercase is correct for
        // ASCII and avoids the cost of to_lowercase()).
        if byte_utils::simd_is_ascii(s.as_bytes()) {
            return self.fold_ascii(s);
        }

        #[cfg(feature = "normalize")]
        {
            // Use simd-normalizer's SIMD-accelerated case folding.
            use simd_normalizer::CaseFoldMode;
            let folded = casefold(s.as_ref(), CaseFoldMode::Standard);
            match folded {
                Cow::Owned(owned) => Cow::Owned(owned),
                Cow::Borrowed(_) => s,
            }
        }

        #[cfg(not(feature = "normalize"))]
        {
            // Fallback: per-character `to_lowercase()` (handles
            // multi-char expansions like 'ß' → 'ss').
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
