//! Configuration enums for the [`Normalizer`](super::Normalizer).

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
