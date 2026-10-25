//! Bounded browser-document substrate.
//!
//! The parser intentionally owns no network, script, or pixel resources. It
//! produces a fixed-capacity borrowed DOM that later layout and policy layers
//! can consume without heap allocation.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidUtf8,
    TooManyNodes,
    TooDeep,
    MalformedTag,
    MismatchedClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node<'a> {
    pub tag: &'a str,
    pub text: Option<&'a str>,
    pub parent: Option<usize>,
}

pub struct Document<'a, const N: usize> {
    nodes: [Option<Node<'a>>; N],
    len: usize,
}

impl<'a, const N: usize> Document<'a, N> {
    pub const fn new() -> Self {
        Self {
            nodes: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn node(&self, index: usize) -> Option<Node<'a>> {
        self.nodes.get(index).copied().flatten()
    }

    fn push(&mut self, node: Node<'a>) -> Result<usize, ParseError> {
        if self.len == N {
            return Err(ParseError::TooManyNodes);
        }
        let index = self.len;
        self.nodes[index] = Some(node);
        self.len += 1;
        Ok(index)
    }
}

impl<'a, const N: usize> Default for Document<'a, N> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_html<'a, const N: usize>(input: &'a [u8]) -> Result<Document<'a, N>, ParseError> {
    let source = core::str::from_utf8(input).map_err(|_| ParseError::InvalidUtf8)?;
    let mut document = Document::new();
    let mut stack = [0usize; 16];
    let mut depth = 0usize;
    let mut cursor = 0usize;
    while cursor < source.len() {
        let rest = &source[cursor..];
        if let Some(open) = rest.find('<') {
            if open > 0 {
                let text = &rest[..open];
                if !text.trim_ascii().is_empty() {
                    if let Some(parent) = depth
                        .checked_sub(1)
                        .and_then(|index| stack.get(index).copied())
                    {
                        if let Some(node) = &mut document.nodes[parent] {
                            node.text = Some(text);
                        }
                    }
                }
            }
            let tag_start = cursor + open + 1;
            let tag_end = source[tag_start..]
                .find('>')
                .map(|offset| tag_start + offset)
                .ok_or(ParseError::MalformedTag)?;
            let mut tag = &source[tag_start..tag_end];
            let closing = tag.starts_with('/');
            if closing {
                tag = &tag[1..];
            }
            let self_closing = tag.ends_with('/');
            if self_closing {
                tag = tag[..tag.len() - 1].trim_ascii();
            }
            let name_end = tag
                .find(|character: char| character.is_ascii_whitespace())
                .unwrap_or(tag.len());
            let name = &tag[..name_end];
            if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                return Err(ParseError::MalformedTag);
            }
            if closing {
                if depth == 0 || document.node(stack[depth - 1]).map(|node| node.tag) != Some(name)
                {
                    return Err(ParseError::MismatchedClose);
                }
                depth -= 1;
            } else {
                let parent = depth
                    .checked_sub(1)
                    .and_then(|index| stack.get(index).copied());
                let index = document.push(Node {
                    tag: name,
                    text: None,
                    parent,
                })?;
                if !self_closing {
                    if depth == stack.len() {
                        return Err(ParseError::TooDeep);
                    }
                    stack[depth] = index;
                    depth += 1;
                }
            }
            cursor = tag_end + 1;
        } else {
            let text = rest;
            if !text.trim_ascii().is_empty() {
                if let Some(parent) = depth
                    .checked_sub(1)
                    .and_then(|index| stack.get(index).copied())
                {
                    if let Some(node) = &mut document.nodes[parent] {
                        node.text = Some(text);
                    }
                }
            }
            cursor = source.len();
        }
    }
    if depth != 0 {
        return Err(ParseError::MismatchedClose);
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::{ParseError, parse_html};

    #[test]
    fn parses_borrowed_nested_document_with_text() {
        let document = parse_html::<4>(b"<main><p>Hello</p><br/></main>").unwrap();
        assert_eq!(document.len(), 3);
        assert_eq!(document.node(0).unwrap().tag, "main");
        assert_eq!(document.node(1).unwrap().text, Some("Hello"));
        assert_eq!(document.node(2).unwrap().parent, Some(0));
    }

    #[test]
    fn rejects_malformed_markup_and_bounds_nodes() {
        assert!(matches!(
            parse_html::<4>(b"<main><p></main>"),
            Err(ParseError::MismatchedClose)
        ));
        assert!(matches!(
            parse_html::<1>(b"<main><p></p></main>"),
            Err(ParseError::TooManyNodes)
        ));
        assert!(matches!(
            parse_html::<4>(b"<main"),
            Err(ParseError::MalformedTag)
        ));
    }
}
