#![cfg_attr(not(test), no_std)]

const MAX_ARGUMENT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Help,
    List(&'a [u8]),
    Info(&'a [u8]),
    Invalid,
}

pub fn parse<'a>(args: &'a [&'a [u8]]) -> Command<'a> {
    match args {
        [] | [b"--help"] | [b"help"] => Command::Help,
        [b"--list", path] | [b"list", path] if path.len() <= MAX_ARGUMENT => Command::List(path),
        [b"--info", path] | [b"info", path] if path.len() <= MAX_ARGUMENT => Command::Info(path),
        _ => Command::Invalid,
    }
}

pub fn kind(header: &[u8]) -> &'static str {
    if header.starts_with(b"\x89PNG\r\n\x1a\n") {
        "PNG"
    } else if header.starts_with(b"\xff\xd8\xff") {
        "JPEG"
    } else if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
        "GIF"
    } else if header.starts_with(b"BM") {
        "BMP"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::{kind, parse, Command};

    #[test]
    fn accepts_bounded_commands_and_headers() {
        assert_eq!(parse(&[b"--list", b"/mnt"]), Command::List(b"/mnt"));
        assert_eq!(
            parse(&[b"info", b"/mnt/a.png"]),
            Command::Info(b"/mnt/a.png")
        );
        assert_eq!(kind(b"\x89PNG\r\n\x1a\n"), "PNG");
        assert_eq!(kind(b"\xff\xd8\xff\x00"), "JPEG");
        assert_eq!(kind(b"GIF89a"), "GIF");
        assert_eq!(kind(b"BM"), "BMP");
    }

    #[test]
    fn rejects_unknown_and_overlong_arguments() {
        assert_eq!(parse(&[b"--render", b"/mnt/a"]), Command::Invalid);
        assert_eq!(parse(&[b"--info", &[b'x'; 257]]), Command::Invalid);
        assert_eq!(kind(b"not an image"), "unknown");
    }
}
