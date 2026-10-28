#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    List,
    Help,
    Invalid,
}

pub fn parse(input: &[u8]) -> Command {
    match input {
        b"--list" | b"list" | b"--all" | b"" => Command::List,
        b"--help" | b"help" => Command::Help,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Command};

    #[test]
    fn accepts_catalog_modes() {
        assert_eq!(parse(b""), Command::List);
        assert_eq!(parse(b"--list"), Command::List);
        assert_eq!(parse(b"help"), Command::Help);
    }

    #[test]
    fn rejects_mutation_modes() {
        assert_eq!(parse(b"--install"), Command::Invalid);
        assert_eq!(parse(b"--remove"), Command::Invalid);
    }
}
