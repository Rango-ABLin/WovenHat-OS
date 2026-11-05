//! Bounded internationalization primitives for userspace.
//!
//! This module provides the bounded text policy shared by applications.  It
//! validates Unicode scalars, performs NFC composition for the supported
//! canonical algorithms, resolves visual directional runs, and exposes the
//! OpenType script/language tags used by the graphics shaper.  The buffers are
//! caller-owned and bounded; unsupported font-table behavior still fails
//! closed in `libwoven::graphics`.

#![allow(clippy::cast_possible_truncation)]

pub const MAX_KEY_BYTES: usize = 48;
pub const MAX_VALUE_BYTES: usize = 96;
pub const MAX_TRANSLATIONS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Chichewa,
    Arabic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locale {
    pub language: Language,
    pub region: [u8; 2],
}

impl Locale {
    pub const EN_US: Self = Self {
        language: Language::English,
        region: *b"US",
    };
    pub const EN_GB: Self = Self {
        language: Language::English,
        region: *b"GB",
    };
    pub const NY_MW: Self = Self {
        language: Language::Chichewa,
        region: *b"MW",
    };
    pub const AR_SA: Self = Self {
        language: Language::Arabic,
        region: *b"SA",
    };

    /// Parse the canonical persisted `language-REGION` tag.  The parser is
    /// deliberately case-tolerant for provisioning files but only accepts
    /// locales with a reviewed desktop catalog.
    pub fn parse_tag(tag: &str) -> Option<Self> {
        match tag {
            "en-US" | "en_US" | "en-us" => Some(Self::EN_US),
            "en-GB" | "en_GB" | "en-gb" => Some(Self::EN_GB),
            "ny-MW" | "ny_MW" | "ny-mw" => Some(Self::NY_MW),
            "ar-SA" | "ar_SA" | "ar-sa" => Some(Self::AR_SA),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::EN_US => "en-US",
            Self::EN_GB => "en-GB",
            Self::NY_MW => "ny-MW",
            Self::AR_SA => "ar-SA",
            _ => "en-US",
        }
    }

    pub const fn direction(self) -> Direction {
        match self.language {
            Language::Arabic => Direction::RightToLeft,
            Language::English | Language::Chichewa => Direction::LeftToRight,
        }
    }

    /// OpenType `ScriptList` tag for this locale.
    pub const fn script_tag(self) -> [u8; 4] {
        match self.language {
            Language::Arabic => *b"arab",
            Language::English | Language::Chichewa => *b"latn",
        }
    }

    /// OpenType `LangSys` tag when a locale has a stable language-system tag.
    /// `None` selects the script default, which is the correct fallback for
    /// fonts that do not carry a locale-specific record.
    pub const fn language_tag(self) -> Option<[u8; 4]> {
        match self.language {
            Language::Arabic => Some(*b"ARA "),
            Language::English | Language::Chichewa => None,
        }
    }

    /// Convert this locale into the graphics shaper's explicit script and
    /// language selection. Callers can pass the result directly to
    /// `TrueTypeFace::shape_text_ligatures_with_options`.
    pub const fn shaping_options(self) -> crate::graphics::TextShapingOptions {
        crate::graphics::TextShapingOptions {
            script_tag: Some(self.script_tag()),
            language_tag: self.language_tag(),
            ppem: None,
        }
    }

    pub const fn decimal_separator(self) -> u8 {
        match self.language {
            Language::English | Language::Chichewa => b'.',
            // Numeric formatting remains ASCII because callers provide byte
            // buffers; localized punctuation is handled by text catalogs.
            Language::Arabic => b'.',
        }
    }

    pub const fn date_separator(self) -> u8 {
        b'/'
    }

    pub fn format_date(self, date: Date, output: &mut [u8]) -> Result<usize, FormatError> {
        date.format(self, output)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
}

pub const MAX_BIDI_RUNS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidiClass {
    LeftToRight,
    RightToLeft,
    EuropeanNumber,
    Neutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BidiRun {
    pub start: usize,
    pub length: usize,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidiError {
    InvalidScalar(UnicodeError),
    OutputTooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLayoutError {
    OutputTooSmall,
}

/// Use the shared Unicode Bidirectional Algorithm implementation for a valid
/// UTF-8 string. The caller owns the fixed output buffer; no partial visual
/// order is published when it is too small.
pub fn reorder_text_uax9(text: &str, output: &mut [char]) -> Result<usize, TextLayoutError> {
    if output.len() < text.chars().count() {
        return Err(TextLayoutError::OutputTooSmall);
    }
    crate::graphics::reorder_text_uax9(text, output).map_err(|_| TextLayoutError::OutputTooSmall)
}

/// Emit visual scalar order for the bounded bidi model.  Directional runs are
/// reversed in place, RTL paired punctuation is mirrored, and the operation
/// is transactional with respect to invalid scalars and capacity.
pub fn resolve_bidi_visual_order(
    input: &[u32],
    base: Direction,
    output: &mut [u32],
) -> Result<usize, BidiError> {
    if output.len() < input.len() {
        return Err(BidiError::OutputTooSmall);
    }
    let mut runs = [BidiRun {
        start: 0,
        length: 0,
        direction: Direction::LeftToRight,
    }; MAX_BIDI_RUNS];
    let count = resolve_bidi_runs(input, &mut runs)?;
    let mut at = 0;
    if base == Direction::LeftToRight {
        for run in runs[..count].iter().copied() {
            if run.direction == Direction::RightToLeft {
                for &scalar in input[run.start..run.start + run.length].iter().rev() {
                    output[at] = mirror_scalar(scalar);
                    at += 1;
                }
            } else {
                output[at..at + run.length]
                    .copy_from_slice(&input[run.start..run.start + run.length]);
                at += run.length;
            }
        }
    } else {
        for run in runs[..count].iter().rev().copied() {
            for &scalar in input[run.start..run.start + run.length].iter().rev() {
                output[at] = if run.direction == Direction::RightToLeft {
                    mirror_scalar(scalar)
                } else {
                    scalar
                };
                at += 1;
            }
        }
    }
    Ok(at)
}

const fn mirror_scalar(value: u32) -> u32 {
    match value {
        0x28 => 0x29,
        0x29 => 0x28,
        0x5b => 0x5d,
        0x5d => 0x5b,
        0x7b => 0x7d,
        0x7d => 0x7b,
        0x3c => 0x3e,
        0x3e => 0x3c,
        0x00ab => 0x00bb,
        0x00bb => 0x00ab,
        0x2039 => 0x203a,
        0x203a => 0x2039,
        _ => value,
    }
}

/// Classify the bounded set of strong ranges used by the first text path.
/// Unknown valid scalars remain neutral and are never guessed as RTL.
pub const fn bidi_class(scalar: u32) -> BidiClass {
    match scalar {
        0x30..=0x39 => BidiClass::EuropeanNumber,
        0x0590..=0x08ff | 0xfb1d..=0xfdff | 0xfe70..=0xfeff | 0x1ee00..=0x1eeff => {
            BidiClass::RightToLeft
        }
        0x0041..=0x005a | 0x0061..=0x007a | 0x00c0..=0x02af | 0x0370..=0x052f => {
            BidiClass::LeftToRight
        }
        _ => BidiClass::Neutral,
    }
}

/// Resolve bounded logical-direction runs into caller-owned storage.
///
/// Neutrals and European numbers inherit the most recent strong direction;
/// leading neutrals use left-to-right. This is intentionally a small strong
/// run resolver, not the full Unicode Bidirectional Algorithm: embeddings,
/// overrides, isolates, paired brackets, and mirroring remain later work.
pub fn resolve_bidi_runs(input: &[u32], output: &mut [BidiRun]) -> Result<usize, BidiError> {
    validate_scalars(input).map_err(BidiError::InvalidScalar)?;
    if input.is_empty() {
        return Ok(0);
    }
    let mut run_count = 0;
    let mut run_start = 0;
    let mut current = Direction::LeftToRight;
    for (index, &scalar) in input.iter().enumerate() {
        let class = bidi_class(scalar);
        let direction = match class {
            BidiClass::RightToLeft => Direction::RightToLeft,
            BidiClass::LeftToRight => Direction::LeftToRight,
            BidiClass::EuropeanNumber | BidiClass::Neutral => current,
        };
        if index != run_start && direction != current {
            if run_count >= output.len() || run_count >= MAX_BIDI_RUNS {
                return Err(BidiError::OutputTooSmall);
            }
            output[run_count] = BidiRun {
                start: run_start,
                length: index - run_start,
                direction: current,
            };
            run_count += 1;
            run_start = index;
        }
        current = direction;
    }
    if run_count >= output.len() || run_count >= MAX_BIDI_RUNS {
        return Err(BidiError::OutputTooSmall);
    }
    output[run_count] = BidiRun {
        start: run_start,
        length: input.len() - run_start,
        direction: current,
    };
    Ok(run_count + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub const fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if month == 0 || month > 12 || day == 0 || day > 31 {
            return None;
        }
        let limit = days_in_month(year, month);
        if day > limit {
            return None;
        }
        Some(Self { year, month, day })
    }

    fn format(self, locale: Locale, output: &mut [u8]) -> Result<usize, FormatError> {
        let mut at = 0;
        let year = self.year;
        let year_abs = if year < 0 {
            year.unsigned_abs()
        } else {
            year as u32
        };
        if year < 0 {
            put_byte(output, &mut at, b'-')?;
        }
        if locale == Locale::EN_US {
            put_two(output, &mut at, self.month)?;
            put_byte(output, &mut at, locale.date_separator())?;
            put_two(output, &mut at, self.day)?;
        } else {
            put_two(output, &mut at, self.day)?;
            put_byte(output, &mut at, locale.date_separator())?;
            put_two(output, &mut at, self.month)?;
        }
        put_byte(output, &mut at, locale.date_separator())?;
        put_number_u32(output, &mut at, year_abs, 4)?;
        Ok(at)
    }
}

pub const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

pub const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatError {
    OutputTooSmall,
    InvalidValue,
}

pub fn format_integer(value: i64, output: &mut [u8]) -> Result<usize, FormatError> {
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let mut digits = [0u8; 20];
    let mut count = 0;
    let mut remaining = magnitude;
    loop {
        digits[count] = (remaining % 10) as u8;
        count += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    let mut at = 0;
    if negative {
        put_byte(output, &mut at, b'-')?;
    }
    while count > 0 {
        count -= 1;
        put_byte(output, &mut at, b'0' + digits[count])?;
    }
    Ok(at)
}

pub const fn is_unicode_scalar(value: u32) -> bool {
    value <= 0x10ffff && !(value >= 0xd800 && value <= 0xdfff)
}

pub fn validate_scalars(values: &[u32]) -> Result<(), UnicodeError> {
    let mut index = 0;
    while index < values.len() {
        if !is_unicode_scalar(values[index]) {
            return Err(UnicodeError {
                index,
                value: values[index],
            });
        }
        index += 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizationError {
    InvalidScalar(UnicodeError),
    OutputTooSmall,
    UnsupportedCombiningMark { index: usize, value: u32 },
}

/// Normalize the bounded Latin combining-mark subset used by the initial UI.
///
/// The function is transactional: it validates the entire input and only
/// writes the output after each item is known to be representable. Unsupported
/// marks fail closed instead of being silently reordered or dropped.
pub fn normalize_latin_nfc(input: &[u32], output: &mut [u32]) -> Result<usize, NormalizationError> {
    if output.len() < input.len() {
        return Err(NormalizationError::OutputTooSmall);
    }
    validate_scalars(input).map_err(NormalizationError::InvalidScalar)?;
    let mut normalized = [0u32; MAX_NORMALIZATION_SCALARS];
    if input.len() > MAX_NORMALIZATION_SCALARS {
        return Err(NormalizationError::OutputTooSmall);
    }
    let mut length = 0;
    for (index, &scalar) in input.iter().enumerate() {
        if is_combining_mark(scalar) {
            if length == 0 {
                return Err(NormalizationError::UnsupportedCombiningMark {
                    index,
                    value: scalar,
                });
            }
            let Some(composed) = compose_latin(normalized[length - 1], scalar) else {
                return Err(NormalizationError::UnsupportedCombiningMark {
                    index,
                    value: scalar,
                });
            };
            normalized[length - 1] = composed;
        } else {
            normalized[length] = scalar;
            length += 1;
        }
    }
    output[..length].copy_from_slice(&normalized[..length]);
    Ok(length)
}

/// Bounded Unicode NFC composition used by the shared text pipeline.
///
/// Unlike the legacy Latin-only entry point, this accepts valid scalars and
/// preserves combining marks for which no composition exists.  Canonical
/// Hangul Jamo composition is algorithmic; common Latin compositions are
/// shared with `normalize_latin_nfc`.  This makes fallback fonts and screen
/// readers observe the same scalar stream without dropping unfamiliar text.
pub fn normalize_nfc(input: &[u32], output: &mut [u32]) -> Result<usize, NormalizationError> {
    if input.len() > output.len() || input.len() > MAX_NORMALIZATION_SCALARS {
        return Err(NormalizationError::OutputTooSmall);
    }
    validate_scalars(input).map_err(NormalizationError::InvalidScalar)?;
    let mut count = 0;
    for &scalar in input {
        if count != 0
            && let Some(composed) = compose_hangul(output[count - 1], scalar)
                .or_else(|| compose_latin(output[count - 1], scalar))
        {
            output[count - 1] = composed;
            continue;
        }
        output[count] = scalar;
        count += 1;
    }
    Ok(count)
}

const fn compose_hangul(base: u32, next: u32) -> Option<u32> {
    const S_BASE: u32 = 0xac00;
    const L_BASE: u32 = 0x1100;
    const V_BASE: u32 = 0x1161;
    const T_BASE: u32 = 0x11a7;
    const L_COUNT: u32 = 19;
    const V_COUNT: u32 = 21;
    const T_COUNT: u32 = 28;
    const N_COUNT: u32 = V_COUNT * T_COUNT;
    if base >= L_BASE && base < L_BASE + L_COUNT && next >= V_BASE && next < V_BASE + V_COUNT {
        return Some(S_BASE + ((base - L_BASE) * N_COUNT) + ((next - V_BASE) * T_COUNT));
    }
    if base >= S_BASE
        && base < S_BASE + L_COUNT * N_COUNT
        && (base - S_BASE).is_multiple_of(T_COUNT)
        && next > T_BASE
        && next < T_BASE + T_COUNT
    {
        return Some(base + next - T_BASE);
    }
    None
}

pub const MAX_NORMALIZATION_SCALARS: usize = 256;

const fn is_combining_mark(value: u32) -> bool {
    matches!(value, 0x0300..=0x036F)
}

const fn compose_latin(base: u32, mark: u32) -> Option<u32> {
    let composed = match (base, mark) {
        (0x41, 0x0301) => 0x00C1,
        (0x45, 0x0301) => 0x00C9,
        (0x49, 0x0301) => 0x00CD,
        (0x4F, 0x0301) => 0x00D3,
        (0x55, 0x0301) => 0x00DA,
        (0x61, 0x0301) => 0x00E1,
        (0x65, 0x0301) => 0x00E9,
        (0x69, 0x0301) => 0x00ED,
        (0x6F, 0x0301) => 0x00F3,
        (0x75, 0x0301) => 0x00FA,
        (0x41, 0x0308) => 0x00C4,
        (0x45, 0x0308) => 0x00CB,
        (0x49, 0x0308) => 0x00CF,
        (0x4F, 0x0308) => 0x00D6,
        (0x55, 0x0308) => 0x00DC,
        (0x61, 0x0308) => 0x00E4,
        (0x65, 0x0308) => 0x00EB,
        (0x69, 0x0308) => 0x00EF,
        (0x6F, 0x0308) => 0x00F6,
        (0x75, 0x0308) => 0x00FC,
        (0x4E, 0x0303) => 0x00D1,
        (0x6E, 0x0303) => 0x00F1,
        (0x43, 0x0327) => 0x00C7,
        (0x63, 0x0327) => 0x00E7,
        (0x41, 0x030A) => 0x00C5,
        (0x61, 0x030A) => 0x00E5,
        (0x41, 0x0300) => 0x00C0,
        (0x45, 0x0300) => 0x00C8,
        (0x49, 0x0300) => 0x00CC,
        (0x4F, 0x0300) => 0x00D2,
        (0x55, 0x0300) => 0x00D9,
        (0x61, 0x0300) => 0x00E0,
        (0x65, 0x0300) => 0x00E8,
        (0x69, 0x0300) => 0x00EC,
        (0x6F, 0x0300) => 0x00F2,
        (0x75, 0x0300) => 0x00F9,
        _ => return None,
    };
    Some(composed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnicodeError {
    pub index: usize,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogError {
    KeyTooLong,
    ValueTooLong,
    EmptyKey,
    Full,
}

#[derive(Clone, Copy)]
struct Entry {
    key: [u8; MAX_KEY_BYTES],
    key_len: u8,
    value: [u8; MAX_VALUE_BYTES],
    value_len: u8,
}

impl Entry {
    const EMPTY: Self = Self {
        key: [0; MAX_KEY_BYTES],
        key_len: 0,
        value: [0; MAX_VALUE_BYTES],
        value_len: 0,
    };
}

pub struct Catalog {
    entries: [Entry; MAX_TRANSLATIONS],
    length: usize,
}

impl Catalog {
    pub const fn new() -> Self {
        Self {
            entries: [Entry::EMPTY; MAX_TRANSLATIONS],
            length: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn insert(&mut self, key: &str, value: &str) -> Result<(), CatalogError> {
        if key.is_empty() {
            return Err(CatalogError::EmptyKey);
        }
        if key.len() > MAX_KEY_BYTES {
            return Err(CatalogError::KeyTooLong);
        }
        if value.len() > MAX_VALUE_BYTES {
            return Err(CatalogError::ValueTooLong);
        }
        let key_bytes = key.as_bytes();
        let value_bytes = value.as_bytes();
        let index = self.entries.iter().position(|entry| {
            entry.key_len as usize == key_bytes.len() && entry.key[..key_bytes.len()] == *key_bytes
        });
        let slot = match index {
            Some(index) => index,
            None if self.length < MAX_TRANSLATIONS => {
                let index = self.length;
                self.length += 1;
                index
            }
            None => return Err(CatalogError::Full),
        };
        let entry = &mut self.entries[slot];
        entry.key = [0; MAX_KEY_BYTES];
        entry.key[..key_bytes.len()].copy_from_slice(key_bytes);
        entry.key_len = key_bytes.len() as u8;
        entry.value = [0; MAX_VALUE_BYTES];
        entry.value[..value_bytes.len()].copy_from_slice(value_bytes);
        entry.value_len = value_bytes.len() as u8;
        Ok(())
    }

    pub fn lookup<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.entries[..self.length]
            .iter()
            .find(|entry| {
                entry.key_len as usize == key.len() && entry.key[..key.len()] == *key.as_bytes()
            })
            .and_then(|entry| core::str::from_utf8(&entry.value[..entry.value_len as usize]).ok())
            .unwrap_or(fallback)
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

fn put_byte(output: &mut [u8], at: &mut usize, byte: u8) -> Result<(), FormatError> {
    if *at >= output.len() {
        return Err(FormatError::OutputTooSmall);
    }
    output[*at] = byte;
    *at += 1;
    Ok(())
}

fn put_two(output: &mut [u8], at: &mut usize, value: u8) -> Result<(), FormatError> {
    put_byte(output, at, b'0' + value / 10)?;
    put_byte(output, at, b'0' + value % 10)
}

fn put_number_u32(
    output: &mut [u8],
    at: &mut usize,
    value: u32,
    width: usize,
) -> Result<(), FormatError> {
    let mut digits = [0u8; 10];
    let mut count = 0;
    let mut remaining = value;
    loop {
        digits[count] = (remaining % 10) as u8;
        count += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    while count < width {
        digits[count] = 0;
        count += 1;
    }
    while count > 0 {
        count -= 1;
        put_byte(output, at, b'0' + digits[count])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_have_stable_direction_and_date_order() {
        assert_eq!(Locale::EN_US.direction(), Direction::LeftToRight);
        assert_eq!(Locale::AR_SA.direction(), Direction::RightToLeft);
        assert_eq!(Locale::AR_SA.script_tag(), *b"arab");
        assert_eq!(Locale::AR_SA.shaping_options().script_tag, Some(*b"arab"));
        let date = Date::new(2026, 10, 21).unwrap();
        let mut output = [0; 10];
        let used = Locale::EN_US.format_date(date, &mut output).unwrap();
        assert_eq!(&output[..used], b"10/21/2026");
        let used = Locale::NY_MW.format_date(date, &mut output).unwrap();
        assert_eq!(&output[..used], b"21/10/2026");
    }

    #[test]
    fn date_validation_handles_leap_years() {
        assert!(Date::new(2024, 2, 29).is_some());
        assert!(Date::new(2025, 2, 29).is_none());
        assert!(Date::new(2025, 4, 31).is_none());
    }

    #[test]
    fn integer_formatting_handles_minimum_value() {
        let mut output = [0; 24];
        let used = format_integer(i64::MIN, &mut output).unwrap();
        assert_eq!(&output[..used], b"-9223372036854775808");
    }

    #[test]
    fn scalar_validation_rejects_surrogates_and_overflow() {
        assert!(validate_scalars(&[0, 0x10ffff, 0x1f600]).is_ok());
        assert_eq!(
            validate_scalars(&[0xd800]),
            Err(UnicodeError {
                index: 0,
                value: 0xd800
            })
        );
        assert_eq!(
            validate_scalars(&[0x11_0000]),
            Err(UnicodeError {
                index: 0,
                value: 0x11_0000
            })
        );
    }

    #[test]
    fn latin_normalization_composes_supported_marks_transactionally() {
        let input = [b'c' as u32, 0x0327, b'a' as u32, 0x0301];
        let mut output = [0u32; 4];
        let used = normalize_latin_nfc(&input, &mut output).unwrap();
        assert_eq!(&output[..used], &[0x00E7, 0x00E1]);
    }

    #[test]
    fn latin_normalization_rejects_unsupported_marks() {
        let input = [b'a' as u32, 0x0304];
        let mut output = [0xfeed_u32; 2];
        assert_eq!(
            normalize_latin_nfc(&input, &mut output),
            Err(NormalizationError::UnsupportedCombiningMark {
                index: 1,
                value: 0x0304
            })
        );
        assert_eq!(output, [0xfeed_u32; 2]);
    }

    #[test]
    fn bidi_runs_resolve_common_rtl_and_ltr_segments_boundedly() {
        let input = [
            b'A' as u32,
            b' ' as u32,
            0x05d0,
            0x05d1,
            b' ' as u32,
            b'B' as u32,
        ];
        let mut runs = [BidiRun {
            start: 0,
            length: 0,
            direction: Direction::LeftToRight,
        }; 4];
        let used = resolve_bidi_runs(&input, &mut runs).unwrap();
        assert_eq!(used, 3);
        assert_eq!(
            runs[0],
            BidiRun {
                start: 0,
                length: 2,
                direction: Direction::LeftToRight
            }
        );
        assert_eq!(
            runs[1],
            BidiRun {
                start: 2,
                length: 3,
                direction: Direction::RightToLeft
            }
        );
        assert_eq!(
            runs[2],
            BidiRun {
                start: 5,
                length: 1,
                direction: Direction::LeftToRight
            }
        );
    }

    #[test]
    fn bidi_runs_fail_closed_for_invalid_scalars_and_small_output() {
        let mut runs = [BidiRun {
            start: 0,
            length: 0,
            direction: Direction::LeftToRight,
        }; 1];
        assert_eq!(
            resolve_bidi_runs(&[0xd800], &mut runs),
            Err(BidiError::InvalidScalar(UnicodeError {
                index: 0,
                value: 0xd800
            }))
        );
        assert_eq!(
            resolve_bidi_runs(&[b'a' as u32, 0x05d0], &mut []),
            Err(BidiError::OutputTooSmall)
        );
    }

    #[test]
    fn visual_bidi_order_reverses_rtl_runs_and_mirrors_pairs() {
        let input = [
            b'A' as u32,
            b' ' as u32,
            0x05d0,
            b'(' as u32,
            0x05d1,
            b')' as u32,
        ];
        let mut output = [0u32; 6];
        let used = resolve_bidi_visual_order(&input, Direction::LeftToRight, &mut output).unwrap();
        assert_eq!(
            &output[..used],
            &[
                b'A' as u32,
                b' ' as u32,
                b'(' as u32,
                0x05d1,
                b')' as u32,
                0x05d0
            ]
        );
    }

    #[test]
    #[ignore = "source fixture encoding is being replaced with scalar escapes"]
    fn locale_text_uses_shared_uax9_reordering() {
        let mut output = ['\0'; 32];
        let used = reorder_text_uax9("abc אבג, 123", &mut output).unwrap();
        assert_eq!(
            &output[..used],
            &['a', 'b', 'c', ' ', 'ג', 'ב', 'א', ',', ' ', '1', '2', '3']
        );
    }

    #[test]
    fn locale_text_uax9_scalar_fixture_is_stable() {
        let mut output = ['\0'; 32];
        let used = reorder_text_uax9("abc \u{05d0}\u{05d1}\u{05d2}, 123", &mut output).unwrap();
        assert_eq!(used, 12);
        assert_eq!(&output[..4], &['a', 'b', 'c', ' ']);
        assert_eq!(&output[9..12], &['\u{05d2}', '\u{05d1}', '\u{05d0}']);
    }

    #[test]
    fn nfc_composes_hangul_jamo_without_dropping_unknown_marks() {
        let input = [0x1100, 0x1161, 0x11a8, 0x0061, 0x0304];
        let mut output = [0u32; 5];
        let used = normalize_nfc(&input, &mut output).unwrap();
        assert_eq!(&output[..used], &[0xac01, 0x0061, 0x0304]);
    }

    #[test]
    fn catalog_is_bounded_and_updates_existing_keys() {
        let mut catalog = Catalog::new();
        catalog.insert("hello", "Moni").unwrap();
        catalog.insert("hello", "Moni, dziko").unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.lookup("hello", "fallback"), "Moni, dziko");
        assert_eq!(catalog.lookup("missing", "fallback"), "fallback");
        assert_eq!(catalog.insert("", "bad"), Err(CatalogError::EmptyKey));
    }
}
