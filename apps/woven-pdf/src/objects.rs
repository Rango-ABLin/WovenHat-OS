//! Indirect object access through validated classic cross-reference entries.
//! This deliberately excludes object streams, updates and compressed xrefs.
use crate::lexer::{LexError, Lexer, Token};
use crate::xref::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectError {
    NotFound,
    BadOffset,
    InvalidHeader,
    Unterminated,
    Lex(LexError),
    Unsupported,
}

impl From<LexError> for ObjectError {
    fn from(value: LexError) -> Self { Self::Lex(value) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    pub number: u32,
    pub generation: u16,
}

pub struct Resolver<'a> {
    bytes: &'a [u8],
    entries: &'a [Entry],
}

fn next<'a>(lexer: &mut Lexer<'a>) -> Result<Token<'a>, ObjectError> {
    lexer.next_token()?.ok_or(ObjectError::Unterminated)
}

fn unsigned(token: Token<'_>) -> Result<u32, ObjectError> {
    match token {
        Token::Integer(n) => u32::try_from(n).map_err(|_| ObjectError::InvalidHeader),
        _ => Err(ObjectError::InvalidHeader),
    }
}

impl<'a> Resolver<'a> {
    pub fn new(bytes: &'a [u8], entries: &'a [Entry]) -> Self {
        Self { bytes, entries }
    }

    pub fn resolve(&self, reference: Reference) -> Result<&'a [u8], ObjectError> {
        let entry = self.entries.iter().find(|e| e.in_use
            && e.object == reference.number && e.generation == reference.generation)
            .ok_or(ObjectError::NotFound)?;
        let slice = self.bytes.get(entry.offset..).ok_or(ObjectError::BadOffset)?;
        let mut lexer = Lexer::new(slice);
        if unsigned(next(&mut lexer)?)? != reference.number
            || unsigned(next(&mut lexer)?)? != u32::from(reference.generation)
            || next(&mut lexer)? != Token::Keyword(b"obj")
        {
            return Err(ObjectError::InvalidHeader);
        }
        let start = lexer.position();
        // Parsing tokens, not scanning raw substrings, prevents an 'endobj'
        // inside a literal or hex string from prematurely terminating an object.
        // Streams need length-based skipping and are intentionally rejected.
        loop {
            let at = lexer.position();
            match lexer.next_token()? {
                Some(Token::Keyword(b"endobj")) => return Ok(&slice[start..at]),
                Some(Token::Keyword(b"stream")) => return Err(ObjectError::Unsupported),
                Some(_) => {}
                None => return Err(ObjectError::Unterminated),
            }
        }
    }

    /// Resolve a top-level indirect reference stored under a named dictionary key.
    /// Nested dictionaries and arrays are traversed without mistaking their keys
    /// for top-level members.
    pub fn dictionary_reference(
        &self,
        reference: Reference,
        key: &[u8],
    ) -> Result<Reference, ObjectError> {
        let body = self.resolve(reference)?;
        let mut lexer = Lexer::new(body);
        if next(&mut lexer)? != Token::DictionaryStart {
            return Err(ObjectError::InvalidHeader);
        }
        let mut depth = 1usize;
        while let Some(token) = lexer.next_token()? {
            match token {
                Token::DictionaryStart | Token::ArrayStart => {
                    depth = depth.checked_add(1).ok_or(ObjectError::Unsupported)?;
                    if depth > 32 { return Err(ObjectError::Unsupported); }
                }
                Token::DictionaryEnd | Token::ArrayEnd => {
                    depth = depth.checked_sub(1).ok_or(ObjectError::InvalidHeader)?;
                    if depth == 0 { break; }
                }
                Token::Name(name) if depth == 1 && name == key => {
                    let number = unsigned(next(&mut lexer)?)?;
                    let generation = unsigned(next(&mut lexer)?)?;
                    if next(&mut lexer)? != Token::Keyword(b"R") {
                        return Err(ObjectError::InvalidHeader);
                    }
                    return Ok(Reference {
                        number,
                        generation: u16::try_from(generation)
                            .map_err(|_| ObjectError::InvalidHeader)?,
                    });
                }
                _ => {}
            }
        }
        Err(ObjectError::NotFound)
    }

    pub fn dictionary_name(
        &self,
        reference: Reference,
        key: &[u8],
    ) -> Result<&'a [u8], ObjectError> {
        let body = self.resolve(reference)?;
        let mut lexer = Lexer::new(body);
        if next(&mut lexer)? != Token::DictionaryStart {
            return Err(ObjectError::InvalidHeader);
        }
        let mut depth = 1usize;
        while let Some(token) = lexer.next_token()? {
            match token {
                Token::DictionaryStart | Token::ArrayStart => {
                    depth = depth.checked_add(1).ok_or(ObjectError::Unsupported)?;
                    if depth > 32 { return Err(ObjectError::Unsupported); }
                }
                Token::DictionaryEnd | Token::ArrayEnd => {
                    depth = depth.checked_sub(1).ok_or(ObjectError::InvalidHeader)?;
                    if depth == 0 { break; }
                }
                Token::Name(name) if depth == 1 && name == key => {
                    return match next(&mut lexer)? {
                        Token::Name(value) => Ok(value),
                        _ => Err(ObjectError::InvalidHeader),
                    };
                }
                _ => {}
            }
        }
        Err(ObjectError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &[u8] = b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra << /Pages 99 0 R >> >>\nendobj\n2 0 obj\n<< /Type /Pages >>\nendobj\n";

    fn resolver() -> Resolver<'static> {
        Resolver::new(BODY, &[
            Entry { object: 1, generation: 0, offset: 0, in_use: true },
            Entry { object: 2, generation: 0, offset: 78, in_use: true },
        ])
    }

    #[test]
    fn resolves_catalog_reference() {
        let r = resolver();
        let root = Reference { number: 1, generation: 0 };
        assert_eq!(r.dictionary_name(root, b"Type"), Ok(&b"Catalog"[..]));
        assert_eq!(r.dictionary_reference(root, b"Pages"),
            Ok(Reference { number: 2, generation: 0 }));
    }

    #[test]
    fn rejects_missing_object_and_bad_header() {
        let r = resolver();
        assert!(matches!(r.resolve(Reference { number: 7, generation: 0 }),
            Err(ObjectError::NotFound)));
        let entries = [Entry { object: 1, generation: 0, offset: 3, in_use: true }];
        let corrupt = Resolver::new(BODY, &entries);
        assert!(corrupt.resolve(Reference { number: 1, generation: 0 }).is_err());
    }

    #[test]
    fn embedded_endobj_string_is_not_terminator() {
        let data = b"1 0 obj (endobj) /Title (ok) endobj";
        let entries = [Entry { object: 1, generation: 0, offset: 0, in_use: true }];
        let resolved = Resolver::new(data, &entries)
            .resolve(Reference { number: 1, generation: 0 }).unwrap();
        assert!(resolved.ends_with(b"(ok) "));
    }
}
