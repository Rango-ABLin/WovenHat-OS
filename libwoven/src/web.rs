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
    for header in bytes[line_end + 2..].split(|byte| *byte == b'\n') {
        let header = header.strip_suffix(b"\r").unwrap_or(header);
        if header.is_empty() {
            break;
        }
        let Some(separator) = header.iter().position(|byte| *byte == b':') else {
            return None;
        };
        let name = &header[..separator];
        let value = &header[separator + 1..];
        if name.eq_ignore_ascii_case(b"content-length") {
            let value = core::str::from_utf8(value.trim_ascii()).ok()?;
            content_length = Some(value.parse().ok()?);
        }
    }
    Some(ResponseHead {
        status,
        content_length,
    })
}

#[cfg(test)]
mod tests {
    use super::{ParseError, parse_endpoint, parse_response_head};

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
        assert!(parse_response_head(b"NOPE\r\n\r\n").is_none());
    }
}
