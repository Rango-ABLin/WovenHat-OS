#![cfg_attr(not(test), no_std)]

/// Bounded arithmetic errors. No parser path allocates or panics on malformed
/// userspace input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    Invalid,
    TooLong,
    Unbalanced,
    DivideByZero,
    Overflow,
}

const MAX_EXPRESSION: usize = 96;
const MAX_DEPTH: usize = 16;

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Result<Self, Error> {
        if input.is_empty() {
            return Err(Error::Empty);
        }
        if input.len() > MAX_EXPRESSION {
            return Err(Error::TooLong);
        }
        Ok(Self {
            input,
            position: 0,
            depth: 0,
        })
    }

    fn skip_spaces(&mut self) {
        while self.position < self.input.len() && self.input[self.position] == b' ' {
            self.position += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_spaces();
        self.input.get(self.position).copied()
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn expression(&mut self) -> Result<i64, Error> {
        let mut value = self.term()?;
        loop {
            let operation = match self.peek() {
                Some(b'+') => b'+',
                Some(b'-') => b'-',
                _ => break,
            };
            self.position += 1;
            let right = self.term()?;
            value = if operation == b'+' {
                value.checked_add(right).ok_or(Error::Overflow)?
            } else {
                value.checked_sub(right).ok_or(Error::Overflow)?
            };
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<i64, Error> {
        let mut value = self.factor()?;
        loop {
            let operation = match self.peek() {
                Some(b'*') => b'*',
                Some(b'/') => b'/',
                _ => break,
            };
            self.position += 1;
            let right = self.factor()?;
            value = if operation == b'*' {
                value.checked_mul(right).ok_or(Error::Overflow)?
            } else if right == 0 {
                return Err(Error::DivideByZero);
            } else {
                value.checked_div(right).ok_or(Error::Overflow)?
            };
        }
        Ok(value)
    }

    fn factor(&mut self) -> Result<i64, Error> {
        if self.consume(b'-') {
            return self.factor()?.checked_neg().ok_or(Error::Overflow);
        }
        if self.consume(b'+') {
            return self.factor();
        }
        if self.consume(b'(') {
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                return Err(Error::Unbalanced);
            }
            let value = self.expression()?;
            self.depth -= 1;
            if !self.consume(b')') {
                return Err(Error::Unbalanced);
            }
            return Ok(value);
        }

        self.skip_spaces();
        let start = self.position;
        let mut value = 0i64;
        while let Some(byte) = self.input.get(self.position).copied() {
            if !byte.is_ascii_digit() {
                break;
            }
            self.position += 1;
            value = value
                .checked_mul(10)
                .and_then(|number| number.checked_add(i64::from(byte - b'0')))
                .ok_or(Error::Overflow)?;
        }
        if self.position == start {
            Err(Error::Invalid)
        } else {
            Ok(value)
        }
    }
}

/// Evaluate one ASCII arithmetic expression with bounded nesting and checked
/// signed 64-bit operations.
pub fn evaluate(input: &[u8]) -> Result<i64, Error> {
    let mut parser = Parser::new(input)?;
    let value = parser.expression()?;
    if parser.peek().is_some() {
        return Err(Error::Invalid);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{evaluate, Error};

    #[test]
    fn precedence_and_parentheses_are_exact() {
        assert_eq!(evaluate(b"2 + 3 * (4 - 1)"), Ok(11));
        assert_eq!(evaluate(b"-8 / 2 + 7"), Ok(3));
    }

    #[test]
    fn malformed_and_unsafe_operations_fail_closed() {
        assert_eq!(evaluate(b""), Err(Error::Empty));
        assert_eq!(evaluate(b"1 / 0"), Err(Error::DivideByZero));
        assert_eq!(evaluate(b"(1 + 2"), Err(Error::Unbalanced));
        assert_eq!(evaluate(b"1 + nope"), Err(Error::Invalid));
        assert_eq!(evaluate(b"9223372036854775807 + 1"), Err(Error::Overflow));
    }

    #[test]
    fn input_and_nesting_are_bounded() {
        assert_eq!(evaluate(&[b'1'; 97]), Err(Error::TooLong));
        let deep = b"((((((((((((((((((1))))))))))))))))))";
        assert_eq!(evaluate(deep), Err(Error::Unbalanced));
    }
}
