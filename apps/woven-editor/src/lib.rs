#![cfg_attr(not(test), no_std)]

const MAX_ARGUMENT: usize = 256;

/// The only note path the bounded GUI may target by default.
pub const DEFAULT_NOTE: &str = "/mnt/woven-note.txt";

/// App-side least-privilege check for GUI Open/Save As targets. The kernel
/// still enforces `FileRead`/`FileWrite` and the mounted-path policy; this only
/// keeps the editor from silently targeting a system path.
pub fn valid_note_path(path: &str) -> bool {
    if path.is_empty() || path.len() > MAX_ARGUMENT || !path.starts_with("/mnt/") {
        return false;
    }
    if path.as_bytes().contains(&0) || path.as_bytes().contains(&b'\\') {
        return false;
    }
    path.split('/').skip(1).all(|part| {
        !part.is_empty() && part != "." && part != ".." && part.len() <= 64
    })
}

/// Trim a file read into a bounded editor buffer at a UTF-8 boundary.
pub fn trim_utf8(bytes: &[u8]) -> &str {
    let mut end = bytes.len();
    while end > 0 && core::str::from_utf8(&bytes[..end]).is_err() {
        end -= 1;
    }
    core::str::from_utf8(&bytes[..end]).unwrap_or("")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    InvalidPath,
    OpenDenied,
    ReadFailed,
    WriteFailed,
    SyncFailed,
}

impl SaveError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid path",
            Self::OpenDenied => "open denied",
            Self::ReadFailed => "read failed",
            Self::WriteFailed => "write failed",
            Self::SyncFailed => "sync failed",
        }
    }
}

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
    use super::{parse, trim_utf8, valid_note_path, Command, SaveError, DEFAULT_NOTE};

    #[test]
    fn note_paths_are_bounded_and_confined_to_mnt() {
        assert!(valid_note_path(DEFAULT_NOTE));
        assert!(valid_note_path("/mnt/notes/a-b_1.txt"));
        assert!(!valid_note_path(""));
        assert!(!valid_note_path("/bin/wovenhat-editor"));
        assert!(!valid_note_path("/mnt/"));
        assert!(!valid_note_path("/mnt/../etc/passwd"));
        assert!(!valid_note_path("/mnt/a/./b"));
        assert!(!valid_note_path("/mnt\\note"));
        let mut long = String::from("/mnt/");
        long.push_str(&"a".repeat(300));
        assert!(!valid_note_path(&long));
    }

    #[test]
    fn file_reads_trim_at_a_utf8_boundary() {
        assert_eq!(trim_utf8(b"hello"), "hello");
        assert_eq!(trim_utf8(&[b'a', 0xF0, 0x9F]), "a");
        assert_eq!(trim_utf8(&[]), "");
    }

    #[test]
    fn save_errors_are_named_for_display() {
        assert_eq!(SaveError::InvalidPath.as_str(), "invalid path");
        assert_eq!(SaveError::OpenDenied.as_str(), "open denied");
        assert_eq!(SaveError::ReadFailed.as_str(), "read failed");
        assert_eq!(SaveError::WriteFailed.as_str(), "write failed");
        assert_eq!(SaveError::SyncFailed.as_str(), "sync failed");
    }

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
