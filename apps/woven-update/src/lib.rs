#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Check,
    Service,
    Help,
    Invalid,
}

pub fn parse(input: &[u8]) -> Command {
    match input {
        b"--check" | b"check" | b"--all" => Command::Check,
        b"--service" | b"service" => Command::Service,
        b"--help" | b"help" => Command::Help,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Command};

    #[test]
    fn accepts_preflight_modes() {
        assert_eq!(parse(b"--check"), Command::Check);
        assert_eq!(parse(b"--all"), Command::Check);
        assert_eq!(parse(b"--service"), Command::Service);
        assert_eq!(parse(b"help"), Command::Help);
    }

    #[test]
    fn rejects_mutation_modes() {
        assert_eq!(parse(b"--install"), Command::Invalid);
        assert_eq!(parse(b"--rollback"), Command::Invalid);
    }
}
