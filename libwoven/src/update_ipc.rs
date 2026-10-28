//! Bounded WovenUpdate service protocol carried over handle-addressed IPC.
//!
//! The protocol carries update control metadata and a candidate path, never
//! the image bytes. The privileged service owns signature verification,
//! durable A/B state, and the storage/boot handoff. Validation here keeps
//! malformed requests out of that service boundary before an IPC send.

pub const MAGIC: [u8; 4] = *b"WUDI";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 4 + 1 + 1 + 2 + 8 + 8 + 1;
pub const MAX_PATH_BYTES: usize = 240;
pub const MAX_MESSAGE_BYTES: usize = 4096;
pub const NO_SLOT: u8 = 0xff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    BadLength,
    UnknownOperation,
    BadGeneration,
    BadSlot,
    BadPath,
    OutputTooSmall,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Stage = 1,
    Activate = 2,
    BootSucceeded = 3,
    BootFailed = 4,
    Query = 5,
}

impl Operation {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            1 => Ok(Self::Stage),
            2 => Ok(Self::Activate),
            3 => Ok(Self::BootSucceeded),
            4 => Ok(Self::BootFailed),
            5 => Ok(Self::Query),
            _ => Err(Error::UnknownOperation),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub operation: Operation,
    pub request_id: u64,
    pub generation: u64,
    pub slot: u8,
    pub source: &'a [u8],
}

impl<'a> Request<'a> {
    pub fn encode(self, output: &mut [u8]) -> Result<usize, Error> {
        validate(self.operation, self.generation, self.slot, self.source)?;
        let total = HEADER_LEN
            .checked_add(self.source.len())
            .ok_or(Error::BadLength)?;
        if total > output.len()
            || total > MAX_MESSAGE_BYTES
            || self.source.len() > u16::MAX as usize
        {
            return Err(Error::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = self.operation as u8;
        output[6..8].copy_from_slice(&((self.source.len()) as u16).to_le_bytes());
        output[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        output[16..24].copy_from_slice(&self.generation.to_le_bytes());
        output[24] = self.slot;
        output[25..total].copy_from_slice(self.source);
        Ok(total)
    }

    pub fn decode(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER_LEN {
            return Err(Error::TooShort);
        }
        if bytes[..4] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[4] != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let source_length = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        let total = HEADER_LEN
            .checked_add(source_length)
            .ok_or(Error::BadLength)?;
        if total != bytes.len() || total > MAX_MESSAGE_BYTES {
            return Err(Error::BadLength);
        }
        let operation = Operation::decode(bytes[5])?;
        let request = Self {
            operation,
            request_id: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            generation: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            slot: bytes[24],
            source: &bytes[25..total],
        };
        validate(
            request.operation,
            request.generation,
            request.slot,
            request.source,
        )?;
        Ok(request)
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
        output[6..8].copy_from_slice(&10u16.to_le_bytes());
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

fn validate(operation: Operation, generation: u64, slot: u8, source: &[u8]) -> Result<(), Error> {
    match operation {
        Operation::Stage => {
            if generation == 0 || slot != NO_SLOT {
                return Err(if generation == 0 {
                    Error::BadGeneration
                } else {
                    Error::BadSlot
                });
            }
            validate_source(source)
        }
        Operation::Activate | Operation::BootSucceeded | Operation::BootFailed => {
            if generation != 0 || slot >= 2 {
                return Err(if generation != 0 {
                    Error::BadGeneration
                } else {
                    Error::BadSlot
                });
            }
            if source.is_empty() {
                Ok(())
            } else {
                Err(Error::BadPath)
            }
        }
        Operation::Query => {
            if generation != 0 || slot != NO_SLOT {
                return Err(if generation != 0 {
                    Error::BadGeneration
                } else {
                    Error::BadSlot
                });
            }
            if source.is_empty() {
                Ok(())
            } else {
                validate_source(source)
            }
        }
    }
}

fn validate_source(source: &[u8]) -> Result<(), Error> {
    if source.is_empty()
        || source.len() > MAX_PATH_BYTES
        || !source.starts_with(b"/updates/")
        || source.windows(2).any(|window| window == b"..")
        || source.iter().any(|byte| {
            !matches!(*byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'-')
        })
    {
        return Err(Error::BadPath);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Error, NO_SLOT, Operation, Request, Response};

    #[test]
    fn stage_request_round_trip_is_bounded() {
        let request = Request {
            operation: Operation::Stage,
            request_id: 7,
            generation: 12,
            slot: NO_SLOT,
            source: b"/updates/kernel-12.wupd",
        };
        let mut bytes = [0u8; 128];
        let length = request.encode(&mut bytes).unwrap();
        assert_eq!(Request::decode(&bytes[..length]).unwrap(), request);
    }

    #[test]
    fn lifecycle_operations_require_the_right_fields() {
        let mut bytes = [0u8; 128];
        let request = Request {
            operation: Operation::BootSucceeded,
            request_id: 8,
            generation: 0,
            slot: 1,
            source: b"",
        };
        let length = request.encode(&mut bytes).unwrap();
        assert_eq!(Request::decode(&bytes[..length]).unwrap(), request);
        assert_eq!(
            (Request {
                operation: Operation::Stage,
                request_id: 1,
                generation: 0,
                slot: NO_SLOT,
                source: b"/updates/kernel.wupd",
            })
            .encode(&mut bytes),
            Err(Error::BadGeneration)
        );
    }

    #[test]
    fn source_paths_are_confined_to_updates() {
        let mut bytes = [0u8; 128];
        let bad = Request {
            operation: Operation::Stage,
            request_id: 1,
            generation: 1,
            slot: NO_SLOT,
            source: b"/wfs/kernel.wupd",
        };
        assert_eq!(bad.encode(&mut bytes), Err(Error::BadPath));
        let traversal = Request {
            operation: Operation::Stage,
            request_id: 1,
            generation: 1,
            slot: NO_SLOT,
            source: b"/updates/../kernel.wupd",
        };
        assert_eq!(traversal.encode(&mut bytes), Err(Error::BadPath));
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
