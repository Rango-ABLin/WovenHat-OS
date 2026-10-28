//! USB mass storage, Bulk-Only Transport with SCSI commands (Stage 13.18).
//!
//! The protocol half of the driver: interface discovery in a configuration
//! descriptor, Command Block Wrappers, Command Status Wrapper validation and
//! the SCSI command blocks the disk needs. It is pure computation over byte
//! buffers, so the host tests in `tests/usb_msc.rs` check it exhaustively;
//! the transfers themselves are in `xhci.rs`.

/// Interface class, subclass and protocol of a Bulk-Only SCSI disk.
const CLASS_MASS_STORAGE: u8 = 0x08;
const SUBCLASS_SCSI: u8 = 0x06;
const PROTOCOL_BULK_ONLY: u8 = 0x50;

const DESCRIPTOR_INTERFACE: u8 = 4;
const DESCRIPTOR_ENDPOINT: u8 = 5;
const ENDPOINT_BULK: u8 = 0x02;

const CBW_SIGNATURE: u32 = 0x4342_5355;
const CSW_SIGNATURE: u32 = 0x5342_5355;
/// Bytes in a Command Block Wrapper and a Command Status Wrapper.
pub const CBW_LENGTH: usize = 31;
pub const CSW_LENGTH: usize = 13;

/// The disk's interface in its configuration, with its two bulk endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interface {
    pub configuration: u8,
    pub interface: u8,
    pub bulk_in: u8,
    pub bulk_in_max_packet: u16,
    pub bulk_out: u8,
    pub bulk_out_max_packet: u16,
}

/// Find a Bulk-Only SCSI interface, with one bulk IN and one bulk OUT
/// endpoint, in a configuration descriptor and everything that follows it.
/// Malformed descriptors end the walk.
pub fn parse_interface(config: &[u8], configuration: u8) -> Option<Interface> {
    let mut offset = 0;
    let mut current: Option<Interface> = None;
    while offset + 2 <= config.len() {
        let length = usize::from(config[offset]);
        if length < 2 || offset + length > config.len() {
            return None;
        }
        let descriptor = &config[offset..offset + length];
        match descriptor[1] {
            DESCRIPTOR_INTERFACE if length >= 9 => {
                if let Some(found) = current.filter(complete) {
                    return Some(found);
                }
                current = (descriptor[5] == CLASS_MASS_STORAGE
                    && descriptor[6] == SUBCLASS_SCSI
                    && descriptor[7] == PROTOCOL_BULK_ONLY)
                    .then_some(Interface {
                        configuration,
                        interface: descriptor[2],
                        bulk_in: 0,
                        bulk_in_max_packet: 0,
                        bulk_out: 0,
                        bulk_out_max_packet: 0,
                    });
            }
            DESCRIPTOR_ENDPOINT if length >= 7 => {
                if let Some(interface) = current.as_mut() {
                    let address = descriptor[2];
                    let max_packet = u16::from_le_bytes([descriptor[4], descriptor[5]]) & 0x07ff;
                    if descriptor[3] & 0x03 == ENDPOINT_BULK
                        && address & 0x0f != 0
                        && max_packet != 0
                    {
                        if address & 0x80 != 0 && interface.bulk_in == 0 {
                            interface.bulk_in = address;
                            interface.bulk_in_max_packet = max_packet;
                        } else if address & 0x80 == 0 && interface.bulk_out == 0 {
                            interface.bulk_out = address;
                            interface.bulk_out_max_packet = max_packet;
                        }
                    }
                }
            }
            _ => {}
        }
        offset += length;
    }
    current.filter(complete)
}

fn complete(interface: &Interface) -> bool {
    interface.bulk_in != 0 && interface.bulk_out != 0
}

/// A Command Block Wrapper for LUN 0: `tag` is echoed in the status,
/// `length` bytes of data follow in the direction `data_in` gives.
pub fn command_block(
    tag: u32,
    length: u32,
    data_in: bool,
    command: &[u8],
) -> Option<[u8; CBW_LENGTH]> {
    if command.is_empty() || command.len() > 16 {
        return None;
    }
    let mut block = [0u8; CBW_LENGTH];
    block[0..4].copy_from_slice(&CBW_SIGNATURE.to_le_bytes());
    block[4..8].copy_from_slice(&tag.to_le_bytes());
    block[8..12].copy_from_slice(&length.to_le_bytes());
    block[12] = if data_in { 0x80 } else { 0 };
    block[13] = 0;
    block[14] = command.len() as u8;
    block[15..15 + command.len()].copy_from_slice(command);
    Some(block)
}

/// What a Command Status Wrapper reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The command completed; `residue` bytes of the expected data were not
    /// transferred.
    Passed { residue: u32 },
    /// The command failed; REQUEST SENSE says why.
    Failed,
    /// The device and host disagree about the transfer: only a reset
    /// recovery restores the transport.
    PhaseError,
}

/// Why a Command Status Wrapper was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusError {
    Length,
    Signature,
    Tag,
    /// A status byte the specification does not define.
    Unknown,
    /// More residue than data was asked for.
    Residue,
}

/// Validate the status for the command sent with `tag` and `length`.
pub fn parse_status(bytes: &[u8], tag: u32, length: u32) -> Result<Status, StatusError> {
    if bytes.len() < CSW_LENGTH {
        return Err(StatusError::Length);
    }
    if u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != CSW_SIGNATURE {
        return Err(StatusError::Signature);
    }
    if u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) != tag {
        return Err(StatusError::Tag);
    }
    let residue = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    match bytes[12] {
        0 if residue <= length => Ok(Status::Passed { residue }),
        0 => Err(StatusError::Residue),
        1 => Ok(Status::Failed),
        2 => Ok(Status::PhaseError),
        _ => Err(StatusError::Unknown),
    }
}

/// SCSI TEST UNIT READY.
pub const fn test_unit_ready() -> [u8; 6] {
    [0x00, 0, 0, 0, 0, 0]
}

/// SCSI REQUEST SENSE for `length` bytes of fixed-format sense data.
pub const fn request_sense(length: u8) -> [u8; 6] {
    [0x03, 0, 0, 0, length, 0]
}

/// SCSI INQUIRY for `length` bytes of standard data.
pub const fn inquiry(length: u8) -> [u8; 6] {
    [0x12, 0, 0, 0, length, 0]
}

/// SCSI READ CAPACITY (10).
pub const fn read_capacity() -> [u8; 10] {
    [0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// SCSI READ (10) or WRITE (10) of `blocks` blocks at `lba`.
pub fn read_write(write: bool, lba: u32, blocks: u16) -> [u8; 10] {
    let lba = lba.to_be_bytes();
    let blocks = blocks.to_be_bytes();
    [
        if write { 0x2a } else { 0x28 },
        0,
        lba[0],
        lba[1],
        lba[2],
        lba[3],
        0,
        blocks[0],
        blocks[1],
        0,
    ]
}

/// SCSI SYNCHRONIZE CACHE (10) of the whole medium.
pub const fn synchronize_cache() -> [u8; 10] {
    [0x35, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// READ CAPACITY (10) data: (number of blocks, block length). `None` when
/// the medium is too large for the 10-byte commands or reports no blocks.
pub fn parse_capacity(data: &[u8]) -> Option<(u64, u32)> {
    let data: &[u8; 8] = data.get(..8)?.try_into().ok()?;
    let last = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let length = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if last == u32::MAX || length == 0 {
        return None;
    }
    Some((u64::from(last) + 1, length))
}

/// Fixed-format sense data: (sense key, additional sense code, qualifier).
pub fn parse_sense(data: &[u8]) -> Option<(u8, u8, u8)> {
    if data.len() < 14 || !matches!(data[0] & 0x7f, 0x70 | 0x71) {
        return None;
    }
    Some((data[2] & 0x0f, data[12], data[13]))
}
