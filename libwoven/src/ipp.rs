//! Bounded IPP/1.1 print-job protocol records.
//!
//! This is a protocol codec, not a network client. Transport, printer
//! discovery, TLS, and user authorization remain outside the codec.

pub const MAX_REQUEST: usize = 4096;
pub const MAX_ATTRIBUTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Invalid,
    Unsupported,
}

pub struct Request {
    bytes: [u8; MAX_REQUEST],
    len: usize,
    attributes: usize,
}

impl Request {
    pub fn new(operation: u16, request_id: u32) -> Self {
        let mut request = Self {
            bytes: [0; MAX_REQUEST],
            len: 8,
            attributes: 0,
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
    pub fn copy_to(self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < self.len {
            return Err(Error::TooLarge);
        }
        output[..self.len].copy_from_slice(&self.bytes[..self.len]);
        Ok(self.len)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
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
}
