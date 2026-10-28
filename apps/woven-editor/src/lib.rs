#![cfg_attr(not(test), no_std)]

const MAX_ARGUMENT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Help,
    View(&'a [u8]),
    Write(&'a [u8], &'a [u8]),
    Invalid,
}

pub fn parse<'a>(args: &'a [&'a [u8]]) -> Command<'a> {
    match args {
        [] | [b"--help"] | [b"help"] => Command::Help,
        [b"--view", path] | [b"view", path] if path.len() <= MAX_ARGUMENT => Command::View(path),
        [b"--write", path, text] | [b"write", path, text]
            if path.len() <= MAX_ARGUMENT && text.len() <= MAX_ARGUMENT =>
        {
            Command::Write(path, text)
        }
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Command};

    #[test]
    fn accepts_bounded_commands() {
        assert_eq!(parse(&[b"--help"]), Command::Help);
        assert_eq!(parse(&[b"--view", b"/mnt/a"]), Command::View(b"/mnt/a"));
        assert_eq!(
            parse(&[b"--write", b"/wfs/note", b"hello"]),
            Command::Write(b"/wfs/note", b"hello")
        );
    }

    #[test]
    fn rejects_bad_arity_and_overlong_arguments() {
        assert_eq!(parse(&[b"--write", b"/wfs/note"]), Command::Invalid);
        assert_eq!(parse(&[b"--view", &[b'x'; 257]]), Command::Invalid);
    }
}
