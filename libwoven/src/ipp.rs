//! Bounded IPP/1.1 print-job records and fail-closed secure transport
//! boundaries. A platform network service supplies sockets, DNS-SD and TLS;
//! this crate never treats a plaintext or unverified endpoint as a printer.

use crate::web::{parse_endpoint, Endpoint};

pub const MAX_REQUEST: usize = 4096;
pub const MAX_ATTRIBUTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Invalid,
    Unsupported,
    InvalidEndpoint,
    InsecureEndpoint,
    Transport,
    Connect,
    Tls,
    Read,
    Http,
    Certificate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Printer<'a> {
    pub name: &'a str,
    pub endpoint: Endpoint<'a>,
}

/// A platform-owned transport must perform DNS resolution, TCP connection,
/// TLS 1.2+ negotiation, certificate-chain validation, hostname validation,
/// and revocation policy before returning an IPP response.
pub trait SecureTransport {
    fn post_ipp(
        &mut self,
        endpoint: &Endpoint<'_>,
        request: &[u8],
        response: &mut [u8],
    ) -> Result<usize, Error>;
}

pub struct Client<T> {
    transport: T,
    printer: Printer<'static>,
}

impl<T: SecureTransport> Client<T> {
    /// Construct only from an HTTPS endpoint. The caller must promote the
    /// endpoint's borrowed strings to a service-owned lifetime, so a client
    /// cannot outlive the discovery record it uses.
    pub fn connect(transport: T, uri: &'static str, name: &'static str) -> Result<Self, Error> {
        let endpoint = parse_endpoint(uri).map_err(|_| Error::InvalidEndpoint)?;
        if !endpoint.secure {
            return Err(Error::InsecureEndpoint);
        }
        Ok(Self {
            transport,
            printer: Printer { name, endpoint },
        })
    }

    pub fn submit(&mut self, request: &Request, response: &mut [u8]) -> Result<Response, Error> {
        let mut encoded = [0u8; MAX_REQUEST];
        let length = request.copy_to(&mut encoded)?;
        let received =
            self.transport
                .post_ipp(&self.printer.endpoint, &encoded[..length], response)?;
        let reply = parse_response(&response[..received])?;
        if reply.request_id != request.request_id() {
            return Err(Error::Invalid);
        }
        Ok(reply)
    }

    pub const fn printer(&self) -> Printer<'static> {
        self.printer
    }
}

/// Parse one DNS-SD/service-discovery result after the platform resolver has
/// authenticated the source. The record format is intentionally tiny and
/// deterministic: `name=<display>;uri=https://host[:port]/ipp`.
pub fn parse_discovery_record(record: &str) -> Result<Printer<'_>, Error> {
    let (name, uri) = record.split_once(";uri=").ok_or(Error::InvalidEndpoint)?;
    let name = name.strip_prefix("name=").ok_or(Error::InvalidEndpoint)?;
    if name.is_empty() || name.len() > 96 {
        return Err(Error::InvalidEndpoint);
    }
    let endpoint = parse_endpoint(uri).map_err(|_| Error::InvalidEndpoint)?;
    if !endpoint.secure {
        return Err(Error::InsecureEndpoint);
    }
    Ok(Printer { name, endpoint })
}

pub struct Request {
    bytes: [u8; MAX_REQUEST],
    len: usize,
    attributes: usize,
    request_id: u32,
}

impl Request {
    pub fn new(operation: u16, request_id: u32) -> Self {
        let mut request = Self {
            bytes: [0; MAX_REQUEST],
            len: 8,
            attributes: 0,
            request_id,
        };
        request.bytes[..2].copy_from_slice(&[1, 1]);
        request.bytes[2..4].copy_from_slice(&operation.to_be_bytes());
        request.bytes[4..8].copy_from_slice(&request_id.to_be_bytes());
        request
    }
    pub fn add_text(&mut self, group: u8, name: &str, value: &str) -> Result<(), Error> {
        if self.attributes == MAX_ATTRIBUTES
            || name.len() > u16::MAX as usize
            || value.len() > u16::MAX as usize
        {
            return Err(Error::TooLarge);
        }
        let needed = 1usize
            .checked_add(1 + 2 + name.len() + 2 + value.len())
            .ok_or(Error::TooLarge)?;
        if self
            .len
            .checked_add(needed)
            .is_none_or(|end| end > MAX_REQUEST)
        {
            return Err(Error::TooLarge);
        }
        self.bytes[self.len] = if self.attributes == 0 { group } else { 0x41 };
        self.len += 1;
        self.bytes[self.len] = 0x41;
        self.len += 1;
        self.put_string(name.as_bytes())?;
        self.put_string(value.as_bytes())?;
        self.attributes += 1;
        Ok(())
    }
    fn put_string(&mut self, value: &[u8]) -> Result<(), Error> {
        let length = u16::try_from(value.len()).map_err(|_| Error::TooLarge)?;
        let end = self
            .len
            .checked_add(2 + value.len())
            .ok_or(Error::TooLarge)?;
        if end > MAX_REQUEST {
            return Err(Error::TooLarge);
        }
        self.bytes[self.len..self.len + 2].copy_from_slice(&length.to_be_bytes());
        self.bytes[self.len + 2..end].copy_from_slice(value);
        self.len = end;
        Ok(())
    }
    pub fn copy_to(&self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < self.len {
            return Err(Error::TooLarge);
        }
        output[..self.len].copy_from_slice(&self.bytes[..self.len]);
        Ok(self.len)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub const fn request_id(&self) -> u32 {
        self.request_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    pub version: [u8; 2],
    pub status: u16,
    pub request_id: u32,
}

pub fn parse_response(bytes: &[u8]) -> Result<Response, Error> {
    if bytes.len() < 8 || bytes[0..2] != [1, 1] {
        return Err(Error::Invalid);
    }
    Ok(Response {
        version: [bytes[0], bytes[1]],
        status: u16::from_be_bytes([bytes[2], bytes[3]]),
        request_id: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTransport;
    impl SecureTransport for MockTransport {
        fn post_ipp(
            &mut self,
            endpoint: &Endpoint<'_>,
            request: &[u8],
            response: &mut [u8],
        ) -> Result<usize, Error> {
            assert!(endpoint.secure);
            assert_eq!(&request[..2], &[1, 1]);
            response[..8].copy_from_slice(&[1, 1, 0, 0, 0, 0, 0, 7]);
            Ok(8)
        }
    }
    #[test]
    fn request_and_response_are_bounded() {
        let mut r = Request::new(0x0002, 7);
        r.add_text(0x01, "attributes-charset", "utf-8").unwrap();
        assert_eq!(&r.as_bytes()[..8], &[1, 1, 0, 2, 0, 0, 0, 7]);
        assert_eq!(parse_response(r.as_bytes()).unwrap().request_id, 7);
    }
    #[test]
    fn malformed_response_fails_closed() {
        assert_eq!(parse_response(&[1, 1, 0]), Err(Error::Invalid));
    }

    #[test]
    fn discovery_and_secure_client_reject_plaintext() {
        let printer =
            parse_discovery_record("name=Office;uri=https://printer.example/ipp").unwrap();
        assert_eq!(printer.name, "Office");
        assert_eq!(
            parse_discovery_record("name=Office;uri=http://printer.example/ipp"),
            Err(Error::InsecureEndpoint)
        );
        assert!(matches!(
            Client::connect(MockTransport, "http://printer.example/ipp", "Office"),
            Err(Error::InsecureEndpoint)
        ));
    }

    #[test]
    fn secure_client_requires_transport_response_for_matching_request() {
        let mut client =
            Client::connect(MockTransport, "https://printer.example/ipp", "Office").unwrap();
        let request = Request::new(2, 7);
        let mut response = [0; 16];
        assert_eq!(
            client.submit(&request, &mut response).unwrap().request_id,
            7
        );
    }
}
