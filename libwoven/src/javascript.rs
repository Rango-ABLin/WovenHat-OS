//! Bounded freestanding JavaScript runtime foundation.
//!
//! This module deliberately starts with a deterministic, allocation-free
//! scalar execution core. It is suitable for small UI bindings and policy
//! expressions while the larger ECMAScript object/DOM/async surface remains a
//! separate capability boundary.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Undefined,
    Boolean(bool),
    Integer(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidToken,
    UnexpectedToken,
    InvalidNumber,
    UnknownVariable,
    VariableCapacity,
    DivisionByZero,
    ArithmeticOverflow,
    ScriptTooLong,
    NestingTooDeep,
}

#[derive(Clone, Copy)]
struct Binding<'a> {
    name: &'a str,
    value: Value,
}

pub struct Runtime<'a, const N: usize> {
    bindings: [Option<Binding<'a>>; N],
    len: usize,
}

impl<'a, const N: usize> Runtime<'a, N> {
    pub const fn new() -> Self {
        Self {
            bindings: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.bindings[..self.len]
            .iter()
            .flatten()
            .find(|binding| binding.name == name)
            .map(|binding| binding.value)
    }

    fn assign(&mut self, name: &'a str, value: Value) -> Result<(), Error> {
        if let Some(binding) = self.bindings[..self.len]
            .iter_mut()
            .flatten()
            .find(|binding| binding.name == name)
        {
            binding.value = value;
            return Ok(());
        }
        if self.len == N {
            return Err(Error::VariableCapacity);
        }
        self.bindings[self.len] = Some(Binding { name, value });
        self.len += 1;
        Ok(())
    }
}

impl<'a, const N: usize> Default for Runtime<'a, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token<'a> {
    End,
    Number(i64),
    Identifier(&'a str),
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Bang,
    LParen,
    RParen,
    Semicolon,
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    fn next(&mut self) -> Result<Token<'a>, Error> {
        let bytes = self.source.as_bytes();
        while self.cursor < bytes.len() && bytes[self.cursor].is_ascii_whitespace() {
            self.cursor += 1;
        }
        if self.cursor == bytes.len() {
            return Ok(Token::End);
        }
        if bytes[self.cursor] == b'/' && bytes.get(self.cursor + 1) == Some(&b'/') {
            self.cursor += 2;
            while self.cursor < bytes.len() && bytes[self.cursor] != b'\n' {
                self.cursor += 1;
            }
            return self.next();
        }
        let start = self.cursor;
        let token = match bytes[self.cursor] {
            b'=' if bytes.get(self.cursor + 1) == Some(&b'=') => {
                self.cursor += 1;
                Token::EqualEqual
            }
            b'!' if bytes.get(self.cursor + 1) == Some(&b'=') => {
                self.cursor += 1;
                Token::NotEqual
            }
            b'<' if bytes.get(self.cursor + 1) == Some(&b'=') => {
                self.cursor += 1;
                Token::LessEqual
            }
            b'>' if bytes.get(self.cursor + 1) == Some(&b'=') => {
                self.cursor += 1;
                Token::GreaterEqual
            }
            b'&' if bytes.get(self.cursor + 1) == Some(&b'&') => {
                self.cursor += 1;
                Token::AndAnd
            }
            b'|' if bytes.get(self.cursor + 1) == Some(&b'|') => {
                self.cursor += 1;
                Token::OrOr
            }
            b'+' => Token::Plus,
            b'-' => Token::Minus,
            b'*' => Token::Star,
            b'/' => Token::Slash,
            b'=' => Token::Equal,
            b'<' => Token::Less,
            b'>' => Token::Greater,
            b'!' => Token::Bang,
            b'(' => Token::LParen,
            b')' => Token::RParen,
            b';' => Token::Semicolon,
            b'0'..=b'9' => {
                let mut value = 0i64;
                while self.cursor < bytes.len() && bytes[self.cursor].is_ascii_digit() {
                    value = value
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(i64::from(bytes[self.cursor] - b'0')))
                        .ok_or(Error::InvalidNumber)?;
                    self.cursor += 1;
                }
                return Ok(Token::Number(value));
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' => {
                self.cursor += 1;
                while self.cursor < bytes.len()
                    && (bytes[self.cursor].is_ascii_alphanumeric()
                        || matches!(bytes[self.cursor], b'_' | b'$'))
                {
                    self.cursor += 1;
                }
                return Ok(Token::Identifier(&self.source[start..self.cursor]));
            }
            _ => return Err(Error::InvalidToken),
        };
        self.cursor += 1;
        Ok(token)
    }
}

struct Parser<'source, 'runtime, const N: usize> {
    lexer: Lexer<'source>,
    current: Token<'source>,
    runtime: &'runtime mut Runtime<'source, N>,
}

impl<'source, 'runtime, const N: usize> Parser<'source, 'runtime, N> {
    fn new(
        source: &'source str,
        runtime: &'runtime mut Runtime<'source, N>,
    ) -> Result<Self, Error> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next()?;
        Ok(Self {
            lexer,
            current,
            runtime,
        })
    }

    fn advance(&mut self) -> Result<(), Error> {
        self.current = self.lexer.next()?;
        Ok(())
    }

    fn script(&mut self) -> Result<Value, Error> {
        let mut result = Value::Undefined;
        while self.current != Token::End {
            result = self.statement()?;
            if self.current == Token::Semicolon {
                self.advance()?;
            } else if self.current != Token::End {
                return Err(Error::UnexpectedToken);
            }
        }
        Ok(result)
    }

    fn statement(&mut self) -> Result<Value, Error> {
        if self.current == Token::Identifier("let") {
            self.advance()?;
            let Token::Identifier(name) = self.current else {
                return Err(Error::UnexpectedToken);
            };
            self.advance()?;
            if self.current != Token::Equal {
                return Err(Error::UnexpectedToken);
            }
            self.advance()?;
            let value = self.expression(0, 0)?;
            self.runtime.assign(name, value)?;
            Ok(value)
        } else {
            self.expression(0, 0)
        }
    }

    fn expression(&mut self, depth: usize, minimum_precedence: u8) -> Result<Value, Error> {
        if depth >= 32 {
            return Err(Error::NestingTooDeep);
        }
        let mut left = match self.current {
            Token::Number(value) => {
                self.advance()?;
                Value::Integer(value)
            }
            Token::Identifier(name) => {
                self.advance()?;
                match name {
                    "true" => Value::Boolean(true),
                    "false" => Value::Boolean(false),
                    _ => self.runtime.get(name).ok_or(Error::UnknownVariable)?,
                }
            }
            Token::Minus => {
                self.advance()?;
                match self.expression(depth + 1, 7)? {
                    Value::Integer(value) => {
                        Value::Integer(value.checked_neg().ok_or(Error::ArithmeticOverflow)?)
                    }
                    _ => return Err(Error::UnexpectedToken),
                }
            }
            Token::Bang => {
                self.advance()?;
                match self.expression(depth + 1, 7)? {
                    Value::Boolean(value) => Value::Boolean(!value),
                    _ => return Err(Error::UnexpectedToken),
                }
            }
            Token::LParen => {
                self.advance()?;
                let value = self.expression(depth + 1, 0)?;
                if self.current != Token::RParen {
                    return Err(Error::UnexpectedToken);
                }
                self.advance()?;
                value
            }
            _ => return Err(Error::UnexpectedToken),
        };
        loop {
            let operator = self.current;
            let precedence = match operator {
                Token::OrOr => 1,
                Token::AndAnd => 2,
                Token::EqualEqual | Token::NotEqual => 3,
                Token::Less | Token::LessEqual | Token::Greater | Token::GreaterEqual => 4,
                Token::Plus | Token::Minus => 5,
                Token::Star | Token::Slash => 6,
                _ => break,
            };
            if precedence < minimum_precedence {
                break;
            }
            self.advance()?;
            let right = self.expression(depth + 1, precedence + 1)?;
            left = match (left, right, operator, precedence) {
                (Value::Boolean(left), Value::Boolean(right), Token::OrOr, 1) => {
                    Value::Boolean(left || right)
                }
                (Value::Boolean(left), Value::Boolean(right), Token::AndAnd, 2) => {
                    Value::Boolean(left && right)
                }
                (Value::Integer(left), Value::Integer(right), Token::EqualEqual, 3) => {
                    Value::Boolean(left == right)
                }
                (Value::Integer(left), Value::Integer(right), Token::NotEqual, 3) => {
                    Value::Boolean(left != right)
                }
                (Value::Boolean(left), Value::Boolean(right), Token::EqualEqual, 3) => {
                    Value::Boolean(left == right)
                }
                (Value::Boolean(left), Value::Boolean(right), Token::NotEqual, 3) => {
                    Value::Boolean(left != right)
                }
                (Value::Integer(left), Value::Integer(right), Token::Less, 4) => {
                    Value::Boolean(left < right)
                }
                (Value::Integer(left), Value::Integer(right), Token::LessEqual, 4) => {
                    Value::Boolean(left <= right)
                }
                (Value::Integer(left), Value::Integer(right), Token::Greater, 4) => {
                    Value::Boolean(left > right)
                }
                (Value::Integer(left), Value::Integer(right), Token::GreaterEqual, 4) => {
                    Value::Boolean(left >= right)
                }
                (Value::Integer(left), Value::Integer(right), Token::Plus, 5) => {
                    Value::Integer(left.checked_add(right).ok_or(Error::ArithmeticOverflow)?)
                }
                (Value::Integer(left), Value::Integer(right), Token::Minus, 5) => {
                    Value::Integer(left.checked_sub(right).ok_or(Error::ArithmeticOverflow)?)
                }
                (Value::Integer(left), Value::Integer(right), Token::Star, 6) => {
                    Value::Integer(left.checked_mul(right).ok_or(Error::ArithmeticOverflow)?)
                }
                (Value::Integer(left), Value::Integer(right), Token::Slash, 6) => {
                    if right == 0 {
                        return Err(Error::DivisionByZero);
                    }
                    Value::Integer(left.checked_div(right).ok_or(Error::ArithmeticOverflow)?)
                }
                _ => return Err(Error::UnexpectedToken),
            };
        }
        Ok(left)
    }
}

pub fn eval<'source, const N: usize>(
    source: &'source str,
    runtime: &mut Runtime<'source, N>,
) -> Result<Value, Error> {
    if source.len() > 4096 {
        return Err(Error::ScriptTooLong);
    }
    Parser::new(source, runtime)?.script()
}

#[cfg(test)]
mod tests {
    use super::{Error, Runtime, Value, eval};

    #[test]
    fn evaluates_bounded_script_and_preserves_bindings() {
        let mut runtime = Runtime::<2>::new();
        assert_eq!(
            eval("let answer = 6 * 7; answer + 1", &mut runtime),
            Ok(Value::Integer(43))
        );
        assert_eq!(runtime.get("answer"), Some(Value::Integer(42)));
    }

    #[test]
    fn rejects_unknown_variables_capacity_and_division_by_zero() {
        let mut runtime = Runtime::<1>::new();
        assert_eq!(
            eval("missing + 1", &mut runtime),
            Err(Error::UnknownVariable)
        );
        assert_eq!(
            eval("let a = 1; let b = 2", &mut runtime),
            Err(Error::VariableCapacity)
        );
        assert_eq!(eval("1 / 0", &mut runtime), Err(Error::DivisionByZero));
    }

    #[test]
    fn supports_comments_unary_values_and_fail_closed_tokens() {
        let mut runtime = Runtime::<1>::new();
        assert_eq!(
            eval("// comment\n-3 + (2 * 4)", &mut runtime),
            Ok(Value::Integer(5))
        );
        assert_eq!(eval("1.5", &mut runtime), Err(Error::InvalidToken));
    }

    #[test]
    fn evaluates_bounded_booleans_comparisons_and_logical_operators() {
        let mut runtime = Runtime::<2>::new();
        assert_eq!(
            eval("let ready = 6 * 7 == 42; ready && !false", &mut runtime),
            Ok(Value::Boolean(true))
        );
        assert_eq!(
            eval("3 < 4 && 8 >= 8", &mut runtime),
            Ok(Value::Boolean(true))
        );
        assert_eq!(
            eval("true != false", &mut runtime),
            Ok(Value::Boolean(true))
        );
    }
}
