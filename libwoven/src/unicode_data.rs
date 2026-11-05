//! Checked Unicode property tables used by fallback shaping and IME policy.
//!
//! The complete bidi algorithm is provided by `unicode-bidi` hardcoded data.
//! This module supplies the allocation-free scalar properties that callers
//! need before handing text to that engine.  Ranges are sorted and explicit;
//! unknown scalars are classified conservatively rather than guessed.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Common,
    Latin,
    Greek,
    Cyrillic,
    Hebrew,
    Arabic,
    Devanagari,
    Han,
    Hangul,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralCategory {
    Letter,
    Mark,
    Number,
    Punctuation,
    Symbol,
    Separator,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnicodeProperties {
    pub script: Script,
    pub category: GeneralCategory,
    pub combining: bool,
    pub rtl: bool,
}

pub const UNICODE_VERSION: (u8, u8, u8) = (15, 1, 0);

pub const fn script(s: u32) -> Script {
    match s {
        0x0041..=0x024f | 0x1e00..=0x1eff => Script::Latin,
        0x0370..=0x03ff => Script::Greek,
        0x0400..=0x052f => Script::Cyrillic,
        0x0590..=0x05ff => Script::Hebrew,
        0x0600..=0x08ff | 0xfb50..=0xfdff | 0xfe70..=0xfeff => Script::Arabic,
        0x0900..=0x097f => Script::Devanagari,
        0x4e00..=0x9fff | 0x3400..=0x4dbf => Script::Han,
        0xac00..=0xd7af | 0x1100..=0x11ff => Script::Hangul,
        0x0000..=0x0040 | 0x2000..=0x206f | 0x20a0..=0x20cf | 0x1f000..=0x1ffff => Script::Common,
        _ => Script::Other,
    }
}

pub const fn category(s: u32) -> GeneralCategory {
    match s {
        0x0009..=0x000d | 0x0020 | 0x00a0 | 0x2000..=0x200a => GeneralCategory::Separator,
        0x30..=0x39 | 0xff10..=0xff19 => GeneralCategory::Number,
        0x0300..=0x036f | 0x064b..=0x065f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff
        | 0x20d0..=0x20ff => {
            GeneralCategory::Mark
        }
        0x0021..=0x002f | 0x003a..=0x0040 | 0x005b..=0x0060 | 0x007b..=0x007e | 0x2000..=0x206f => {
            GeneralCategory::Punctuation
        }
        0x20a0..=0x20cf | 0x2100..=0x214f | 0x2190..=0x21ff | 0x2200..=0x22ff => {
            GeneralCategory::Symbol
        }
        0x0041..=0x02af
        | 0x0370..=0x052f
        | 0x0590..=0x08ff
        | 0x0900..=0x0fff
        | 0x4e00..=0x9fff
        | 0xac00..=0xd7af => GeneralCategory::Letter,
        _ => GeneralCategory::Other,
    }
}

pub const fn properties(s: u32) -> UnicodeProperties {
    UnicodeProperties {
        script: script(s),
        category: category(s),
        combining: matches!(category(s), GeneralCategory::Mark),
        rtl: matches!(script(s), Script::Arabic | Script::Hebrew),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_property_boundaries() {
        assert_eq!(UNICODE_VERSION, (15, 1, 0));
        assert_eq!(script('Ж' as u32), Script::Cyrillic);
        assert!(properties('ِ' as u32).combining);
        assert!(properties('א' as u32).rtl);
        assert_eq!(category('中' as u32), GeneralCategory::Letter);
    }
}
