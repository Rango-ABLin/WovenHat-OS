#![cfg_attr(not(test), no_std)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Status,
    FilesystemCheck,
    Logs,
    Reset,
    Reinstall,
    Help,
    Invalid,
}

pub fn parse(input: &[u8]) -> Command {
    match input {
        b"status" | b"--status" => Command::Status,
        b"check" | b"--check" | b"fsck" => Command::FilesystemCheck,
        b"logs" | b"--logs" => Command::Logs,
        b"reset" | b"--reset" => Command::Reset,
        b"reinstall" | b"--reinstall" => Command::Reinstall,
        b"help" | b"--help" => Command::Help,
        _ => Command::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Command};

    #[test]
    fn accepts_bounded_recovery_commands() {
        assert_eq!(parse(b"status"), Command::Status);
        assert_eq!(parse(b"--check"), Command::FilesystemCheck);
        assert_eq!(parse(b"logs"), Command::Logs);
        assert_eq!(parse(b"reset"), Command::Reset);
        assert_eq!(parse(b"--reinstall"), Command::Reinstall);
    }

    #[test]
    fn rejects_unimplemented_or_ambiguous_commands() {
        assert_eq!(parse(b"snapshot"), Command::Invalid);
        assert_eq!(parse(b"repair-boot"), Command::Invalid);
        assert_eq!(parse(b""), Command::Invalid);
    }
}
