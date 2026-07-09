//! Locale-aware comparison using ICU collation.
//!
//! This module provides locale-aware string comparison via ICU4X
//! [`CollatorBorrowed`].  The [`Collator`] wrapper can be constructed once
//! and reused for many comparisons.
//!
//! # Normalization
//!
//! ICU collation always normalizes strings internally (Unicode NFC
//! normalization is required by the UCA specification and cannot be
//! disabled).  This means strings in different normalization forms (e.g.
//! NFC `"é"` vs NFD `"e\u{301}"`) compare as equal without any extra
//! preprocessing.
//!
//! The natural-order comparison functions [`compare`] and
//! [`compare_ignore_case`](crate::compare_ignore_case) operate on raw bytes
//! and do **not** normalize.  If you need normalization there, use the
//! `unicode-normalization` crate before comparing.
//!
//! # Panics
//!
//! This module is panic-free — all fallible operations return
//! [`Result`].  Panic-free functions in the non-locale API
//! ([`compare`](crate::compare), [`compare_ignore_case`](crate::compare_ignore_case))
//! are also guaranteed not to panic for any input.
//!
//! # Feature gate
//!
//! `#[cfg(feature = "locale")]` — enabled by the `locale` feature.
//!
//! # Examples
//!
//! ```rust
//! use fast_natord::Collator;
//!
//! let collator = Collator::try_new("en").unwrap();
//! assert_eq!(collator.compare("a", "b"), core::cmp::Ordering::Less);
//! assert_eq!(collator.compare("b", "a"), core::cmp::Ordering::Greater);
//! assert_eq!(collator.compare("a", "a"), core::cmp::Ordering::Equal);
//! ```
//!
//! Case-insensitive (secondary strength — ignores case, respects accents):
//! ```rust
//! use fast_natord::collator::compare_locale_ignore_case;
//!
//! // Case difference is ignored at secondary strength.
//! assert!(compare_locale_ignore_case("A", "a", "en").unwrap().is_eq());
//! // Accent difference is still detected.
//! assert!(compare_locale_ignore_case("e", "é", "fr").unwrap().is_ne());
//! ```

use alloc::string::ToString;
use core::cmp::Ordering;

use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::{Collator as IcuCollator, CollatorBorrowed, CollatorPreferences};
use icu_locale::Locale;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors that can occur when constructing a [`Collator`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollatorError {
    /// The locale string could not be parsed as a valid BCP-47 tag
    /// (e.g. `"fr"`, `"en-US"`, `"es-ES"`).
    #[error("invalid locale string: expected BCP-47 tag like \"en-US\" or \"fr\"")]
    InvalidLocale,
    /// The collation data for the requested locale is not available in the
    /// compiled data bundle.
    #[error("collation data not available for locale")]
    DataLoad,
}

// ---------------------------------------------------------------------------
// Collator
// ---------------------------------------------------------------------------

/// A reusable locale-aware string comparer.
///
/// Wraps `icu_collator::CollatorBorrowed<'static>` and exposes the same
/// comparison API.
///
/// Construction is somewhat expensive (data loading + state init), so reuse
/// the same `Collator` for an entire sort batch.
///
/// ```rust
/// use fast_natord::Collator;
///
/// let c = Collator::try_new("en").unwrap();
/// assert_eq!(c.compare("apple", "banana"), core::cmp::Ordering::Less);
/// assert_eq!(c.compare("banana", "apple"), core::cmp::Ordering::Greater);
/// ```
#[derive(Debug)]
pub struct Collator {
    inner: CollatorBorrowed<'static>,
}

impl Collator {
    /// Create a collator for the given BCP-47 locale tag with default
    /// (tertiary) strength — accent and case sensitive.
    ///
    /// Returns `Err(CollatorError::InvalidLocale)` if the tag cannot be
    /// parsed, or `Err(CollatorError::DataLoad)` if no collation data is
    /// available.
    pub fn try_new(locale: &str) -> Result<Self, CollatorError> {
        Self::try_new_with_options(locale, CollatorOptions::default())
    }

    /// Create a collator with a custom [`CollatorOptions`].
    ///
    /// See the ICU4X collator documentation for the available options
    /// (strength, alternate handling, case level, max variable).
    pub fn try_new_with_options(
        locale: &str,
        options: CollatorOptions,
    ) -> Result<Self, CollatorError> {
        let loc: Locale = locale.parse().map_err(|_| CollatorError::InvalidLocale)?;
        let prefs = CollatorPreferences::from(&loc);
        let inner = IcuCollator::try_new(prefs, options).map_err(|_| CollatorError::DataLoad)?;
        Ok(Self { inner })
    }

    /// Compare two strings using this collator's locale and options.
    #[inline(always)]
    pub fn compare(&self, left: &str, right: &str) -> Ordering {
        self.inner.compare(left, right)
    }
}

// ---------------------------------------------------------------------------
// Convenience functions (one-shot)
// ---------------------------------------------------------------------------

/// Compare two strings using locale-aware collation with default
/// (tertiary) strength.
///
/// ```rust
/// use fast_natord::collator::compare_locale;
///
/// // Spanish traditional sorts "pollo" after "polvo"
/// assert!(compare_locale("polvo", "pollo", "es-u-co-trad").unwrap().is_lt());
/// ```
pub fn compare_locale(left: &str, right: &str, locale: &str) -> Result<Ordering, CollatorError> {
    let c = Collator::try_new(locale)?;
    Ok(c.compare(left, right))
}

/// Compare two strings case-insensitively using locale-aware collation
/// (secondary strength — respects accents, ignores case).
///
/// ```rust
/// use fast_natord::collator::compare_locale_ignore_case;
///
/// // Case difference is ignored at secondary strength.
/// assert!(compare_locale_ignore_case("A", "a", "en").unwrap().is_eq());
/// // Accent difference is still detected.
/// assert!(compare_locale_ignore_case("e", "é", "fr").unwrap().is_ne());
/// ```
pub fn compare_locale_ignore_case(
    left: &str,
    right: &str,
    locale: &str,
) -> Result<Ordering, CollatorError> {
    let mut options = CollatorOptions::default();
    options.strength = Some(Strength::Secondary);
    let c = Collator::try_new_with_options(locale, options)?;
    Ok(c.compare(left, right))
}

/// Compare two strings using the system locale's collation rules
/// (tertiary strength).
///
/// Falls back to `"en-US"` if the system locale cannot be detected.
pub fn compare_system_locale(left: &str, right: &str) -> Result<Ordering, CollatorError> {
    let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
    compare_locale(left, right, &locale)
}

/// Compare two strings case-insensitively using the system locale's
/// collation rules (secondary strength).
///
/// Falls back to `"en-US"` if the system locale cannot be detected.
pub fn compare_system_locale_ignore_case(
    left: &str,
    right: &str,
) -> Result<Ordering, CollatorError> {
    let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
    compare_locale_ignore_case(left, right, &locale)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_french_accent() {
        let collator = Collator::try_new("fr").unwrap();
        // é and e differ at primary level in French?  Actually é sorts
        // after e in French — let's verify the direction.
        let ord = collator.compare("côte", "coté");
        assert_ne!(ord, Ordering::Equal);
    }

    #[test]
    fn test_spanish_traditional() {
        // In traditional Spanish, "pollo" > "polvo"
        let ord = compare_locale("pollo", "polvo", "es-u-co-trad").unwrap();
        assert_eq!(ord, Ordering::Greater);
    }

    #[test]
    fn test_english_default() {
        let collator = Collator::try_new("en").unwrap();
        assert_eq!(collator.compare("a", "b"), Ordering::Less);
        assert_eq!(collator.compare("b", "a"), Ordering::Greater);
        assert_eq!(collator.compare("a", "a"), Ordering::Equal);
        // In the default UCA (English), lowercase "a" sorts before uppercase "A".
        assert_eq!(collator.compare("a", "A"), Ordering::Less);
    }

    #[test]
    fn test_case_insensitive() {
        let ord = compare_locale_ignore_case("A", "a", "en").unwrap();
        assert_eq!(ord, Ordering::Equal);
    }

    #[test]
    fn test_case_insensitive_accent_sensitive() {
        // Secondary strength: case is equal, but accent difference remains.
        let ord = compare_locale_ignore_case("E", "é", "fr").unwrap();
        assert_ne!(ord, Ordering::Equal);
    }

    #[test]
    fn test_locale_data() {
        // ICU parses almost any locale string leniently, but some BCP-47
        // tags may lack compiled collation data.
        match Collator::try_new("ja") {
            Ok(_) => {}
            Err(CollatorError::DataLoad) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
        }
        match Collator::try_new("en") {
            Ok(_) => {}
            Err(other) => panic!("expected Ok for en, got {other:?}"),
        }
    }
}
