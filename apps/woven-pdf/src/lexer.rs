//! Bounded PDF lexical scanner: infrastructure for object/xref parsing.
//! No allocations, decoding, object resolution or page rendering here.
use core::str;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Integer(i64),
    Real(&'a [u8]),
    Name(&'a [u8]),
    LiteralString(&'a [u8]),
    HexString(&'a [u8]),
    Keyword(&'a [u8]),
    DictionaryStart,
    DictionaryEnd,
    ArrayStart,
    ArrayEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexError {
    UnterminatedString,
    UnterminatedHex,
    NestingLimit,
    InvalidNumber,
    TokenTooLong,
}

pub struct Lexer<'a> {
    input: &'a [u8],
    offset: usize,
}

const MAX_TOKEN: usize = 65536;
const MAX_NESTING: usize = 32;

fn whitespace(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}
fn delimiter(b: u8) -> bool {
    whitespace(b) || matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}
fn number_like(s: &[u8]) -> bool {
    !s.is_empty() && matches!(s[0], b'+' | b'-' | b'.' | b'0'..=b'9')
}
fn valid_real(s: &[u8]) -> bool {
    let mut i = 0;
    if matches!(s.first(), Some(b'+' | b'-')) { i += 1; }
    let mut digits = 0usize;
    let mut points = 0usize;
    for &b in &s[i..] {
        if b.is_ascii_digit() { digits += 1; }
        else if b == b'.' { points += 1; }
        else { return false; }
    }
    digits > 0 && points <= 1
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    pub fn position(&self) -> usize { self.offset }

    fn skip_space(&mut self) {
        while self.offset < self.input.len() {
            if whitespace(self.input[self.offset]) {
                self.offset += 1;
            } else if self.input[self.offset] == b'%' {
                while self.offset < self.input.len()
                    && !matches!(self.input[self.offset], b'\n' | b'\r')
                {
                    self.offset += 1;
                }
            } else {
                break;
            }
        }
    }

    fn scan_regular(&mut self, start: usize) -> Result<&'a [u8], LexError> {
        while self.offset < self.input.len() && !delimiter(self.input[self.offset]) {
            self.offset += 1;
            if self.offset - start > MAX_TOKEN { return Err(LexError::TokenTooLong); }
        }
        Ok(&self.input[start..self.offset])
    }

    pub fn next_token(&mut self) -> Result<Option<Token<'a>>, LexError> {
        self.skip_space();
        let start = self.offset;
        let Some(&b) = self.input.get(start) else { return Ok(None); };
        self.offset += 1;
        let token = match b {
            b'[' => Token::ArrayStart,
            b']' => Token::ArrayEnd,
            b'<' if self.input.get(self.offset) == Some(&b'<') => {
                self.offset += 1;
                Token::DictionaryStart
            }
            b'>' if self.input.get(self.offset) == Some(&b'>') => {
                self.offset += 1;
                Token::DictionaryEnd
            }
            b'<' => {
                let begin = self.offset;
                while self.offset < self.input.len() && self.input[self.offset] != b'>' {
                    if self.offset - begin > MAX_TOKEN { return Err(LexError::TokenTooLong); }
                    self.offset += 1;
                }
                if self.offset == self.input.len() { return Err(LexError::UnterminatedHex); }
                let bytes = &self.input[begin..self.offset];
                self.offset += 1;
                Token::HexString(bytes)
            }
            b'(' => {
                let begin = self.offset;
                let mut nesting = 1usize;
                while self.offset < self.input.len() {
                    let c = self.input[self.offset];
                    self.offset += 1;
                    if self.offset - begin > MAX_TOKEN { return Err(LexError::TokenTooLong); }
                    if c == b'\\' {
                        // Escape consumes the following raw character; escaped newlines
                        // and octal strings are decoded by a later semantic pass.
                        if self.offset < self.input.len() { self.offset += 1; }
                    } else if c == b'(' {
                        nesting += 1;
                        if nesting > MAX_NESTING { return Err(LexError::NestingLimit); }
                    } else if c == b')' {
                        nesting -= 1;
                        if nesting == 0 {
                            return Ok(Some(Token::LiteralString(
                                &self.input[begin..self.offset - 1]
                            )));
                        }
                    }
                }
                return Err(LexError::UnterminatedString);
            }
            b'/' => {
                let name_start = self.offset;
                Token::Name(self.scan_regular(name_start)?)
            }
            _ => {
                self.offset = start;
                let bytes = self.scan_regular(start)?;
                // Unexpected PDF punctuation must not cause a non-advancing loop.
                if bytes.is_empty() {
                    self.offset += 1;
                    Token::Keyword(&self.input[start..self.offset])
                } else if number_like(bytes) {
                    if !valid_real(bytes) { return Err(LexError::InvalidNumber); }
                    if !bytes.contains(&b'.') {
                        if let Ok(text) = str::from_utf8(bytes) {
                            if let Ok(n) = text.parse::<i64>() {
                                return Ok(Some(Token::Integer(n)));
                            }
                        }
                        return Err(LexError::InvalidNumber);
                    }
                    Token::Real(bytes)
                } else {
                    Token::Keyword(bytes)
                }
            }
        };
        Ok(Some(token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_pdf_objects() {
        let mut lex = Lexer::new(b"% comment\r\n2 0 obj << /Type /Page /Count 1 >> [12 -3.50]");
        assert_eq!(lex.next_token(), Ok(Some(Token::Integer(2))));
        assert_eq!(lex.next_token(), Ok(Some(Token::Integer(0))));
        assert_eq!(lex.next_token(), Ok(Some(Token::Keyword(b"obj"))));
        assert_eq!(lex.next_token(), Ok(Some(Token::DictionaryStart)));
        assert_eq!(lex.next_token(), Ok(Some(Token::Name(b"Type"))));
        assert_eq!(lex.next_token(), Ok(Some(Token::Name(b"Page"))));
        assert_eq!(lex.next_token(), Ok(Some(Token::Name(b"Count"))));
        assert_eq!(lex.next_token(), Ok(Some(Token::Integer(1))));
        assert_eq!(lex.next_token(), Ok(Some(Token::DictionaryEnd)));
        assert_eq!(lex.next_token(), Ok(Some(Token::ArrayStart)));
        assert_eq!(lex.next_token(), Ok(Some(Token::Integer(12))));
        assert_eq!(lex.next_token(), Ok(Some(Token::Real(b"-3.50"))));
        assert_eq!(lex.next_token(), Ok(Some(Token::ArrayEnd)));
        assert_eq!(lex.next_token(), Ok(None));
    }

    #[test]
    fn retains_nested_and_escaped_literal_raw_bytes() {
        let mut lex = Lexer::new(br"(hello (world) \) end)");
        assert_eq!(lex.next_token(), Ok(Some(Token::LiteralString(br"hello (world) \) end"))));
    }

    #[test]
    fn handles_hex_and_name() {
        let mut lex = Lexer::new(b"/A#20B <4869>");
        assert_eq!(lex.next_token(), Ok(Some(Token::Name(b"A#20B"))));
        assert_eq!(lex.next_token(), Ok(Some(Token::HexString(b"4869"))));
    }

    #[test]
    fn rejects_unterminated_or_invalid() {
        assert_eq!(Lexer::new(b"(broken").next_token(), Err(LexError::UnterminatedString));
        assert_eq!(Lexer::new(b"<A1").next_token(), Err(LexError::UnterminatedHex));
        assert_eq!(Lexer::new(b"12a").next_token(), Err(LexError::InvalidNumber));
    }

    #[test]
    fn always_advances_on_stray_delimiters() {
        let mut lexer = Lexer::new(b"){}");
        for i in 1..=3 {
            assert!(lexer.next_token().unwrap().is_some());
            assert_eq!(lexer.position(), i);
        }
        assert_eq!(lexer.next_token(), Ok(None));
    }
}
