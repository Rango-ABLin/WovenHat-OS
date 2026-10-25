//! Length-delimited extraction of unfiltered PDF stream data.
//! This never searches binary content for "endstream" to determine length.
//! Indirect /Length references and compressed streams remain unsupported.
use crate::lexer::{Lexer, Token};
use crate::objects::Reference;
use crate::xref::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamError {
    NotFound,
    InvalidHeader,
    InvalidDictionary,
    InvalidLength,
    UnsupportedFilter,
    Truncated,
    InvalidTerminator,
}

fn next<'a>(scanner: &mut Lexer<'a>) -> Result<Token<'a>, StreamError> {
    scanner.next_token().map_err(|_| StreamError::InvalidDictionary)?
        .ok_or(StreamError::Truncated)
}

fn number(t: Token<'_>) -> Result<u32, StreamError> {
    if let Token::Integer(n) = t {
        u32::try_from(n).map_err(|_| StreamError::InvalidHeader)
    } else { Err(StreamError::InvalidHeader) }
}

/// Returns a borrowed view of an unfiltered stream attached to the specified
/// indirect object. Dictionary /Length must be a direct nonnegative integer.
pub fn raw_stream<'a>(
    input: &'a [u8],
    entries: &[Entry],
    target: Reference,
) -> Result<&'a [u8], StreamError> {
    let entry = entries.iter().find(|e| e.in_use
        && e.object == target.number && e.generation == target.generation)
        .ok_or(StreamError::NotFound)?;
    let body = input.get(entry.offset..).ok_or(StreamError::InvalidHeader)?;
    let mut lex = Lexer::new(body);
    if number(next(&mut lex)?)? != target.number
        || number(next(&mut lex)?)? != u32::from(target.generation)
        || next(&mut lex)? != Token::Keyword(b"obj") {
        return Err(StreamError::InvalidHeader);
    }
    if next(&mut lex)? != Token::DictionaryStart {
        return Err(StreamError::InvalidDictionary);
    }
    let mut depth = 1usize;
    let mut length = None;
    let mut filter = false;
    while depth != 0 {
        match next(&mut lex)? {
            Token::DictionaryStart | Token::ArrayStart => {
                depth += 1;
                if depth > 32 { return Err(StreamError::InvalidDictionary); }
            }
            Token::DictionaryEnd | Token::ArrayEnd => {
                depth -= 1;
            }
            Token::Name(name) if depth == 1 && name == b"Length" => {
                let value = number(next(&mut lex)?)?;
                length = Some(value as usize);
            }
            Token::Name(name) if depth == 1 && (name == b"Filter" || name == b"F") => {
                filter = true;
            }
            _ => {}
        }
    }
    if filter { return Err(StreamError::UnsupportedFilter); }
    let length = length.ok_or(StreamError::InvalidLength)?;
    if next(&mut lex)? != Token::Keyword(b"stream") {
        return Err(StreamError::InvalidDictionary);
    }
    let at = lex.position();
    // The lexical scanner leaves the newline following "stream" untouched.
    let start = match body.get(at..at.saturating_add(2)) {
        Some(b"\r\n") => at + 2,
        _ if matches!(body.get(at), Some(b'\r' | b'\n')) => at + 1,
        _ => return Err(StreamError::InvalidDictionary),
    };
    let end = start.checked_add(length).ok_or(StreamError::InvalidLength)?;
    let bytes = body.get(start..end).ok_or(StreamError::Truncated)?;
    let suffix = body.get(end..).ok_or(StreamError::Truncated)?;
    let suffix = if suffix.starts_with(b"\r\n") { &suffix[2..] }
        else if suffix.starts_with(b"\n") || suffix.starts_with(b"\r") { &suffix[1..] }
        else { suffix };
    if !suffix.starts_with(b"endstream") { return Err(StreamError::InvalidTerminator); }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PDF: &[u8] = b"1 0 obj\n<< /Length 13 >>\nstream\nhelloendobj!\nendstream\nendobj";
    const ENTRY: [Entry; 1] = [Entry { object: 1, generation: 0, offset: 0, in_use: true }];
    const REF: Reference = Reference { number: 1, generation: 0 };

    #[test]
    fn uses_length_not_embedded_delimiters() {
        assert_eq!(raw_stream(PDF, &ENTRY, REF), Ok(&b"helloendobj!"[..]));
    }
    #[test]
    fn rejects_missing_filter_or_wrong_object() {
        assert_eq!(raw_stream(PDF, &ENTRY, Reference { number: 2, generation: 0 }),
            Err(StreamError::NotFound));
        const COMP: &[u8] = b"1 0 obj << /Length 3 /Filter /FlateDecode >>\nstream\nabc\nendstream\nendobj";
        assert_eq!(raw_stream(COMP, &ENTRY, REF), Err(StreamError::UnsupportedFilter));
    }
    #[test]
    fn rejects_incorrect_lengths() {
        const BAD: &[u8] = b"1 0 obj << /Length 2 >>\nstream\nabc\nendstream\nendobj";
        assert_eq!(raw_stream(BAD, &ENTRY, REF), Err(StreamError::InvalidTerminator));
    }
}
