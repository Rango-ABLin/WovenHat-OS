//! Host-side Rustls transport for the HTTPS IPP client.
//!
//! The freestanding image intentionally keeps TLS behind the privileged
//! service boundary. This implementation is the real host/service transport:
//! it performs TCP connect, TLS negotiation, trust-root validation, hostname
//! validation, bounded HTTPS parsing, and timeout enforcement. It never falls
//! back to plaintext.

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use crate::ipp::{Error, SecureTransport};
use crate::web::Endpoint;

const MAX_HTTP_HEADERS: usize = 4096;
const MAX_HTTP_BODY: usize = 4096;

pub struct RustlsTransport {
    config: Arc<ClientConfig>,
    timeout: Duration,
    connect_address: Option<String>,
}

impl RustlsTransport {
    /// Build a transport from an explicit trust store. The caller must load
    /// only release-approved CA certificates; no ambient OS trust store is
    /// consulted.
    pub fn with_roots(roots: RootCertStore, timeout: Duration) -> Result<Self, Error> {
        if roots.is_empty() || timeout.is_zero() {
            return Err(Error::Certificate);
        }
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Self {
            config: Arc::new(config),
            timeout,
            connect_address: None,
        })
    }

    /// Build a transport that connects to an explicit address while retaining
    /// the endpoint host for TLS SNI and certificate-name validation. This is
    /// for controlled diagnostics and printers whose certificate DNS name is
    /// not present in local DNS; it never disables hostname validation.
    pub fn with_roots_and_connect_address(
        roots: RootCertStore,
        timeout: Duration,
        connect_address: &str,
    ) -> Result<Self, Error> {
        if connect_address.is_empty() {
            return Err(Error::Transport);
        }
        let mut transport = Self::with_roots(roots, timeout)?;
        transport.connect_address = Some(connect_address.to_owned());
        Ok(transport)
    }

    pub fn with_system_roots(timeout: Duration) -> Result<Self, Error> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Self::with_roots(roots, timeout)
    }
}

impl SecureTransport for RustlsTransport {
    fn post_ipp(
        &mut self,
        endpoint: &Endpoint<'_>,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, Error> {
        if !endpoint.secure || request.len() > MAX_HTTP_BODY || response.is_empty() {
            return Err(Error::InsecureEndpoint);
        }
        let address = self.connect_address.as_deref().unwrap_or(endpoint.host);
        let stream = TcpStream::connect((address, endpoint.port)).map_err(|_| Error::Connect)?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|_| Error::Connect)?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|_| Error::Connect)?;
        let name = ServerName::try_from(endpoint.host.to_owned()).map_err(|_| Error::Certificate)?;
        let connection = ClientConnection::new(Arc::clone(&self.config), name)
            .map_err(|_| Error::Certificate)?;
        let mut tls = StreamOwned::new(connection, stream);
        let header = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/ipp\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            endpoint.path,
            endpoint.host,
            request.len()
        );
        tls.write_all(header.as_bytes()).map_err(|_| Error::Tls)?;
        tls.write_all(request).map_err(|_| Error::Tls)?;
        tls.flush().map_err(|_| Error::Tls)?;

        let mut raw = [0u8; MAX_HTTP_HEADERS + MAX_HTTP_BODY];
        let mut length = 0;
        let header_end = loop {
            if length == raw.len() {
                return Err(Error::TooLarge);
            }
            let read = tls.read(&mut raw[length..]).map_err(|_| Error::Read)?;
            if read == 0 {
                return Err(Error::Read);
            }
            length += read;
            if let Some(end) = raw[..length].windows(4).position(|window| window == b"\r\n\r\n") {
                break end + 4;
            }
            if length > MAX_HTTP_HEADERS {
                return Err(Error::TooLarge);
            }
        };
        let status = parse_status(&raw[..header_end]).ok_or(Error::Http)?;
        if !(200..300).contains(&status) || has_chunked_encoding(&raw[..header_end]) {
            return Err(Error::Http);
        }
        let body_length = content_length(&raw[..header_end]).ok_or(Error::Http)?;
        if body_length > response.len() || body_length > MAX_HTTP_BODY {
            return Err(Error::TooLarge);
        }
        let already = length.saturating_sub(header_end);
        let copied = already.min(body_length);
        response[..copied].copy_from_slice(&raw[header_end..header_end + copied]);
        let mut received = copied;
        while received < body_length {
            let read = tls
                .read(&mut response[received..body_length])
                .map_err(|_| Error::Read)?;
            if read == 0 {
                return Err(Error::Read);
            }
            received += read;
        }
        Ok(received)
    }
}

fn parse_status(headers: &[u8]) -> Option<u16> {
    let line_end = headers.windows(2).position(|window| window == b"\r\n")?;
    let line = core::str::from_utf8(&headers[..line_end]).ok()?;
    let mut fields = line.split_ascii_whitespace();
    if fields.next()? != "HTTP/1.1" {
        return None;
    }
    fields.next()?.parse().ok()
}

fn content_length(headers: &[u8]) -> Option<usize> {
    for line in headers.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let separator = line.iter().position(|byte| *byte == b':')?;
        let (name, value) = line.split_at(separator);
        let value = &value[1..];
        if name.eq_ignore_ascii_case(b"content-length") {
            return core::str::from_utf8(value.trim_ascii()).ok()?.parse().ok();
        }
    }
    None
}

fn has_chunked_encoding(headers: &[u8]) -> bool {
    headers
        .split(|byte| *byte == b'\n')
        .any(|line| line.windows(7).any(|window| window.eq_ignore_ascii_case(b"chunked")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_headers_are_fail_closed_and_bounded() {
        assert_eq!(parse_status(b"HTTP/1.1 200 OK\r\n"), Some(200));
        assert_eq!(content_length(b"Content-Length: 8\r\n"), Some(8));
        assert!(has_chunked_encoding(b"Transfer-Encoding: chunked\r\n"));
        assert!(!has_chunked_encoding(b"Content-Length: 8\r\n"));
    }
}
