//! Bounded WovenPackage service protocol carried over handle-addressed IPC.
//!
//! The protocol deliberately carries control metadata and a bounded path, not
//! package bytes. Package payloads remain in the authenticated WPKG/storage
//! path; the service owns the durable read/write boundary.

pub const MAGIC: [u8; 4] = *b"WPKI";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 4 + 1 + 1 + 2 + 8;
pub const MAX_PATH_BYTES: usize = 240;
pub const MAX_MESSAGE_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    BadLength,
    UnknownOperation,
    BadPath,
    OutputTooSmall,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Install = 1,
    Remove = 2,
    Rollback = 3,
    Query = 4,
}

impl Operation {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            1 => Ok(Self::Install),
            2 => Ok(Self::Remove),
            3 => Ok(Self::Rollback),
            4 => Ok(Self::Query),
            _ => Err(Error::UnknownOperation),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub operation: Operation,
    pub request_id: u64,
    pub path: &'a [u8],
}

impl<'a> Request<'a> {
    pub fn encode(self, output: &mut [u8]) -> Result<usize, Error> {
        validate_path(self.path)?;
        let total = HEADER_LEN
            .checked_add(1)
            .and_then(|value| value.checked_add(self.path.len()))
            .ok_or(Error::BadLength)?;
        if total > output.len() || total > MAX_MESSAGE_BYTES || self.path.len() > u16::MAX as usize
        {
            return Err(Error::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = self.operation as u8;
        output[6..8].copy_from_slice(&((total - HEADER_LEN) as u16).to_le_bytes());
        output[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        output[16] = self.path.len() as u8;
        output[17..total].copy_from_slice(self.path);
        Ok(total)
    }

    pub fn decode(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER_LEN + 1 {
            return Err(Error::TooShort);
        }
        if bytes[..4] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[4] != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let body_len = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        let total = HEADER_LEN.checked_add(body_len).ok_or(Error::BadLength)?;
        if total != bytes.len() || total > MAX_MESSAGE_BYTES {
            return Err(Error::BadLength);
        }
        let path_len = bytes[16] as usize;
        if path_len != body_len.saturating_sub(1) {
            return Err(Error::BadLength);
        }
        let path = &bytes[17..total];
        validate_path(path)?;
        Ok(Self {
            operation: Operation::decode(bytes[5])?,
            request_id: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            path,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    pub request_id: u64,
    pub status: u16,
    pub value: u64,
}

impl Response {
    pub const ENCODED_LEN: usize = 4 + 1 + 1 + 2 + 8 + 2 + 8;

    pub fn encode(self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < Self::ENCODED_LEN {
            return Err(Error::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = 0;
        output[6..8].copy_from_slice(&(10u16).to_le_bytes());
        output[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        output[16..18].copy_from_slice(&self.status.to_le_bytes());
        output[18..26].copy_from_slice(&self.value.to_le_bytes());
        Ok(Self::ENCODED_LEN)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != Self::ENCODED_LEN || bytes[..4] != MAGIC || bytes[4] != VERSION {
            return Err(Error::BadLength);
        }
        if bytes[5] != 0 || u16::from_le_bytes([bytes[6], bytes[7]]) != 10 {
            return Err(Error::BadLength);
        }
        Ok(Self {
            request_id: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            status: u16::from_le_bytes([bytes[16], bytes[17]]),
            value: u64::from_le_bytes(bytes[18..26].try_into().unwrap()),
        })
    }
}

fn validate_path(path: &[u8]) -> Result<(), Error> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.windows(2).any(|window| window == b"..")
        || path.iter().any(|byte| {
            !matches!(*byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'-')
        })
    {
        return Err(Error::BadPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Error, Operation, Request, Response};

    #[test]
    fn request_round_trip_is_bounded() {
        let request = Request {
            operation: Operation::Install,
            request_id: 7,
            path: b"/wfs/packages/demo.wpkg",
        };
        let mut bytes = [0u8; 128];
        let length = request.encode(&mut bytes).unwrap();
        assert_eq!(Request::decode(&bytes[..length]).unwrap(), request);
    }

    #[test]
    fn request_rejects_bad_paths_and_lengths() {
        let mut bytes = [0u8; 64];
        let bad = Request {
            operation: Operation::Query,
            request_id: 1,
            path: b"../escape",
        };
        assert_eq!(bad.encode(&mut bytes), Err(Error::BadPath));
        assert_eq!(Request::decode(b"WPKI"), Err(Error::TooShort));
    }

    #[test]
    fn response_round_trip_preserves_status_and_value() {
        let response = Response {
            request_id: 42,
            status: 5,
            value: 99,
        };
        let mut bytes = [0u8; Response::ENCODED_LEN];
        let length = response.encode(&mut bytes).unwrap();
        assert_eq!(Response::decode(&bytes[..length]).unwrap(), response);
    }
}
