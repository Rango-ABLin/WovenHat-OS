#![cfg_attr(not(test), no_std)]

/// App-side bound for candidate paths handed to the privileged update broker.
pub fn valid_update_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && path.starts_with("/updates/")
        && !path.as_bytes().contains(&0)
        && !path.contains('\\')
        && !path.split('/').any(|part| part == "." || part == "..")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Check,
    Service,
    Query,
    Stage,
    Activate,
    Help,
    Invalid,
}

pub fn parse(args: &[&[u8]]) -> Command {
    match args {
        [b"--check"] | [b"check"] | [b"--all"] | [] => Command::Check,
        [b"--service"] | [b"service"] => Command::Service,
        [b"--query"] => Command::Query,
        [b"--stage", _] => Command::Stage,
        [b"--activate", _] => Command::Activate,
        [b"--help"] | [b"help"] => Command::Help,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, valid_update_path, Command};

    #[test]
    fn accepts_preflight_modes() {
        assert_eq!(parse(&[b"--check"]), Command::Check);
        assert_eq!(parse(&[b"--all"]), Command::Check);
        assert_eq!(parse(&[b"--service"]), Command::Service);
        assert_eq!(parse(&[b"help"]), Command::Help);
    }

    #[test]
    fn accepts_bounded_transaction_modes() {
        assert_eq!(parse(&[b"--query"]), Command::Query);
        assert_eq!(parse(&[b"--stage", b"/updates/a.img"]), Command::Stage);
        assert_eq!(parse(&[b"--activate", b"0"]), Command::Activate);
        assert_eq!(parse(&[b"--stage"]), Command::Invalid);
    }

    #[test]
    fn update_paths_are_confined() {
        assert!(valid_update_path("/updates/a.img"));
        assert!(!valid_update_path(""));
        assert!(!valid_update_path("/updates/../a.img"));
        assert!(!valid_update_path("/updates\\a.img"));
    }
}
