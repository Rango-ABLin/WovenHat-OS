//! Bounded internationalization primitives for userspace.
//!
//! This module deliberately stops at the data and formatting boundary. It
//! does not implement Unicode normalization, bidi, shaping, keyboard layouts,
//! or IMEs; those require compositor/input integration and remain later
//! acceptance work.

#![allow(clippy::cast_possible_truncation)]

pub const MAX_KEY_BYTES: usize = 48;
pub const MAX_VALUE_BYTES: usize = 96;
pub const MAX_TRANSLATIONS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Chichewa,
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

    pub const fn direction(self) -> Direction {
        Direction::LeftToRight
    }

    pub const fn decimal_separator(self) -> u8 {
        match self.language {
            Language::English | Language::Chichewa => b'.',
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
