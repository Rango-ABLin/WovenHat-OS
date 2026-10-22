//! Bounded WovenInstaller service protocol.
//!
//! The display process never receives storage authority. It sends only a
//! validated, fixed-capacity request to the supervised installer service.

pub const MAX_MESSAGE_BYTES: usize = 256;
pub const MAX_PLAN_BYTES: usize = 192;
pub const SERVICE_NAME: &[u8] = b"sys.storage.installer";
pub const STATUS_READY: u16 = 0;
pub const STATUS_BAD_REQUEST: u16 = 1;
pub const STATUS_NOT_READY: u16 = 2;
pub const STATUS_TARGET_UNAVAILABLE: u16 = 3;
pub const STATUS_INVALID_PLAN: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Invalid,
    TooLarge,
    ShortBuffer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Operation {
    Inventory = 1,
    Validate = 2,
    Install = 3,
    Cancel = 4,
}

impl Operation {
    fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Inventory),
            2 => Some(Self::Validate),
            3 => Some(Self::Install),
            4 => Some(Self::Cancel),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub operation: Operation,
    pub request_id: u32,
    pub plan: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    pub request_id: u32,
    pub status: u16,
    pub progress: u8,
    pub value: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    MissingField,
    InvalidField,
    Unsupported,
    Capacity,
    Confirmation,
}

/// Validate the compact service plan without allocating or touching storage.
/// Fields are canonical ASCII `key=value` pairs separated by semicolons.
pub fn validate_plan(plan: &[u8], target_sectors: u64, require_confirmation: bool) -> Result<(), PlanError> {
    let mut target = None;
    let mut partition = None;
    let mut filesystem = None;
    let mut username = None;
    let mut timezone = None;
    let mut keyboard = None;
    let mut network = None;
    let mut confirmed = false;
    if plan.is_empty() || plan.len() > MAX_PLAN_BYTES {
        return Err(PlanError::InvalidField);
    }
    for field in plan.split(|byte| *byte == b';') {
        let Some(separator) = field.iter().position(|byte| *byte == b'=') else {
            return Err(PlanError::InvalidField);
        };
        let key = &field[..separator];
        let value = &field[separator + 1..];
        match key {
            b"target-sectors" => target = Some(parse_positive(value)?),
            b"partition-sectors" => partition = Some(parse_positive(value)?),
            b"filesystem" if value == b"fat32" => filesystem = Some(()),
            b"filesystem" => return Err(PlanError::Unsupported),
            b"user" => username = Some(valid_username(value)?),
            b"timezone" if matches!(value, b"UTC" | b"Africa/Blantyre" | b"America/New_York" | b"Asia/Tokyo" | b"Europe/London") => timezone = Some(()),
            b"timezone" => return Err(PlanError::Unsupported),
            b"keyboard" if matches!(value, b"us" | b"gb" | b"de" | b"es" | b"fr") => keyboard = Some(()),
            b"keyboard" => return Err(PlanError::Unsupported),
            b"network" if matches!(value, b"disabled" | b"dhcp") => network = Some(()),
            b"network" => return Err(PlanError::Unsupported),
            b"confirmed" if value == b"1" => confirmed = true,
            _ => return Err(PlanError::InvalidField),
        }
    }
    let Some(target) = target else { return Err(PlanError::MissingField) };
    let Some(partition) = partition else { return Err(PlanError::MissingField) };
    if filesystem.is_none() || username.is_none() || timezone.is_none() || keyboard.is_none() || network.is_none() {
        return Err(PlanError::MissingField);
    }
    if target != target_sectors || partition < 32_768 || partition > target.saturating_sub(2_048) {
        return Err(PlanError::Capacity);
    }
    if require_confirmation && !confirmed {
        return Err(PlanError::Confirmation);
    }
    Ok(())
}

fn parse_positive(value: &[u8]) -> Result<u64, PlanError> {
    if value.is_empty() || value.len() > 20 || value.iter().any(|byte| !byte.is_ascii_digit()) {
        return Err(PlanError::InvalidField);
    }
    let mut result = 0u64;
    for byte in value {
        result = result.checked_mul(10).and_then(|value| value.checked_add(u64::from(byte - b'0'))).ok_or(PlanError::InvalidField)?;
    }
    if result == 0 { Err(PlanError::InvalidField) } else { Ok(result) }
}

fn valid_username(value: &[u8]) -> Result<(), PlanError> {
    if value.is_empty() || value.len() > 32 || !value[0].is_ascii_lowercase() || value.iter().any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-')) {
        Err(PlanError::InvalidField)
    } else {
        Ok(())
    }
}

impl Request<'_> {
    pub const HEADER_BYTES: usize = 8;

    pub fn encode(&self, output: &mut [u8]) -> Result<usize, CodecError> {
        if self.plan.len() > MAX_PLAN_BYTES || output.len() < Self::HEADER_BYTES + self.plan.len() {
            return Err(if self.plan.len() > MAX_PLAN_BYTES { CodecError::TooLarge } else { CodecError::ShortBuffer });
        }
        output[0..3].copy_from_slice(b"WI\x01");
        output[3] = self.operation as u8;
        output[4..8].copy_from_slice(&self.request_id.to_le_bytes());
        output[8..8 + self.plan.len()].copy_from_slice(self.plan);
        Ok(Self::HEADER_BYTES + self.plan.len())
    }

    pub fn decode(bytes: &[u8]) -> Result<Request<'_>, CodecError> {
        if bytes.len() < Self::HEADER_BYTES || bytes[0..3] != *b"WI\x01" {
            return Err(CodecError::Invalid);
        }
        let operation = Operation::decode(bytes[3]).ok_or(CodecError::Invalid)?;
        let plan = &bytes[Self::HEADER_BYTES..];
        if plan.len() > MAX_PLAN_BYTES || plan.iter().any(|byte| *byte < 0x20 || *byte == 0x7f) {
            return Err(if plan.len() > MAX_PLAN_BYTES { CodecError::TooLarge } else { CodecError::Invalid });
        }
        Ok(Request {
            operation,
            request_id: u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| CodecError::Invalid)?),
            plan,
        })
    }
}

impl Response {
    pub const ENCODED_LEN: usize = 16;

    pub fn encode(self, output: &mut [u8]) -> Result<usize, CodecError> {
        if output.len() < Self::ENCODED_LEN {
            return Err(CodecError::ShortBuffer);
        }
        output[0..4].copy_from_slice(b"WO\x01\0");
        output[4..8].copy_from_slice(&self.request_id.to_le_bytes());
        output[8..10].copy_from_slice(&self.status.to_le_bytes());
        output[10] = self.progress;
        output[11] = 0;
        output[12..16].copy_from_slice(&(self.value as u32).to_le_bytes());
        Ok(Self::ENCODED_LEN)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() != Self::ENCODED_LEN || bytes[0..3] != *b"WO\x01" {
            return Err(CodecError::Invalid);
        }
        Ok(Self {
            request_id: u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| CodecError::Invalid)?),
            status: u16::from_le_bytes(bytes[8..10].try_into().map_err(|_| CodecError::Invalid)?),
            progress: bytes[10],
            value: u64::from(u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| CodecError::Invalid)?)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_plan, Operation, Request, Response};

    #[test]
    fn request_round_trip_is_bounded() {
        let request = Request { operation: Operation::Validate, request_id: 7, plan: b"plan-v1" };
        let mut bytes = [0; 256];
        let length = request.encode(&mut bytes).unwrap();
        assert_eq!(Request::decode(&bytes[..length]).unwrap(), request);
    }

    #[test]
    fn rejects_bad_header_and_control_bytes() {
        assert!(Request::decode(b"bad").is_err());
        let mut bytes = [0; 256];
        let length = (Request { operation: Operation::Inventory, request_id: 1, plan: b"a\n" })
            .encode(&mut bytes).unwrap();
        assert!(Request::decode(&bytes[..length]).is_err());
    }

    #[test]
    fn response_round_trip_is_fixed_size() {
        let response = Response { request_id: 8, status: 3, progress: 42, value: 99 };
        let mut bytes = [0; Response::ENCODED_LEN];
        response.encode(&mut bytes).unwrap();
        assert_eq!(Response::decode(&bytes).unwrap(), response);
    }

    #[test]
    fn validates_capacity_and_install_confirmation() {
        let plan = b"target-sectors=100000;partition-sectors=50000;filesystem=fat32;user=banda;timezone=UTC;keyboard=us;network=disabled";
        assert!(validate_plan(plan, 100000, false).is_ok());
        assert_eq!(validate_plan(plan, 100000, true), Err(super::PlanError::Confirmation));
        assert!(validate_plan(b"target-sectors=100000;partition-sectors=1000;filesystem=fat32;user=banda;timezone=UTC;keyboard=us;network=disabled", 100000, false).is_err());
    }
}
