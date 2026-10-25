//! Bounded classic-PDF page-tree traversal.
//! Requires already-validated indirect xref objects. Does not render pages.
use crate::lexer::{Lexer, Token};
use crate::objects::{ObjectError, Reference, Resolver};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageError {
    Object(ObjectError),
    Malformed,
    Depth,
    Capacity,
    Cycle,
}
impl From<ObjectError> for PageError {
    fn from(e: ObjectError) -> Self {
        Self::Object(e)
    }
}
const MAX_DEPTH: usize = 24;

/// Discover all leaf /Page references in logical page order. The /Count
/// integer is not trusted as a substitute for walking actual /Kids.
pub fn collect_pages(
    resolver: &Resolver<'_>,
    pages_root: Reference,
    output: &mut [Reference],
) -> Result<usize, PageError> {
    fn walk(
        resolver: &Resolver<'_>,
        node: Reference,
        output: &mut [Reference],
        count: &mut usize,
        stack: &mut [Reference; MAX_DEPTH],
        depth: usize,
    ) -> Result<(), PageError> {
        if depth == MAX_DEPTH {
            return Err(PageError::Depth);
        }
        if stack[..depth].contains(&node) {
            return Err(PageError::Cycle);
        }
        stack[depth] = node;
        let typ = resolver.dictionary_name(node, b"Type")?;
        if typ == b"Page" {
            if *count >= output.len() {
                return Err(PageError::Capacity);
            }
            output[*count] = node;
            *count += 1;
            return Ok(());
        }
        if typ != b"Pages" {
            return Err(PageError::Malformed);
        }
        let body = resolver.resolve(node)?;
        let mut lex = Lexer::new(body);
        if lex.next_token().map_err(ObjectError::from)? != Some(Token::DictionaryStart) {
            return Err(PageError::Malformed);
        }
        let mut nesting = 1usize;
        let mut kids = None;
        while let Some(tok) = lex.next_token().map_err(ObjectError::from)? {
            match tok {
                Token::DictionaryStart | Token::ArrayStart => {
                    nesting += 1;
                    if nesting > MAX_DEPTH {
                        return Err(PageError::Depth);
                    }
                }
                Token::DictionaryEnd | Token::ArrayEnd => {
                    nesting = nesting.checked_sub(1).ok_or(PageError::Malformed)?;
                    if nesting == 0 {
                        break;
                    }
                }
                Token::Name(name) if nesting == 1 && name == b"Kids" => {
                    if lex.next_token().map_err(ObjectError::from)? != Some(Token::ArrayStart) {
                        return Err(PageError::Malformed);
                    }
                    kids = Some(lex.position());
                    break;
                }
                _ => {}
            }
        }
        let Some(start) = kids else {
            return Err(PageError::Malformed);
        };
        let mut children = Lexer::new(&body[start..]);
        loop {
            let next = children.next_token().map_err(ObjectError::from)?;
            if next == Some(Token::ArrayEnd) {
                break;
            }
            let Some(Token::Integer(num)) = next else {
                return Err(PageError::Malformed);
            };
            let Some(Token::Integer(generation)) =
                children.next_token().map_err(ObjectError::from)?
            else {
                return Err(PageError::Malformed);
            };
            if children.next_token().map_err(ObjectError::from)? != Some(Token::Keyword(b"R")) {
                return Err(PageError::Malformed);
            }
            let number = u32::try_from(num).map_err(|_| PageError::Malformed)?;
            let generation = u16::try_from(generation).map_err(|_| PageError::Malformed)?;
            walk(
                resolver,
                Reference { number, generation },
                output,
                count,
                stack,
                depth + 1,
            )?;
        }
        Ok(())
    }
    let mut stack = [Reference {
        number: 0,
        generation: 0,
    }; MAX_DEPTH];
    let mut count = 0;
    walk(resolver, pages_root, output, &mut count, &mut stack, 0)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xref::Entry;

    #[test]
    fn traverses_two_page_tree() {
        let obj1 = b"1 0 obj << /Type /Pages /Kids [2 0 R 3 0 R] /Count 2 >> endobj ";
        let obj2 = b"2 0 obj << /Type /Page /Parent 1 0 R >> endobj ";
        let obj3 = b"3 0 obj << /Type /Page /Parent 1 0 R >> endobj";
        let mut data = std::vec::Vec::new();
        data.extend_from_slice(obj1);
        data.extend_from_slice(obj2);
        data.extend_from_slice(obj3);
        let entries = [
            Entry {
                object: 1,
                generation: 0,
                offset: 0,
                in_use: true,
            },
            Entry {
                object: 2,
                generation: 0,
                offset: obj1.len(),
                in_use: true,
            },
            Entry {
                object: 3,
                generation: 0,
                offset: obj1.len() + obj2.len(),
                in_use: true,
            },
        ];
        let resolver = Resolver::new(&data, &entries);
        let mut pages = [Reference {
            number: 0,
            generation: 0,
        }; 4];
        assert_eq!(
            collect_pages(
                &resolver,
                Reference {
                    number: 1,
                    generation: 0
                },
                &mut pages
            ),
            Ok(2)
        );
        assert_eq!(pages[0].number, 2);
        assert_eq!(pages[1].number, 3);
    }

    #[test]
    fn detects_cycle() {
        let data = b"1 0 obj << /Type /Pages /Kids [1 0 R] /Count 1 >> endobj";
        let entries = [Entry {
            object: 1,
            generation: 0,
            offset: 0,
            in_use: true,
        }];
        let resolver = Resolver::new(data, &entries);
        let mut pages = [Reference {
            number: 0,
            generation: 0,
        }; 2];
        assert_eq!(
            collect_pages(
                &resolver,
                Reference {
                    number: 1,
                    generation: 0
                },
                &mut pages
            ),
            Err(PageError::Cycle)
        );
    }
}
