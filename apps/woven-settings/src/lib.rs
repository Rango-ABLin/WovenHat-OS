#![cfg_attr(not(test), no_std)]

const MAX_COMMAND: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    All,
    Identity,
    Clock,
    Display,
    Help,
    Invalid,
}

pub fn parse(input: &[u8]) -> Command {
    if input.len() > MAX_COMMAND {
        return Command::Invalid;
    }
    match input {
        b"--all" | b"all" => Command::All,
        b"--identity" | b"identity" => Command::Identity,
        b"--clock" | b"clock" => Command::Clock,
        b"--display" | b"display" => Command::Display,
        b"--help" | b"help" => Command::Help,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Command};

    #[test]
    fn accepts_bounded_modes() {
        assert_eq!(parse(b"--all"), Command::All);
        assert_eq!(parse(b"identity"), Command::Identity);
        assert_eq!(parse(b"--clock"), Command::Clock);
        assert_eq!(parse(b"display"), Command::Display);
        assert_eq!(parse(b"help"), Command::Help);
    }

    #[test]
    fn rejects_unknown_and_overlong_modes() {
        assert_eq!(parse(b"--network"), Command::Invalid);
        assert_eq!(parse(&[b'x'; 33]), Command::Invalid);
    }
}
