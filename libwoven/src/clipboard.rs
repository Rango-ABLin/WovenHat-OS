//! Bounded clipboard model for GUI applications.
//!
//! This is deliberately a policy-free userspace buffer. A compositor or
//! clipboard service owns transport and permission checks; applications use
//! this type to validate and retain one UTF-8 text payload without allocation.

pub const MAX_TEXT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    TooLong,
    InvalidUtf8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextClipboard {
    bytes: [u8; MAX_TEXT_BYTES],
    len: usize,
    generation: u64,
}

impl Default for TextClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl TextClipboard {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_TEXT_BYTES],
            len: 0,
            generation: 0,
        }
    }

    pub const fn is_empty(self) -> bool { self.len == 0 }
    pub const fn len(self) -> usize { self.len }
    pub const fn generation(self) -> u64 { self.generation }

    pub fn set(&mut self, text: &str) -> Result<(), Error> {
        if text.is_empty() {
            return Err(Error::Empty);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(Error::TooLong);
        }
        self.bytes[..text.len()].copy_from_slice(text.as_bytes());
        self.bytes[text.len()..].fill(0);
        self.len = text.len();
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    pub fn set_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let text = core::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        self.set(text)
    }

    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }

    pub fn clear(&mut self) {
        if self.len != 0 {
            self.bytes[..self.len].fill(0);
            self.len = 0;
            self.generation = self.generation.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_bounded_and_generation_changes_only_on_mutation() {
        let mut clipboard = TextClipboard::new();
        assert!(clipboard.is_empty());
        assert_eq!(clipboard.generation(), 0);
        assert_eq!(clipboard.set("hello"), Ok(()));
        assert_eq!(clipboard.text(), "hello");
        assert_eq!(clipboard.generation(), 1);
        assert_eq!(clipboard.set(""), Err(Error::Empty));
        assert_eq!(clipboard.generation(), 1);
        assert_eq!(clipboard.set_bytes(&[0xff]), Err(Error::InvalidUtf8));
        assert_eq!(clipboard.generation(), 1);
        clipboard.clear();
        assert!(clipboard.is_empty());
        assert_eq!(clipboard.generation(), 2);
    }

    #[test]
    fn overlong_text_is_rejected_without_partial_write() {
        let mut clipboard = TextClipboard::new();
        clipboard.set("keep").unwrap();
        let too_long = [b'x'; MAX_TEXT_BYTES + 1];
        assert_eq!(clipboard.set_bytes(&too_long), Err(Error::TooLong));
        assert_eq!(clipboard.text(), "keep");
    }
}
