//! Resolve a classic PDF trailer /Root, page tree and /Contents reference.
//! Only direct page /Contents references are supported; arrays and inherited
//! resources are explicitly outside this narrow initial path.
use crate::lexer::{Lexer, Token};
use crate::objects::{ObjectError, Reference, Resolver};
use crate::pages::{collect_pages, PageError};
use crate::xref::{startxref, Entry, XrefError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentError {
    Xref(XrefError),
    Object(ObjectError),
    Pages(PageError),
    Malformed,
    NotFound,
}
impl From<ObjectError> for DocumentError {
    fn from(value: ObjectError) -> Self { Self::Object(value) }
}
impl From<XrefError> for DocumentError {
    fn from(value: XrefError) -> Self { Self::Xref(value) }
}
impl From<PageError> for DocumentError {
    fn from(value: PageError) -> Self { Self::Pages(value) }
}
fn number(token: Option<Token<'_>>) -> Result<u32, DocumentError> {
    match token {
        Some(Token::Integer(x)) => u32::try_from(x).map_err(|_| DocumentError::Malformed),
        _ => Err(DocumentError::Malformed),
    }
}
/// Parse the Root reference from the classic trailer dictionary after xref.
pub fn trailer_root(data: &[u8]) -> Result<Reference, DocumentError> {
    let offset = startxref(data)?;
    let mut pos = offset + 4;
    // Bounded search over classic xref entries only, stopping at the
    // first "trailer" keyword. The xref reader validates the table separately.
    let mut found = None;
    while pos + 7 <= data.len() {
        if data.get(pos..pos + 7) == Some(b"trailer") {
            found = Some(pos + 7);
            break;
        }
        pos += 1;
    }
    let start = found.ok_or(DocumentError::NotFound)?;
    let mut lex = Lexer::new(&data[start..]);
    if lex.next_token().map_err(|_| DocumentError::Malformed)? != Some(Token::DictionaryStart) {
        return Err(DocumentError::Malformed);
    }
    let mut depth = 1usize;
    while let Some(token) = lex.next_token().map_err(|_| DocumentError::Malformed)? {
        match token {
            Token::DictionaryStart | Token::ArrayStart => {
                depth += 1;
                if depth > 32 { return Err(DocumentError::Malformed); }
            }
            Token::DictionaryEnd | Token::ArrayEnd => {
                depth = depth.checked_sub(1).ok_or(DocumentError::Malformed)?;
                if depth == 0 { break; }
            }
            Token::Name(name) if depth == 1 && name == b"Root" => {
                let object = number(lex.next_token().map_err(|_| DocumentError::Malformed)?)?;
                let generation = u16::try_from(number(
                    lex.next_token().map_err(|_| DocumentError::Malformed)?
                )?).map_err(|_| DocumentError::Malformed)?;
                if lex.next_token().map_err(|_| DocumentError::Malformed)?
                    != Some(Token::Keyword(b"R")) { return Err(DocumentError::Malformed); }
                return Ok(Reference { number: object, generation });
            }
            _ => {}
        }
    }
    Err(DocumentError::NotFound)
}

/// Resolve content for a numbered page (zero-based) via the actual
/// trailer Catalog -> Pages -> leaf Page -> Contents chain.
pub fn page_content(
    data: &[u8],
    entries: &[Entry],
    index: usize,
    page_buffer: &mut [Reference],
) -> Result<(Reference, usize), DocumentError> {
    let resolver = Resolver::new(data, entries);
    let catalog = trailer_root(data)?;
    if resolver.dictionary_name(catalog, b"Type")? != b"Catalog" {
        return Err(DocumentError::Malformed);
    }
    let pages_root = resolver.dictionary_reference(catalog, b"Pages")?;
    let page_count = collect_pages(&resolver, pages_root, page_buffer)?;
    let page = *page_buffer.get(index).filter(|_| index < page_count)
        .ok_or(DocumentError::NotFound)?;
    if resolver.dictionary_name(page, b"Type")? != b"Page" {
        return Err(DocumentError::Malformed);
    }
    let contents = resolver.dictionary_reference(page, b"Contents")?;
    Ok((contents, page_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_contents_of_first_page() {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = [0usize; 5];
        for (id, body) in [
            (1, "<< /Type /Catalog /Pages 2 0 R >>"),
            (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (3, "<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>"),
            (4, "<< /Length 0 >>\nstream\n\nendstream"),
        ] {
            offsets[id] = pdf.len();
            pdf.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
        }
        let xref_at = pdf.len();
        pdf.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
        for off in offsets.iter().skip(1) {
            pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(format!(
            "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
        ).as_bytes());
        let mut entries = [Entry::default(); 8];
        let count = crate::xref::read_table(&pdf, &mut entries).unwrap();
        let mut pages = [Reference { number: 0, generation: 0 }; 4];
        assert_eq!(page_content(&pdf, &entries[..count], 0, &mut pages),
            Ok((Reference { number: 4, generation: 0 }, 1)));
        assert_eq!(page_content(&pdf, &entries[..count], 1, &mut pages),
            Err(DocumentError::NotFound));
    }
}
