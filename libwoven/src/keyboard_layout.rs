//! Bounded keyboard-layout and composition policy for userspace controls.
//!
//! Hardware drivers deliver physical set-1/HID positions.  This module keeps
//! the locale policy above that ABI, so changing a layout never changes the
//! kernel input record.  The tables cover the printable ISO-105 key positions
//! used by the supported desktop locales and fail closed for unknown keys.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Us,
    Uk,
    German,
    French,
    Spanish,
    SwissGerman,
    Italian,
    Portuguese,
    Nordic,
    Arabic,
}

impl Layout {
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "us" | "en-US" | "en_US" => Some(Self::Us),
            "gb" | "uk" | "en-GB" | "en_GB" => Some(Self::Uk),
            "de" | "de-DE" | "de_DE" => Some(Self::German),
            "fr" | "fr-FR" | "fr_FR" => Some(Self::French),
            "es" | "es-ES" | "es_ES" => Some(Self::Spanish),
            "ch" | "de-CH" | "de_CH" => Some(Self::SwissGerman),
            "it" | "it-IT" | "it_IT" => Some(Self::Italian),
            "pt" | "pt-PT" | "pt_PT" => Some(Self::Portuguese),
            "no" | "da" | "sv" | "fi" | "nb-NO" | "sv-SE" => Some(Self::Nordic),
            "ar" | "ar-SA" | "ar_SA" => Some(Self::Arabic),
            _ => None,
        }
    }

    pub const fn tag(self) -> &'static str {
        match self {
            Self::Us => "us",
            Self::Uk => "gb",
            Self::German => "de",
            Self::French => "fr",
            Self::Spanish => "es",
            Self::SwissGerman => "ch",
            Self::Italian => "it",
            Self::Portuguese => "pt",
            Self::Nordic => "nordic",
            Self::Arabic => "ar",
        }
    }

    /// Translate a PS/2 set-1 physical make code. `altgr` is supplied by HID
    /// or a higher-level modifier tracker; the kernel's stable `Key` ABI is
    /// intentionally unchanged.
    pub fn map_set1(self, code: u8, shift: bool, altgr: bool) -> Option<char> {
        let base = match code {
            0x02..=0x0b => number(code),
            0x10..=0x19 => row(code, 0x10, b"qwertyuiop"),
            0x1e..=0x26 => row(code, 0x1e, b"asdfghjkl"),
            0x2c..=0x32 => row(code, 0x2c, b"zxcvbnm"),
            0x39 => Some(' '),
            0x0c => Some('-'),
            0x0d => Some('='),
            0x1a => Some('['),
            0x1b => Some(']'),
            0x27 => Some(';'),
            0x28 => Some('\''),
            0x29 => Some('`'),
            0x2b => Some('\\'),
            0x33 => Some(','),
            0x34 => Some('.'),
            0x35 => Some('/'),
            _ => None,
        }?;
        let translated = match self {
            Self::French => azerty(base, shift, altgr),
            Self::German | Self::SwissGerman => german(base, shift, altgr),
            Self::Spanish => spanish(base, shift, altgr),
            Self::Italian => italian(base, shift, altgr),
            Self::Portuguese => portuguese(base, shift, altgr),
            Self::Nordic => nordic(base, shift, altgr),
            Self::Arabic => arabic(base, shift, altgr),
            Self::Uk => uk(base, shift, altgr),
            Self::Us => base,
        };
        Some(translated)
    }
}

fn row(code: u8, start: u8, row: &[u8]) -> Option<char> {
    row.get(usize::from(code.saturating_sub(start)))
        .map(|b| char::from(*b))
}

fn number(code: u8) -> Option<char> {
    Some(match code {
        0x02 => '1',
        0x03 => '2',
        0x04 => '3',
        0x05 => '4',
        0x06 => '5',
        0x07 => '6',
        0x08 => '7',
        0x09 => '8',
        0x0a => '9',
        0x0b => '0',
        _ => return None,
    })
}

fn shifted(c: char, shift: bool) -> char {
    if !shift {
        return c;
    }
    match c {
        'a'..='z' => c.to_ascii_uppercase(),
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        ';' => ':',
        '\'' => '"',
        '`' => '~',
        '\\' => '|',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        other => other,
    }
}

fn azerty(c: char, shift: bool, altgr: bool) -> char {
    let c = match c {
        'q' => 'a',
        'w' => 'z',
        'a' => 'q',
        'z' => 'w',
        'm' if !shift => ',',
        ',' if !shift => ';',
        ';' if !shift => 'm',
        _ => c,
    };
    if altgr {
        return match c {
            '2' => '~',
            '0' => '@',
            'e' => '€',
            _ => c,
        };
    }
    shifted(c, shift)
}

fn german(c: char, shift: bool, altgr: bool) -> char {
    if altgr {
        return match c {
            'q' => '@',
            'e' => '€',
            '7' => '{',
            '0' => '}',
            _ => c,
        };
    }
    shifted(
        match c {
            'y' => 'z',
            'z' => 'y',
            '-' => 'ß',
            '=' => '´',
            '[' => 'ü',
            ';' => 'ö',
            '\'' => 'ä',
            _ => c,
        },
        shift,
    )
}

fn spanish(c: char, shift: bool, altgr: bool) -> char {
    if altgr {
        return match c {
            '2' => '@',
            'e' => '€',
            _ => c,
        };
    }
    shifted(
        match c {
            '-' => '¿',
            '=' => '¡',
            ';' => 'ñ',
            '\'' => '´',
            _ => c,
        },
        shift,
    )
}

fn italian(c: char, shift: bool, altgr: bool) -> char {
    if altgr {
        return match c {
            'q' => '@',
            'e' => '€',
            _ => c,
        };
    }
    shifted(
        match c {
            ';' => 'ò',
            '\'' => 'à',
            '[' => 'è',
            ']' => '+',
            _ => c,
        },
        shift,
    )
}

fn portuguese(c: char, shift: bool, altgr: bool) -> char {
    if altgr {
        return match c {
            'q' => '/',
            'e' => '€',
            '2' => '@',
            _ => c,
        };
    }
    shifted(
        match c {
            ';' => 'ç',
            '\'' => 'ã',
            '[' => '´',
            _ => c,
        },
        shift,
    )
}

fn nordic(c: char, shift: bool, altgr: bool) -> char {
    if altgr {
        return match c {
            '2' => '@',
            'e' => '€',
            _ => c,
        };
    }
    shifted(
        match c {
            '[' => 'å',
            ';' => 'ö',
            '\'' => 'ä',
            _ => c,
        },
        shift,
    )
}

fn uk(c: char, shift: bool, altgr: bool) -> char {
    if altgr {
        return match c {
            '4' => '€',
            _ => c,
        };
    }
    shifted(c, shift)
}

fn arabic(c: char, shift: bool, _altgr: bool) -> char {
    if shift {
        return shifted(c, true);
    }
    match c {
        'q' => 'ض',
        'w' => 'ص',
        'e' => 'ث',
        'r' => 'ق',
        't' => 'ف',
        'y' => 'غ',
        'u' => 'ع',
        'i' => 'ه',
        'o' => 'خ',
        'p' => 'ح',
        'a' => 'ش',
        's' => 'س',
        'd' => 'ي',
        'f' => 'ب',
        'g' => 'ل',
        'h' => 'ا',
        'j' => 'ت',
        'k' => 'ن',
        'l' => 'م',
        'z' => 'ئ',
        'x' => 'ء',
        'c' => 'ؤ',
        'v' => 'ر',
        'b' => 'ل',
        'n' => 'ى',
        'm' => 'ة',
        _ => c,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositionResult {
    pub chars: [char; 2],
    pub len: u8,
}

impl CompositionResult {
    pub const fn one(c: char) -> Self {
        Self {
            chars: [c, '\0'],
            len: 1,
        }
    }
    pub const fn two(a: char, b: char) -> Self {
        Self {
            chars: [a, b],
            len: 2,
        }
    }
}

/// Dead-key and compose state shared by Latin IMEs.  The pending mark is
/// bounded to one scalar and commits literally if no canonical pair exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ime {
    pending: Option<char>,
}

impl Ime {
    pub const fn new() -> Self {
        Self { pending: None }
    }
    pub const fn pending(&self) -> Option<char> {
        self.pending
    }
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    pub fn feed(&mut self, c: char) -> Option<CompositionResult> {
        if matches!(c, '`' | '´' | '^' | '~' | '¨') {
            if self.pending == Some(c) {
                self.pending = None;
                return Some(CompositionResult::one(c));
            }
            self.pending = Some(c);
            return None;
        }
        let Some(mark) = self.pending.take() else {
            return Some(CompositionResult::one(c));
        };
        Some(match compose(mark, c) {
            Some(composed) => CompositionResult::one(composed),
            None => CompositionResult::two(mark, c),
        })
    }
}

fn compose(mark: char, c: char) -> Option<char> {
    match (mark, c) {
        ('´', 'a') => Some('á'),
        ('´', 'e') => Some('é'),
        ('´', 'i') => Some('í'),
        ('´', 'o') => Some('ó'),
        ('´', 'u') => Some('ú'),
        ('`', 'a') => Some('à'),
        ('`', 'e') => Some('è'),
        ('`', 'i') => Some('ì'),
        ('`', 'o') => Some('ò'),
        ('`', 'u') => Some('ù'),
        ('^', 'a') => Some('â'),
        ('^', 'e') => Some('ê'),
        ('^', 'i') => Some('î'),
        ('^', 'o') => Some('ô'),
        ('^', 'u') => Some('û'),
        ('~', 'n') => Some('ñ'),
        ('~', 'a') => Some('ã'),
        ('¨', 'a') => Some('ä'),
        ('¨', 'e') => Some('ë'),
        ('¨', 'i') => Some('ï'),
        ('¨', 'o') => Some('ö'),
        ('¨', 'u') => Some('ü'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layouts_cover_common_locale_keys() {
        assert_eq!(Layout::German.map_set1(0x15, false, false), Some('z'));
        assert_eq!(Layout::French.map_set1(0x10, false, false), Some('a'));
        assert_eq!(Layout::Arabic.map_set1(0x10, false, false), Some('ض'));
        assert_eq!(Layout::from_tag("en-GB").unwrap().tag(), "gb");
    }
    #[test]
    fn dead_key_commits_or_falls_back_transactionally() {
        let mut ime = Ime::new();
        assert_eq!(ime.feed('´'), None);
        assert_eq!(ime.feed('e'), Some(CompositionResult::one('é')));
        assert_eq!(ime.feed('¨'), None);
        assert_eq!(ime.feed('x'), Some(CompositionResult::two('¨', 'x')));
    }
}
