//! Bounded WovenAI broker protocol carried over handle-addressed IPC.
//!
//! Stage 23.1 defines only the controlled broker boundary. It does not embed
//! a model or provider: completion requests receive `NoProvider` until a later
//! provider stage supplies one. Inputs remain bounded UTF-8 text and never
//! grant the broker implicit access to files, network, secrets, or devices.

pub const MAGIC: [u8; 4] = *b"WAI1";
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 4 + 1 + 1 + 2 + 8;
pub const MAX_INPUT_BYTES: usize = 224;
pub const MAX_MESSAGE_BYTES: usize = HEADER_LEN + 2 + MAX_INPUT_BYTES;
pub const BROKER_ABI: u64 = 0x0001_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    BadLength,
    UnknownOperation,
    InvalidInput,
    OutputTooSmall,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Describe = 1,
    Complete = 2,
}

impl Operation {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            1 => Ok(Self::Describe),
            2 => Ok(Self::Complete),
            _ => Err(Error::UnknownOperation),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub operation: Operation,
    pub request_id: u64,
    pub input: &'a [u8],
}

impl<'a> Request<'a> {
    pub fn encode(self, output: &mut [u8]) -> Result<usize, Error> {
        validate_input(self.input)?;
        let total = HEADER_LEN
            .checked_add(2)
            .and_then(|value| value.checked_add(self.input.len()))
            .ok_or(Error::BadLength)?;
        if total > output.len() || total > MAX_MESSAGE_BYTES {
            return Err(Error::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = self.operation as u8;
        output[6..8].copy_from_slice(&((total - HEADER_LEN) as u16).to_le_bytes());
        output[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        output[16..18].copy_from_slice(&(self.input.len() as u16).to_le_bytes());
        output[18..total].copy_from_slice(self.input);
        Ok(total)
    }

    pub fn decode(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER_LEN + 2 {
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
        if total != bytes.len() || total > MAX_MESSAGE_BYTES || body_len < 2 {
            return Err(Error::BadLength);
        }
        let input_len = u16::from_le_bytes([bytes[16], bytes[17]]) as usize;
        if input_len != body_len - 2 {
            return Err(Error::BadLength);
        }
        let input = &bytes[18..total];
        validate_input(input)?;
        Ok(Self {
            operation: Operation::decode(bytes[5])?,
            request_id: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            input,
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

fn validate_input(input: &[u8]) -> Result<(), Error> {
    if input.len() > MAX_INPUT_BYTES || core::str::from_utf8(input).is_err() {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BROKER_ABI, Error, Operation, Request, Response};

    #[test]
    fn request_round_trip_is_bounded() {
        let request = Request {
            operation: Operation::Complete,
            request_id: 7,
            input: b"hello WovenHat",
        };
        let mut bytes = [0u8; super::MAX_MESSAGE_BYTES];
        let length = request.encode(&mut bytes).unwrap();
        assert_eq!(Request::decode(&bytes[..length]).unwrap(), request);
    }

    #[test]
    fn request_rejects_invalid_or_oversized_input() {
        let mut bytes = [0u8; super::MAX_MESSAGE_BYTES];
        let invalid = Request {
            operation: Operation::Complete,
            request_id: 1,
            input: &[0xff],
        };
        assert_eq!(invalid.encode(&mut bytes), Err(Error::InvalidInput));
        let oversized = [b'a'; super::MAX_INPUT_BYTES + 1];
        assert_eq!(
            Request {
                operation: Operation::Complete,
                request_id: 1,
                input: &oversized,
            }
            .encode(&mut bytes),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn response_round_trip_preserves_broker_abi() {
        let response = Response {
            request_id: 42,
            status: 0,
            value: BROKER_ABI,
        };
        let mut bytes = [0u8; Response::ENCODED_LEN];
        let length = response.encode(&mut bytes).unwrap();
        assert_eq!(Response::decode(&bytes[..length]).unwrap(), response);
    }
}
