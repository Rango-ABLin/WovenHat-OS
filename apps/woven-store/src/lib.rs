#![cfg_attr(not(test), no_std)]

/// App-side least-privilege check for package paths handed to the privileged
/// broker. The broker and kernel still enforce their own authority; this only
/// rejects malformed or ambiguous paths before an IPC send.
pub fn valid_package_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && path.starts_with('/')
        && !path.as_bytes().contains(&0)
        && !path.contains('\\')
        && !path.split('/').any(|part| part == "." || part == "..")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    List,
    Query,
    Install,
    Remove,
    Rollback,
    Help,
    Invalid,
}

pub fn parse(args: &[&[u8]]) -> Command {
    match args {
        [] | [b"--list"] | [b"list"] | [b"--all"] => Command::List,
        [b"--help"] | [b"help"] => Command::Help,
        [b"--query", _] => Command::Query,
        [b"--install", _] => Command::Install,
        [b"--remove", _] => Command::Remove,
        [b"--rollback", _] => Command::Rollback,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, valid_package_path, Command};

    #[test]
    fn accepts_catalog_modes() {
        assert_eq!(parse(&[]), Command::List);
        assert_eq!(parse(&[b"--list"]), Command::List);
        assert_eq!(parse(&[b"help"]), Command::Help);
    }

    #[test]
    fn accepts_bounded_mutation_modes() {
        assert_eq!(parse(&[b"--query", b"/updates/a.wpkg"]), Command::Query);
        assert_eq!(parse(&[b"--install", b"/updates/a.wpkg"]), Command::Install);
        assert_eq!(parse(&[b"--remove", b"/updates/a.wpkg"]), Command::Remove);
        assert_eq!(parse(&[b"--rollback", b"/updates/a.wpkg"]), Command::Rollback);
        assert_eq!(parse(&[b"--install"]), Command::Invalid);
    }

    #[test]
    fn package_paths_are_bounded_and_confined() {
        assert!(valid_package_path("/updates/a.wpkg"));
        assert!(!valid_package_path(""));
        assert!(!valid_package_path("relative.wpkg"));
        assert!(!valid_package_path("/updates/../a.wpkg"));
        assert!(!valid_package_path("/updates\\a.wpkg"));
    }
}
