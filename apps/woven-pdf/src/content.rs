//! Strict bounded PDF content-stream operator reader.
//! This does not render; it yields a small operator subset to a renderer.
//! Unknown operators must not be silently considered supported.
use crate::lexer::{Lexer, Token};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation<'a> {
    Save,
    Restore,
    BeginText,
    EndText,
    SetFont { name: &'a [u8], size: f32 },
    MoveText { x: f32, y: f32 },
    ShowText(&'a [u8]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentError {
    Malformed,
    Unsupported,
    TooManyOperands,
    Lexer,
}

fn value(token: Token<'_>) -> Result<f32, ContentError> {
    match token {
        Token::Integer(v) => Ok(v as f32),
        Token::Real(raw) => core::str::from_utf8(raw)
            .map_err(|_| ContentError::Malformed)?
            .parse::<f32>()
            .map_err(|_| ContentError::Malformed),
        _ => Err(ContentError::Malformed),
    }
}

/// Parse a conservative subset of PDF drawing instructions. No clipping,
/// graphics matrices, font encodings, or arbitrary operator execution.
pub fn interpret<'a>(
    bytes: &'a [u8],
    mut emit: impl FnMut(Operation<'a>) -> Result<(), ContentError>,
) -> Result<(), ContentError> {
    let mut scanner = Lexer::new(bytes);
    let mut operands = [None; 8];
    let mut count = 0usize;
    let mut text = false;
    let mut state_depth = 0usize;
    while let Some(token) = scanner.next_token().map_err(|_| ContentError::Lexer)? {
        if let Token::Keyword(op) = token {
            let action = match op {
                b"q" if count == 0 => {
                    state_depth += 1;
                    if state_depth > 16 { return Err(ContentError::Unsupported); }
                    Some(Operation::Save)
                }
                b"Q" if count == 0 && state_depth > 0 => {
                    state_depth -= 1;
                    Some(Operation::Restore)
                }
                b"BT" if count == 0 && !text => { text = true; Some(Operation::BeginText) }
                b"ET" if count == 0 && text => { text = false; Some(Operation::EndText) }
                b"Tf" if count == 2 && text => {
                    let name = match operands[0] {
                        Some(Token::Name(name)) => name,
                        _ => return Err(ContentError::Malformed),
                    };
                    let size = value(operands[1].ok_or(ContentError::Malformed)?)?;
                    Some(Operation::SetFont { name, size })
                }
                b"Td" if count == 2 && text => Some(Operation::MoveText {
                    x: value(operands[0].ok_or(ContentError::Malformed)?)?,
                    y: value(operands[1].ok_or(ContentError::Malformed)?)?,
                }),
                b"Tj" if count == 1 && text => {
                    let content = match operands[0] {
                        Some(Token::LiteralString(content)) => content,
                        _ => return Err(ContentError::Unsupported),
                    };
                    Some(Operation::ShowText(content))
                }
                _ => return Err(ContentError::Unsupported),
            };
            if let Some(action) = action { emit(action)?; }
            count = 0;
        } else {
            if count == operands.len() { return Err(ContentError::TooManyOperands); }
            operands[count] = Some(token);
            count += 1;
        }
    }
    if count != 0 || text || state_depth != 0 { return Err(ContentError::Malformed); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_simple_text_instructions() {
        let mut seen = 0;
        let result = interpret(b"q BT /F1 12 Tf 10 20 Td (Hello) Tj ET Q", |op| {
            if let Operation::ShowText(b"Hello") = op { seen += 1; }
            Ok(())
        });
        assert_eq!(result, Ok(()));
        assert_eq!(seen, 1);
    }
    #[test]
    fn refuses_unimplemented_graphics_and_unbalanced_state() {
        assert_eq!(interpret(b"10 10 m", |_| Ok(())), Err(ContentError::Unsupported));
        assert_eq!(interpret(b"BT (hello) Tj", |_| Ok(())), Err(ContentError::Malformed));
        assert_eq!(interpret(b"Q", |_| Ok(())), Err(ContentError::Unsupported));
    }
}
