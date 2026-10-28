//! Bounded WovenSearch service protocol.
//!
//! The service accepts only an explicit UTF-8 query and returns compact
//! identifier/kind/score hits.  It never accepts paths, file contents, or an
//! index mutation request, so discovering `woven.search` cannot become an
//! implicit filesystem enumeration capability.  Publishers populate their
//! own bounded index through a separately reviewed service-owned source
//! adapter.

use crate::semantic_search::{self, Hit, Kind};

pub const SERVICE_NAME: &[u8] = b"woven.search";
pub const MAGIC: [u8; 4] = *b"WSR1";
pub const VERSION: u8 = 1;
const HEADER_LEN: usize = 16;
pub const MAX_QUERY_BYTES: usize = semantic_search::MAX_QUERY_BYTES;
pub const MAX_MESSAGE_BYTES: usize = 256;
pub const SEARCH_ABI: u64 = 0x0001_0000;

const RESPONSE_FIXED_LEN: usize = HEADER_LEN + 2 + 1 + 1;
const ENCODED_HIT_LEN: usize = 8 + 1 + 1 + 2;
pub const RESPONSE_LEN: usize = RESPONSE_FIXED_LEN + semantic_search::MAX_HITS * ENCODED_HIT_LEN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    BadLength,
    UnknownOperation,
    InvalidQuery,
    InvalidHit,
    OutputTooSmall,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Describe = 1,
    Query = 2,
}

impl Operation {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            1 => Ok(Self::Describe),
            2 => Ok(Self::Query),
            _ => Err(Error::UnknownOperation),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub operation: Operation,
    pub request_id: u64,
    pub query: &'a [u8],
}

impl<'a> Request<'a> {
    pub fn encode(self, output: &mut [u8]) -> Result<usize, Error> {
        validate_query(self.query)?;
        let total = HEADER_LEN
            .checked_add(self.query.len())
            .ok_or(Error::BadLength)?;
        if total > output.len() || total > MAX_MESSAGE_BYTES {
            return Err(Error::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = self.operation as u8;
        output[6..8].copy_from_slice(&(self.query.len() as u16).to_le_bytes());
        output[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        output[HEADER_LEN..total].copy_from_slice(self.query);
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
        let length = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        let total = HEADER_LEN.checked_add(length).ok_or(Error::BadLength)?;
        if total != bytes.len() || total > MAX_MESSAGE_BYTES {
            return Err(Error::BadLength);
        }
        let query = &bytes[HEADER_LEN..];
        validate_query(query)?;
        Ok(Self {
            operation: Operation::decode(bytes[5])?,
            request_id: u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| Error::BadLength)?),
            query,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    pub request_id: u64,
    pub status: u16,
    pub hits: [Hit; semantic_search::MAX_HITS],
    pub hit_count: u8,
}

impl Response {
    pub const EMPTY_HIT: Hit = Hit {
        id: 0,
        kind: Kind::Application,
        score: 0,
    };

    pub const fn empty(request_id: u64, status: u16) -> Self {
        Self {
            request_id,
            status,
            hits: [Self::EMPTY_HIT; semantic_search::MAX_HITS],
            hit_count: 0,
        }
    }

    pub fn encode(self, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() < RESPONSE_LEN || self.hit_count as usize > semantic_search::MAX_HITS {
            return Err(Error::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = 0;
        output[6..8].copy_from_slice(&((RESPONSE_LEN - HEADER_LEN) as u16).to_le_bytes());
        output[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        output[16..18].copy_from_slice(&self.status.to_le_bytes());
        output[18] = self.hit_count;
        output[19] = 0;
        let mut offset = RESPONSE_FIXED_LEN;
        for hit in self.hits {
            output[offset..offset + 8].copy_from_slice(&hit.id.to_le_bytes());
            output[offset + 8] = hit.kind as u8;
            output[offset + 9] = 0;
            output[offset + 10..offset + 12].copy_from_slice(&hit.score.to_le_bytes());
            offset += ENCODED_HIT_LEN;
        }
        Ok(RESPONSE_LEN)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != RESPONSE_LEN
            || bytes[..4] != MAGIC
            || bytes[4] != VERSION
            || bytes[5] != 0
        {
            return Err(Error::BadLength);
        }
        if u16::from_le_bytes([bytes[6], bytes[7]]) as usize != RESPONSE_LEN - HEADER_LEN
            || bytes[19] != 0
        {
            return Err(Error::BadLength);
        }
        let hit_count = bytes[18] as usize;
        if hit_count > semantic_search::MAX_HITS {
            return Err(Error::InvalidHit);
        }
        let mut response = Self::empty(
            u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| Error::BadLength)?),
            u16::from_le_bytes([bytes[16], bytes[17]]),
        );
        response.hit_count = hit_count as u8;
        let mut offset = RESPONSE_FIXED_LEN;
        for index in 0..semantic_search::MAX_HITS {
            let kind = match bytes[offset + 8] {
                1 => Kind::Application,
                2 => Kind::File,
                3 => Kind::Setting,
                4 => Kind::Message,
                5 => Kind::Document,
                6 => Kind::Command,
                _ => return Err(Error::InvalidHit),
            };
            if bytes[offset + 9] != 0 {
                return Err(Error::BadLength);
            }
            let hit = Hit {
                id: u64::from_le_bytes(
                    bytes[offset..offset + 8]
                        .try_into()
                        .map_err(|_| Error::BadLength)?,
                ),
                kind,
                score: u16::from_le_bytes([bytes[offset + 10], bytes[offset + 11]]),
            };
            if index < hit_count && hit.score == 0 {
                return Err(Error::InvalidHit);
            }
            response.hits[index] = hit;
            offset += ENCODED_HIT_LEN;
        }
        Ok(response)
    }
}

fn validate_query(query: &[u8]) -> Result<(), Error> {
    if query.len() > MAX_QUERY_BYTES || core::str::from_utf8(query).is_err() {
        return Err(Error::InvalidQuery);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Error, Operation, Request, Response, RESPONSE_LEN, SEARCH_ABI};
    use crate::semantic_search::{Hit, Kind, MAX_HITS};

    #[test]
    fn request_round_trip_is_versioned_and_bounded() {
        let request = Request {
            operation: Operation::Query,
            request_id: 7,
            query: b"terminal settings",
        };
        let mut bytes = [0u8; 256];
        let length = request.encode(&mut bytes).unwrap();
        assert_eq!(Request::decode(&bytes[..length]).unwrap(), request);
        assert_ne!(SEARCH_ABI, 0);
    }

    #[test]
    fn request_rejects_invalid_utf8_and_oversized_queries() {
        let mut bytes = [0u8; 256];
        assert_eq!(
            Request {
                operation: Operation::Query,
                request_id: 1,
                query: &[0xff]
            }
            .encode(&mut bytes),
            Err(Error::InvalidQuery)
        );
        let oversized = [b'x'; super::MAX_QUERY_BYTES + 1];
        assert_eq!(
            Request {
                operation: Operation::Query,
                request_id: 1,
                query: &oversized
            }
            .encode(&mut bytes),
            Err(Error::InvalidQuery)
        );
    }

    #[test]
    fn fixed_response_round_trips_all_hit_slots() {
        let mut response = Response::empty(9, 0);
        response.hits[0] = Hit {
            id: 42,
            kind: Kind::Application,
            score: 2,
        };
        response.hit_count = 1;
        let mut bytes = [0u8; RESPONSE_LEN];
        assert_eq!(response.encode(&mut bytes), Ok(RESPONSE_LEN));
        assert_eq!(Response::decode(&bytes).unwrap(), response);
        assert_eq!(response.hits.len(), MAX_HITS);
    }
}
