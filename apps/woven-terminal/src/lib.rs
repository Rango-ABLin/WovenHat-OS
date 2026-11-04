#![cfg_attr(not(test), no_std)]

const MAX_COMMAND: usize = 128;
pub const MAX_PIPE_COMMANDS: usize = 4;
pub const MAX_ARGS: usize = 8;
pub const MAX_ARG: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellError {
    Empty,
    UnterminatedQuote,
    TooManyCommands,
    TooManyArguments,
    WordTooLong,
    MissingRedirectionTarget,
    InvalidOperator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellCommand {
    pub argc: usize,
    pub args: [[u8; MAX_ARG]; MAX_ARGS],
    pub lengths: [usize; MAX_ARGS],
    pub input: Option<usize>,
    pub output: Option<usize>,
    pub append: bool,
    pub error: Option<usize>,
}

impl ShellCommand {
    const EMPTY: Self = Self {
        argc: 0,
        args: [[0; MAX_ARG]; MAX_ARGS],
        lengths: [0; MAX_ARGS],
        input: None,
        output: None,
        append: false,
        error: None,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellLine {
    pub count: usize,
    pub commands: [ShellCommand; MAX_PIPE_COMMANDS],
    pub background: bool,
}

impl ShellLine {
    const EMPTY: Self = Self {
        count: 0,
        commands: [ShellCommand::EMPTY; MAX_PIPE_COMMANDS],
        background: false,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Help,
    List(&'a [u8]),
    Cat(&'a [u8]),
    Calc(&'a [u8]),
    External(&'a [u8]),
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
    if input.is_ascii() {
        Command::External(input)
    } else {
        Command::Invalid
    }
}

/// Parse a bounded shell line. Quotes and backslashes are retained in words;
/// the executor removes them while performing environment expansion. This
/// keeps single-quoted `$NAME` distinct from expandable unquoted text.
pub fn parse_shell(input: &[u8]) -> Result<ShellLine, ShellError> {
    if input.len() > MAX_COMMAND {
        return Err(ShellError::WordTooLong);
    }
    let mut line = ShellLine::EMPTY;
    let mut command = 0usize;
    let mut word = [0u8; MAX_ARG];
    let mut word_len = 0usize;
    let mut quote = 0u8;
    let mut escaped = false;
    let mut index = 0usize;
    let flush = |line: &mut ShellLine,
                 command: usize,
                 word: &[u8; MAX_ARG],
                 word_len: &mut usize|
     -> Result<(), ShellError> {
        if *word_len == 0 {
            return Ok(());
        }
        let current = &mut line.commands[command];
        if current.argc >= MAX_ARGS {
            return Err(ShellError::TooManyArguments);
        }
        current.args[current.argc][..*word_len].copy_from_slice(&word[..*word_len]);
        current.lengths[current.argc] = *word_len;
        current.argc += 1;
        *word_len = 0;
        Ok(())
    };
    while index < input.len() {
        let byte = input[index];
        if escaped {
            if word_len >= MAX_ARG {
                return Err(ShellError::WordTooLong);
            }
            word[word_len] = b'\\';
            word_len += 1;
            if word_len >= MAX_ARG {
                return Err(ShellError::WordTooLong);
            }
            word[word_len] = byte;
            word_len += 1;
            escaped = false;
            index += 1;
            continue;
        }
        if byte == b'\\' && quote != b'\'' {
            escaped = true;
            index += 1;
            continue;
        }
        if quote != 0 {
            if byte == quote {
                if word_len >= MAX_ARG {
                    return Err(ShellError::WordTooLong);
                }
                word[word_len] = byte;
                word_len += 1;
                quote = 0;
            } else {
                if word_len >= MAX_ARG {
                    return Err(ShellError::WordTooLong);
                }
                word[word_len] = byte;
                word_len += 1;
            }
            index += 1;
            continue;
        }
        if byte == b'\'' || byte == b'"' {
            if word_len >= MAX_ARG {
                return Err(ShellError::WordTooLong);
            }
            word[word_len] = byte;
            word_len += 1;
            quote = byte;
            index += 1;
            continue;
        }
        if byte.is_ascii_whitespace() {
            flush(&mut line, command, &word, &mut word_len)?;
            index += 1;
            continue;
        }
        let op = match byte {
            b'|' => Some(1),
            b'<' => Some(2),
            b'2' if input.get(index + 1) == Some(&b'>') && word_len == 0 => Some(5),
            b'>' => Some(if input.get(index + 1) == Some(&b'>') {
                4
            } else {
                3
            }),
            _ => None,
        };
        if let Some(op) = op {
            flush(&mut line, command, &word, &mut word_len)?;
            if op == 1 {
                if line.commands[command].argc == 0 || command + 1 >= MAX_PIPE_COMMANDS {
                    return Err(ShellError::TooManyCommands);
                }
                command += 1;
            } else {
                let slot_is_set = match op {
                    2 => line.commands[command].input.is_some(),
                    3 | 4 => line.commands[command].output.is_some(),
                    5 => line.commands[command].error.is_some(),
                    _ => true,
                };
                if slot_is_set {
                    return Err(ShellError::InvalidOperator);
                }
                let mut target = [0u8; MAX_ARG];
                let mut target_len = 0usize;
                index += if op == 5 {
                    3
                } else if op == 4 {
                    2
                } else {
                    1
                };
                while index < input.len() && input[index].is_ascii_whitespace() {
                    index += 1;
                }
                if index >= input.len() {
                    return Err(ShellError::MissingRedirectionTarget);
                }
                let mut target_quote = 0u8;
                while index < input.len() {
                    if input[index] == b'\\' && target_quote != b'\'' {
                        if index + 1 >= input.len() {
                            return Err(ShellError::UnterminatedQuote);
                        }
                        if target_len + 1 >= MAX_ARG {
                            return Err(ShellError::WordTooLong);
                        }
                        target[target_len] = input[index];
                        target_len += 1;
                        index += 1;
                        target[target_len] = input[index];
                        target_len += 1;
                        index += 1;
                        continue;
                    } else if input[index] == b'\'' || input[index] == b'"' {
                        if target_quote == 0 {
                            target_quote = input[index];
                        } else if target_quote == input[index] {
                            target_quote = 0;
                        }
                    } else if target_quote == 0
                        && (input[index].is_ascii_whitespace()
                            || matches!(input[index], b'|' | b'<' | b'>'))
                    {
                        break;
                    }
                    if target_len >= MAX_ARG {
                        return Err(ShellError::WordTooLong);
                    }
                    target[target_len] = input[index];
                    target_len += 1;
                    index += 1;
                }
                if target_quote != 0 {
                    return Err(ShellError::UnterminatedQuote);
                }
                if target_len == 0 {
                    return Err(ShellError::MissingRedirectionTarget);
                }
                let current = &mut line.commands[command];
                current.args[current.argc][..target_len].copy_from_slice(&target[..target_len]);
                current.lengths[current.argc] = target_len;
                let target_index = current.argc;
                current.argc += 1;
                match op {
                    2 => current.input = Some(target_index),
                    3 | 4 => current.output = Some(target_index),
                    5 => current.error = Some(target_index),
                    _ => unreachable!(),
                }
                if op == 4 || op == 5 {
                    current.append = true;
                }
                continue;
            }
            index += 1;
            continue;
        }
        if byte == b'&' {
            flush(&mut line, command, &word, &mut word_len)?;
            if index + 1 != input.len() || line.background {
                return Err(ShellError::InvalidOperator);
            }
            line.background = true;
            index += 1;
            continue;
        }
        if word_len >= MAX_ARG {
            return Err(ShellError::WordTooLong);
        }
        word[word_len] = byte;
        word_len += 1;
        index += 1;
    }
    if escaped || quote != 0 {
        return Err(ShellError::UnterminatedQuote);
    }
    flush(&mut line, command, &word, &mut word_len)?;
    if line.commands[command].argc == 0 {
        return Err(ShellError::Empty);
    }
    line.count = command + 1;
    Ok(line)
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
    use super::{parse, parse_shell, Command};

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
        assert_eq!(parse(b"lss /mnt"), Command::External(b"lss /mnt"));
        assert_eq!(parse(b"cat"), Command::External(b"cat"));
        assert_eq!(parse(&[b'x'; 129]), Command::Invalid);
    }

    #[test]
    fn parses_shell_operators_and_quotes() {
        let line = parse_shell(b"echo 'a|b' | cat > out 2>> err &").unwrap();
        assert_eq!(line.count, 2);
        assert!(line.background);
        assert_eq!(
            &line.commands[0].args[1][..line.commands[0].lengths[1]],
            b"'a|b'"
        );
        assert_eq!(line.commands[1].output, Some(1));
        assert_eq!(line.commands[1].error, Some(2));
        assert!(line.commands[1].append);
        let redirected = parse_shell(b"echo ok > 'out file'").unwrap();
        let command = &redirected.commands[0];
        assert_eq!(
            &command.args[command.output.unwrap()][..command.lengths[command.output.unwrap()]],
            b"'out file'"
        );
    }

    #[test]
    fn rejects_unterminated_shell_input() {
        assert!(parse_shell(b"echo 'unfinished").is_err());
        assert!(parse_shell(b"echo |").is_err());
    }
}
