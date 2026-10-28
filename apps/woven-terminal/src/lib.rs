#![cfg_attr(not(test), no_std)]

const MAX_COMMAND: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Help,
    List(&'a [u8]),
    Cat(&'a [u8]),
    Calc(&'a [u8]),
    Quit,
    Invalid,
}

/// Parse one bounded command without allocation. Paths and expressions remain
/// borrowed from the caller and are validated by their respective APIs.
pub fn parse(input: &[u8]) -> Command<'_> {
    if input.len() > MAX_COMMAND {
        return Command::Invalid;
    }
    let input = trim(input);
    if input == b"help" || input == b"?" {
        return Command::Help;
    }
    if input == b"quit" || input == b"exit" {
        return Command::Quit;
    }
    if let Some(argument) = after(input, b"ls") {
        return Command::List(argument);
    }
    if let Some(argument) = after(input, b"cat") {
        return Command::Cat(argument);
    }
    if let Some(argument) = after(input, b"calc") {
        return Command::Calc(argument);
    }
    Command::Invalid
}

fn trim(input: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = input.len();
    while start < end && input[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && input[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &input[start..end]
}

fn after<'a>(input: &'a [u8], command: &[u8]) -> Option<&'a [u8]> {
    let remainder = input.strip_prefix(command)?;
    let remainder = remainder.strip_prefix(b" ")?;
    let remainder = trim(remainder);
    (!remainder.is_empty()).then_some(remainder)
}

#[cfg(test)]
mod tests {
    use super::{parse, Command};

    #[test]
    fn parses_bounded_commands() {
        assert_eq!(parse(b"help"), Command::Help);
        assert_eq!(parse(b"ls /mnt"), Command::List(b"/mnt"));
        assert_eq!(parse(b"cat /wfs/note"), Command::Cat(b"/wfs/note"));
        assert_eq!(parse(b"calc 2+2"), Command::Calc(b"2+2"));
        assert_eq!(parse(b"exit"), Command::Quit);
    }

    #[test]
    fn refuses_ambiguous_or_overlong_commands() {
        assert_eq!(parse(b"lss /mnt"), Command::Invalid);
        assert_eq!(parse(b"cat"), Command::Invalid);
        assert_eq!(parse(&[b'x'; 129]), Command::Invalid);
    }
}
