//! Bounded HTTP(S) protocol boundaries for userspace network clients.
//!
//! This module parses request metadata only. Socket ownership, TLS, response
//! bodies, and policy enforcement remain with the caller and permission
//! broker; malformed input is rejected without allocation.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint<'a> {
    pub host: &'a str,
    pub port: u16,
    pub path: &'a str,
    pub secure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnsupportedScheme,
    MissingHost,
    InvalidPort,
    InvalidCharacter,
    TooLong,
}

pub fn parse_endpoint(uri: &str) -> Result<Endpoint<'_>, ParseError> {
    if uri.is_empty() {
        return Err(ParseError::Empty);
    }
    if uri.len() > 256 {
        return Err(ParseError::TooLong);
    }
    let (secure, remainder) = if let Some(value) = uri.strip_prefix("https://") {
        (true, value)
    } else if let Some(value) = uri.strip_prefix("http://") {
        (false, value)
    } else {
        return Err(ParseError::UnsupportedScheme);
    };
    if remainder
        .bytes()
        .any(|byte| byte < 0x21 || byte == 0x7f || byte == b'@')
    {
        return Err(ParseError::InvalidCharacter);
    }
    let (authority, path) = if let Some(separator) = remainder.find('/') {
        (&remainder[..separator], &remainder[separator..])
    } else {
        (remainder, "/")
    };
    if authority.is_empty() {
        return Err(ParseError::MissingHost);
    }
    let (host, port) = if let Some((host, port_text)) = authority.rsplit_once(':') {
        if host.is_empty() || port_text.is_empty() {
            return Err(ParseError::InvalidPort);
        }
        let mut port = 0u32;
        for byte in port_text.bytes() {
            if !byte.is_ascii_digit() {
                return Err(ParseError::InvalidPort);
            }
            port = port
                .checked_mul(10)
                .and_then(|value| value.checked_add(u32::from(byte - b'0')))
                .ok_or(ParseError::InvalidPort)?;
        }
        (
            host,
            u16::try_from(port).map_err(|_| ParseError::InvalidPort)?,
        )
    } else {
        (authority, if secure { 443 } else { 80 })
    };
    if host.is_empty() || host.len() > 253 || path.len() > 256 {
        return Err(ParseError::MissingHost);
    }
    Ok(Endpoint {
        host,
        port,
        path,
        secure,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseHead {
    pub status: u16,
    pub content_length: Option<u64>,
    pub chunked: bool,
}

pub fn parse_response_head(bytes: &[u8]) -> Option<ResponseHead> {
    let line_end = bytes.windows(2).position(|pair| pair == b"\r\n")?;
    let line = core::str::from_utf8(&bytes[..line_end]).ok()?;
    let mut parts = line.split(' ');
    if parts.next()? != "HTTP/1.1" {
        return None;
    }
    let status = parts.next()?.parse::<u16>().ok()?;
    if !(100..=599).contains(&status) {
        return None;
    }
    let mut content_length = None;
    let mut chunked = false;
    for header in bytes[line_end + 2..].split(|byte| *byte == b'\n') {
        let header = header.strip_suffix(b"\r").unwrap_or(header);
        if header.is_empty() {
            break;
        }
        let separator = header.iter().position(|byte| *byte == b':')?;
        let name = &header[..separator];
        let value = &header[separator + 1..];
        if name.eq_ignore_ascii_case(b"content-length") {
            let value = core::str::from_utf8(value.trim_ascii()).ok()?;
            content_length = Some(value.parse().ok()?);
        } else if name.eq_ignore_ascii_case(b"transfer-encoding")
            && value
                .split(|byte| *byte == b',')
                .any(|part| part.trim_ascii().eq_ignore_ascii_case(b"chunked"))
        {
            chunked = true;
        }
    }
    Some(ResponseHead {
        status,
        content_length,
        chunked,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyError {
    Truncated,
    Malformed,
    OutputTooSmall,
}

fn chunk_size(line: &[u8]) -> Result<usize, BodyError> {
    let number = line
        .split(|byte| *byte == b';')
        .next()
        .unwrap_or(line)
        .trim_ascii();
    if number.is_empty() {
        return Err(BodyError::Malformed);
    }
    let mut size = 0usize;
    for byte in number {
        let digit = match byte {
            b'0'..=b'9' => usize::from(byte - b'0'),
            b'a'..=b'f' => usize::from(byte - b'a' + 10),
            b'A'..=b'F' => usize::from(byte - b'A' + 10),
            _ => return Err(BodyError::Malformed),
        };
        size = size
            .checked_mul(16)
            .and_then(|value| value.checked_add(digit))
            .ok_or(BodyError::Malformed)?;
    }
    Ok(size)
}

fn walk_chunked(bytes: &[u8], mut output: Option<&mut [u8]>) -> Result<usize, BodyError> {
    let mut cursor = 0usize;
    let mut written = 0usize;
    loop {
        let line_end = bytes[cursor..]
            .windows(2)
            .position(|pair| pair == b"\r\n")
            .map(|offset| cursor + offset)
            .ok_or(BodyError::Truncated)?;
        let size = chunk_size(&bytes[cursor..line_end])?;
        cursor = line_end.checked_add(2).ok_or(BodyError::Malformed)?;
        if size == 0 {
            loop {
                let trailer_end = bytes[cursor..]
                    .windows(2)
                    .position(|pair| pair == b"\r\n")
                    .map(|offset| cursor + offset)
                    .ok_or(BodyError::Truncated)?;
                let trailer = &bytes[cursor..trailer_end];
                cursor = trailer_end.checked_add(2).ok_or(BodyError::Malformed)?;
                if trailer.is_empty() {
                    return Ok(written);
                }
                if !trailer.contains(&b':') {
                    return Err(BodyError::Malformed);
                }
            }
        }
        let data_end = cursor.checked_add(size).ok_or(BodyError::Malformed)?;
        let terminator_end = data_end.checked_add(2).ok_or(BodyError::Malformed)?;
        if terminator_end > bytes.len() || &bytes[data_end..terminator_end] != b"\r\n" {
            return Err(BodyError::Truncated);
        }
        if let Some(destination) = output.as_deref_mut() {
            let end = written.checked_add(size).ok_or(BodyError::OutputTooSmall)?;
            if end > destination.len() {
                return Err(BodyError::OutputTooSmall);
            }
            destination[written..end].copy_from_slice(&bytes[cursor..data_end]);
        }
        written = written.checked_add(size).ok_or(BodyError::OutputTooSmall)?;
        cursor = terminator_end;
    }
}

/// Decode an HTTP/1.1 chunked body into caller-owned storage.
pub fn decode_chunked_body(bytes: &[u8], output: &mut [u8]) -> Result<usize, BodyError> {
    let required = walk_chunked(bytes, None)?;
    if required > output.len() {
        return Err(BodyError::OutputTooSmall);
    }
    walk_chunked(bytes, Some(output))
}

#[cfg(test)]
mod tests {
    use super::{BodyError, ParseError, decode_chunked_body, parse_endpoint, parse_response_head};

    #[test]
    fn endpoint_parser_defaults_ports_and_rejects_credentials() {
        assert_eq!(
            parse_endpoint("https://wovenhat.invalid/docs").unwrap(),
            super::Endpoint {
                host: "wovenhat.invalid",
                port: 443,
                path: "/docs",
                secure: true
            }
        );
        assert_eq!(
            parse_endpoint("http://user@host/"),
            Err(ParseError::InvalidCharacter)
        );
        assert_eq!(
            parse_endpoint("http://host:bad/"),
            Err(ParseError::InvalidPort)
        );
    }

    #[test]
    fn response_parser_is_bounded_and_extracts_content_length() {
        let head = b"HTTP/1.1 200 OK\r\nContent-Length: 42\r\n\r\n";
        assert_eq!(parse_response_head(head).unwrap().status, 200);
        assert_eq!(parse_response_head(head).unwrap().content_length, Some(42));
        assert!(!parse_response_head(head).unwrap().chunked);
        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(parse_response_head(chunked).unwrap().chunked);
        assert!(parse_response_head(b"NOPE\r\n\r\n").is_none());
    }

    #[test]
    fn decodes_chunked_body_and_rejects_bad_terminators() {
        let encoded = b"4\r\nWiki\r\n5;ext=x\r\npedia\r\n0\r\nX-Test: yes\r\n\r\n";
        let mut output = [0u8; 9];
        let length = decode_chunked_body(encoded, &mut output).unwrap();
        assert_eq!(length, 9);
        assert_eq!(&output, b"Wikipedia");
        assert_eq!(
            decode_chunked_body(b"3\r\nabcX\r\n0\r\n\r\n", &mut output),
            Err(BodyError::Truncated)
        );
    }
}
